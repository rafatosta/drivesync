//! Connection-owned lifecycle and producer ingress; sessions never cross the public boundary.
use super::*;
use crate::{
    detectors::{
        context::{ContextConfig, PathRoot},
        Detectors,
    },
    models::SyncRootId,
    orchestrator::{self, OrchestratorConfig, OrchestratorHandle},
    synchronization::*,
};
use sha2::{Digest, Sha256};
use std::{os::unix::ffi::OsStrExt, path::PathBuf};

pub(super) struct Session {
    detectors: Arc<Detectors>,
    pub(super) orchestrator: Arc<OrchestratorHandle>,
    sources: Option<crate::detectors::sources::SourceTasks>,
    writer: std::sync::Mutex<Option<Arc<super::scope::WriterLease>>>,
}
impl Session {
    pub(super) async fn stop(&self) -> SynchronizationStatus {
        if let Some(sources) = &self.sources {
            // Close execution admission immediately, even if credential refresh or a blocking
            // source read must still finish before disconnect can remove the account.
            let (_, status) = tokio::join!(sources.stop(), self.orchestrator.stop());
            self.writer.lock().unwrap().take();
            status
        } else {
            let status = self.orchestrator.stop().await;
            self.writer.lock().unwrap().take();
            status
        }
    }
}
pub(super) fn error(kind: SynchronizationErrorKind) -> EngineError {
    EngineError::new(EngineErrorKind::Synchronization(kind))
}
fn validate(request: &StartSynchronizationRequest) -> EngineResult<()> {
    if request.account_id.0.trim().is_empty()
        || request.remote_root_id.0.trim().is_empty()
        || !request.local_root.is_absolute()
        || request.local_root.components().any(|part| {
            matches!(
                part,
                std::path::Component::ParentDir | std::path::Component::CurDir
            ) || crate::projections::private_trash_name(part.as_os_str())
        })
    {
        return Err(error(SynchronizationErrorKind::InvalidConfiguration));
    }
    Ok(())
}
/// Stable scope identity includes opaque account and provider-root IDs, never a local path alone.
pub(super) fn root_id(request: &StartSynchronizationRequest) -> SyncRootId {
    let mut hash = Sha256::new();
    for bytes in [
        request.account_id.0.as_bytes(),
        request.remote_root_id.0.as_bytes(),
        request.local_root.as_os_str().as_bytes(),
    ] {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    SyncRootId(format!("{:x}", hash.finalize()))
}
/// Separate sibling storage, never Accounts metadata, credentials, or an implicit migration.
pub(super) fn directory(accounts: &std::path::Path) -> PathBuf {
    let mut name = accounts.file_name().unwrap_or_default().to_os_string();
    name.push(".synchronization");
    accounts.with_file_name(name)
}
impl Engine {
    /// Starts one bounded coordination worker for an explicitly configured account.
    /// Identical running configuration is idempotent; changing an active scope requires stopping it.
    /// Read-only producers start immediately; mutations still require an executable Analyzer plan.
    pub async fn start_synchronization(
        &self,
        request: StartSynchronizationRequest,
    ) -> EngineResult<SynchronizationStatus> {
        self.start_scope(request, false, false).await
    }

    /// Starts an exact scope with explicit permission to associate equivalent preexisting replicas.
    /// Permission lasts only for this worker session; dashboard startup never calls this method.
    pub async fn start_synchronization_with_initial_association(
        &self,
        request: StartSynchronizationRequest,
    ) -> EngineResult<SynchronizationStatus> {
        self.start_scope(request, true, false).await
    }

    /// Explicitly enables recoverable trash propagation for this worker session only.
    /// Normal startup and initial-association startup leave this policy disabled.
    pub async fn start_synchronization_with_trash_propagation(
        &self,
        request: StartSynchronizationRequest,
    ) -> EngineResult<SynchronizationStatus> {
        self.start_scope(request, false, true).await
    }

    async fn start_scope(
        &self,
        request: StartSynchronizationRequest,
        initial_association_authorized: bool,
        trash_propagation_authorized: bool,
    ) -> EngineResult<SynchronizationStatus> {
        validate(&request)?;
        let inner = self.inner.clone();
        self.runtime()
            .spawn(async move {
                let mut sessions = inner.synchronizations.lock().await;
                if let Some(session) = sessions.get(&request.account_id) {
                    if !session.orchestrator.is_finished() {
                        if session.orchestrator.config().roots == request
                            && session.orchestrator.config().initial_association_authorized
                                == initial_association_authorized
                            && session.orchestrator.config().trash_propagation_authorized
                                == trash_propagation_authorized
                        {
                            return Ok(session.orchestrator.status());
                        }
                        return Err(error(SynchronizationErrorKind::AlreadyRunning));
                    }
                }
                if let Some(finished) = sessions.remove(&request.account_id) {
                    finished.stop().await;
                }
                // Active accounts cannot mutate overlapping local trees under independent plans.
                if sessions.iter().any(|(account, session)| {
                    account != &request.account_id && !session.orchestrator.is_finished() && {
                        let root = &session.orchestrator.config().roots.local_root;
                        request.local_root.starts_with(root)
                            || root.starts_with(&request.local_root)
                    }
                }) {
                    return Err(error(SynchronizationErrorKind::RootInUse));
                }
                let ownership = inner.ownership.account(&request.account_id).gate.clone();
                let _lease = ownership.clone().read_owned().await;
                let accounts = inner.accounts.clone();
                let client = inner.client.clone();
                let directory = inner.synchronization_directory.clone();
                let id = request.account_id.clone();
                let source_accounts = accounts.clone();
                let source_client = client.clone();
                let source_ownership = ownership.clone();
                #[cfg(test)]
                let sources_enabled = inner.sources_enabled;
                #[cfg(not(test))]
                let sources_enabled = true;
                let mut session = tokio::task::spawn_blocking(move || {
                    let account = accounts
                        .get(GetAccountRequest {
                            account_id: request.account_id.clone(),
                        })?
                        .account;
                    if account.state != AccountState::Connected {
                        return Err(error(SynchronizationErrorKind::AccountUnavailable));
                    }
                    let writer = Arc::new(super::scope::WriterLease::acquire(
                        &request.local_root,
                        &account.provider.0,
                        &account.provider_user_id,
                    )?);
                    let scope = root_id(&request);
                    let inventory =
                        crate::inventory::Inventory::open(crate::inventory::InventoryConfig {
                            directory: directory.clone(),
                            account_id: request.account_id.clone(),
                        })
                        .map_err(|_| error(SynchronizationErrorKind::InventoryUnavailable))?;
                    let has_inventory = !inventory
                        .enumerate(crate::inventory::models::InventoryScopeQuery {
                            account_id: request.account_id.clone(),
                            sync_root_id: scope.clone(),
                        })
                        .map_err(|_| error(SynchronizationErrorKind::InventoryUnavailable))?
                        .is_empty();
                    writer.bind(&directory, &scope, has_inventory)?;
                    let config = OrchestratorConfig {
                        expected_root_identity: Some(writer.identity.clone()),
                        initial_association_authorized,
                        trash_propagation_authorized,
                        sync_root_id: root_id(&request),
                        roots: request,
                        provider_id: account.provider,
                    };
                    let detectors = Detectors::open(ContextConfig {
                        inventory_directory: directory.clone(),
                        account_id: config.roots.account_id.clone(),
                        provider_id: config.provider_id.clone(),
                        sync_root_id: config.sync_root_id.clone(),
                        local_root: PathRoot(config.roots.local_root.clone()),
                        remote_root_id: config.roots.remote_root_id.clone(),
                        local_root_available: true,
                        remote_available: false,
                        initial_association_authorized,
                        trash_propagation_authorized,
                    })
                    .map_err(|_| error(SynchronizationErrorKind::InventoryUnavailable))?;
                    let orchestrator = orchestrator::open(
                        config,
                        directory,
                        accounts,
                        client,
                        ownership,
                        writer.clone(),
                    )
                    .map_err(error)?;
                    Ok::<_, EngineError>(Session {
                        detectors: Arc::new(detectors),
                        orchestrator: Arc::new(orchestrator),
                        sources: None,
                        writer: std::sync::Mutex::new(Some(writer)),
                    })
                })
                .await
                .map_err(|_| EngineError::new(EngineErrorKind::Internal))??;
                if sources_enabled {
                    session.sources = Some(crate::detectors::sources::start(
                        session.orchestrator.config().clone(),
                        session.detectors.clone(),
                        session.orchestrator.clone(),
                        source_accounts,
                        source_client,
                        source_ownership,
                    ));
                }
                let status = session.orchestrator.status();
                sessions.insert(id, session);
                Ok(status)
            })
            .await
            .map_err(|_| EngineError::new(EngineErrorKind::Internal))?
    }
    /// Stops admission and waits for the current operation's real receipt and worker termination.
    /// An uncertain operation is left in Inventory for recovery and is never automatically replayed.
    pub async fn stop_synchronization(
        &self,
        account_id: AccountId,
    ) -> EngineResult<SynchronizationStatus> {
        let inner = self.inner.clone();
        self.runtime()
            .spawn(async move {
                let sessions = inner.synchronizations.lock().await;
                Ok(match sessions.get(&account_id) {
                    Some(session) => session.stop().await,
                    None => SynchronizationStatus::new(account_id, SynchronizationState::Stopped),
                })
            })
            .await
            .map_err(|_| EngineError::new(EngineErrorKind::Internal))?
    }
    /// Attests captured replica convergence without equating an idle queue with completion.
    /// Busy/unreadable Inventory or a replaced root returns an error rather than partial proof.
    pub async fn synchronization_audit(
        &self,
        account_id: AccountId,
    ) -> EngineResult<SynchronizationAudit> {
        let (detectors, epoch, identity) = {
            let sessions = self.inner.synchronizations.lock().await;
            let session = sessions
                .get(&account_id)
                .filter(|s| !s.orchestrator.is_finished())
                .ok_or_else(|| error(SynchronizationErrorKind::NotStarted))?;
            (
                session.detectors.clone(),
                session.orchestrator.collection_epoch(),
                session
                    .orchestrator
                    .config()
                    .expected_root_identity
                    .clone()
                    .ok_or_else(|| error(SynchronizationErrorKind::InvalidConfiguration))?,
            )
        };
        self.runtime()
            .spawn_blocking(move || detectors.audit(epoch, &identity))
            .await
            .map_err(|_| EngineError::new(EngineErrorKind::Internal))?
            .map_err(|_| error(SynchronizationErrorKind::InventoryUnavailable))
    }

    /// Returns the newest confirmed user-visible effects for the account's configured roots.
    pub async fn recent_synchronization_activity(
        &self,
        account_id: AccountId,
        limit: usize,
    ) -> EngineResult<Vec<RecentActivity>> {
        use crate::inventory::{models::InventoryScopeQuery, Inventory, InventoryConfig};

        let sync_root_id = {
            let sessions = self.inner.synchronizations.lock().await;
            sessions
                .get(&account_id)
                .ok_or_else(|| error(SynchronizationErrorKind::NotStarted))?
                .orchestrator
                .config()
                .sync_root_id
                .clone()
        };
        let directory = self.inner.synchronization_directory.clone();
        self.runtime()
            .spawn_blocking(move || {
                let unavailable = || error(SynchronizationErrorKind::InventoryUnavailable);
                let inventory = Inventory::open(InventoryConfig {
                    directory,
                    account_id: account_id.clone(),
                })
                .map_err(|_| unavailable())?;
                let rows = inventory
                    .enumerate(InventoryScopeQuery {
                        account_id,
                        sync_root_id,
                    })
                    .map_err(|_| unavailable())?;
                let mut activities: Vec<_> = rows
                    .iter()
                    .flat_map(|row| &row.pending_operations)
                    .filter_map(|operation| {
                        recent_activity(operation).map(|activity| (&operation.id.0, activity))
                    })
                    .collect();
                activities.sort_unstable_by(|(left_id, left), (right_id, right)| {
                    right
                        .confirmed_at
                        .unix_millis
                        .cmp(&left.confirmed_at.unix_millis)
                        .then_with(|| left_id.cmp(right_id))
                });
                Ok(activities
                    .into_iter()
                    .take(limit)
                    .map(|(_, activity)| activity)
                    .collect())
            })
            .await
            .map_err(|_| EngineError::new(EngineErrorKind::Internal))?
    }

    /// Latest account status without I/O, authentication, or a connectivity claim.
    pub async fn synchronization_status(
        &self,
        account_id: AccountId,
    ) -> EngineResult<SynchronizationStatus> {
        let sessions = self.inner.synchronizations.lock().await;
        Ok(sessions
            .get(&account_id)
            .map(|s| s.orchestrator.status())
            .unwrap_or_else(|| {
                SynchronizationStatus::new(account_id, SynchronizationState::Stopped)
            }))
    }
    /// Subscribes to the latest state of an existing worker, usable from a GLib executor.
    pub async fn subscribe_synchronization(
        &self,
        account_id: AccountId,
    ) -> EngineResult<SynchronizationSubscription> {
        let sessions = self.inner.synchronizations.lock().await;
        sessions
            .get(&account_id)
            .map(|session| session.orchestrator.subscribe())
            .ok_or_else(|| error(SynchronizationErrorKind::NotStarted))
    }
    /// Normalized Observation ingress for Engine-owned integrations. Raw File/Decision/Operation
    /// values are never exposed as application commands or synthesized around an incomplete Context.
    #[allow(dead_code)] // Preserves normalized ingress for Engine-owned integrations.
    pub(crate) async fn observe(
        &self,
        account_id: AccountId,
        observation: Observation,
    ) -> EngineResult<()> {
        let inner = self.inner.clone();
        self.runtime()
            .spawn(async move {
                let (detectors, orchestrator) = {
                    let sessions = inner.synchronizations.lock().await;
                    let session = sessions
                        .get(&account_id)
                        .ok_or_else(|| error(SynchronizationErrorKind::NotStarted))?;
                    (session.detectors.clone(), session.orchestrator.clone())
                };
                let files = detectors
                    .contextualize_batch(observation)
                    .await
                    .map_err(|_| error(SynchronizationErrorKind::InvalidObservation))?;
                for file in files {
                    orchestrator.submit(file).await.map_err(error)?;
                }
                Ok(())
            })
            .await
            .map_err(|_| EngineError::new(EngineErrorKind::Internal))?
    }
}

use crate::detectors::observation::Observation;

fn recent_activity(
    operation: &crate::inventory::models::PendingOperation,
) -> Option<RecentActivity> {
    use crate::inventory::models::{OperationAction as Action, OperationState};
    use crate::models::Evidence;
    use RecentActivityKind as Kind;

    if operation.state != OperationState::Confirmed {
        return None;
    }
    let kind = match operation.action {
        Action::Upload => Kind::Upload,
        Action::Download => Kind::Download,
        Action::UpdateLocalContent | Action::UpdateRemoteContent => Kind::Update,
        Action::RenameLocal | Action::RenameRemote => Kind::Rename,
        Action::MoveLocal | Action::MoveRemote => Kind::Move,
        Action::RemoveLocal | Action::TrashRemote => Kind::Trash,
        Action::CreateLocalDirectory | Action::CreateRemoteDirectory => Kind::CreateFolder,
        Action::CreateLocalShortcut => Kind::ProjectShortcut,
        Action::RegisterIdentity | Action::Adopt | Action::ConfirmBaseline => return None,
    };
    let effect = operation.applied_effect.as_ref()?;
    let record = operation.recovery.as_deref();
    let path_name = |path: &std::path::Path| {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
    };
    let local_name = effect
        .local
        .as_ref()
        .and_then(|local| local.path.as_deref())
        .and_then(path_name);
    let remote_name = effect
        .remote
        .as_ref()
        .and_then(|remote| match &remote.name {
            Evidence::Known(name) => Some(name.clone()),
            _ => None,
        });
    let name = match operation.action {
        Action::RenameRemote => remote_name.or(local_name),
        _ => local_name.or(remote_name),
    }
    .or_else(|| record.and_then(|record| record.command.name.clone()))
    .or_else(|| {
        record
            .and_then(|record| record.command.local_path.as_deref())
            .and_then(path_name)
    })?;

    let size = match operation.action {
        Action::Upload | Action::UpdateRemoteContent => {
            effect.remote.as_ref().map(|remote| &remote.size)
        }
        Action::Download | Action::UpdateLocalContent => {
            effect.local.as_ref().map(|local| &local.size)
        }
        _ => None,
    };
    Some(RecentActivity {
        kind,
        name,
        size: match size {
            Some(Evidence::Known(bytes)) => Some(*bytes),
            _ => None,
        },
        confirmed_at: operation.updated_at,
    })
}
