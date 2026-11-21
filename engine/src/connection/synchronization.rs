//! Connection-owned lifecycle and producer ingress; sessions never cross the public boundary.
use super::*;
use crate::{
    detectors::{
        context::{ContextConfig, PathRoot},
        observation::Observation,
        Detectors,
    },
    models::SyncRootId,
    orchestrator::{self, OrchestratorConfig, OrchestratorHandle},
    synchronization::*,
};
use sha2::{Digest, Sha256};
use std::{os::unix::ffi::OsStrExt, path::PathBuf};

pub(super) struct Session {
    #[allow(dead_code)] // Functional producer ingress is implemented in reconstruction step 9.
    detectors: Arc<Detectors>,
    pub(super) orchestrator: Arc<OrchestratorHandle>,
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
            )
        })
    {
        return Err(error(SynchronizationErrorKind::InvalidConfiguration));
    }
    Ok(())
}
/// Stable scope identity includes opaque account and provider-root IDs, never a local path alone.
fn root_id(request: &StartSynchronizationRequest) -> SyncRootId {
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
    /// No scanner, network mutation, or claim of synchronization is started by this request.
    pub async fn start_synchronization(
        &self,
        request: StartSynchronizationRequest,
    ) -> EngineResult<SynchronizationStatus> {
        validate(&request)?;
        let inner = self.inner.clone();
        self.runtime()
            .spawn(async move {
                let mut sessions = inner.synchronizations.lock().await;
                if let Some(session) = sessions.get(&request.account_id) {
                    if !session.orchestrator.is_finished() {
                        if session.orchestrator.config().roots == request {
                            return Ok(session.orchestrator.status());
                        }
                        return Err(error(SynchronizationErrorKind::AlreadyRunning));
                    }
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
                let _lease = inner.ownership.gate.clone().read_owned().await;
                let accounts = inner.accounts.clone();
                let client = inner.client.clone();
                let ownership = inner.ownership.gate.clone();
                let directory = inner.synchronization_directory.clone();
                let id = request.account_id.clone();
                let session = tokio::task::spawn_blocking(move || {
                    let account = accounts
                        .get(GetAccountRequest {
                            account_id: request.account_id.clone(),
                        })?
                        .account;
                    if account.state != AccountState::Connected {
                        return Err(error(SynchronizationErrorKind::AccountUnavailable));
                    }
                    let config = OrchestratorConfig {
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
                        initial_association_authorized: false,
                    })
                    .map_err(|_| error(SynchronizationErrorKind::InventoryUnavailable))?;
                    let orchestrator =
                        orchestrator::open(config, directory, accounts, client, ownership)
                            .map_err(error)?;
                    Ok::<_, EngineError>(Session {
                        detectors: Arc::new(detectors),
                        orchestrator: Arc::new(orchestrator),
                    })
                })
                .await
                .map_err(|_| EngineError::new(EngineErrorKind::Internal))??;
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
                    Some(session) => session.orchestrator.stop().await,
                    None => SynchronizationStatus::new(account_id, SynchronizationState::Stopped),
                })
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
    /// Sole Observation ingress for later Engine-owned producers. Raw File/Decision/Operation
    /// values are never exposed as application commands or synthesized around an incomplete Context.
    #[allow(dead_code)] // Internal producer entry point for reconstruction step 9.
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
                let file =
                    tokio::task::spawn_blocking(move || detectors.contextualize(observation))
                        .await
                        .map_err(|_| EngineError::new(EngineErrorKind::Internal))?
                        .map_err(|_| error(SynchronizationErrorKind::InvalidObservation))?;
                orchestrator.submit(file).await.map_err(error)
            })
            .await
            .map_err(|_| EngineError::new(EngineErrorKind::Internal))?
    }
}
