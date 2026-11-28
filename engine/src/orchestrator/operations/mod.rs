//! Execution with fixed scope, optimistic persistence, and explicit uncertain effects.
mod activity;
mod conflicts;
mod filesystem;
mod google;
mod interfaces;
#[cfg(test)]
pub(super) use interfaces::RecoveryObserver;
mod journal;
pub(super) mod models;
mod recovery;
mod trash;
use crate::{
    accounts::{Accounts, GetAccountRequest},
    activity::ActivityContext,
    inventory::{models::*, Inventory},
    models::*,
};
pub(super) use activity::{Activity, ActivityInbox, Scope as ActivityScope};
use activity::{Effect, Kind, Outcome, Phase, Span, State, Wait};
#[cfg(test)]
pub(super) use activity::{
    Phase as OperationPhase, State as OperationActivityState, Wait as OperationWait,
};
use filesystem::Filesystem;
#[cfg(test)]
use interfaces::EffectFuture;
#[cfg(test)]
pub(super) use interfaces::RecoveryFuture;
use interfaces::RemoteExecutor;
#[allow(unused_imports)]
pub(super) use interfaces::{OperationFuture, Operator, OperatorFactory};
#[cfg(test)]
use journal::ExecutionJournal;
use journal::Journal;
use models::*;
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

/// The built-in factory supports Google only; unknown providers never fall back to Google.
pub(super) struct DefaultOperatorFactory {
    accounts: Accounts,
    inventory: Arc<Inventory>,
    google: Arc<dyn RemoteExecutor>,
}
impl DefaultOperatorFactory {
    pub(super) fn new(
        accounts: Accounts,
        inventory: Arc<Inventory>,
        client: crate::providers::google::GoogleDriveClient,
    ) -> Self {
        Self {
            google: Arc::new(google::GoogleOperator::new(accounts.clone(), client)),
            accounts,
            inventory,
        }
    }
}
impl OperatorFactory for DefaultOperatorFactory {
    fn create(&self, config: OperatorConfig) -> Result<Box<dyn Operator>, OperatorError> {
        if config.provider_id.0 != "google" {
            return Err(OperatorError::new(OperatorErrorKind::UnsupportedProvider));
        }
        let account = self
            .accounts
            .get(GetAccountRequest {
                account_id: config.account_id.clone(),
            })
            .map_err(|_| OperatorError::new(OperatorErrorKind::PermissionDenied))?
            .account;
        if account.provider != config.provider_id {
            return Err(OperatorError::new(OperatorErrorKind::PermissionDenied));
        }
        if config.sync_root_id.0.is_empty() || config.remote_root_id.0.is_empty() {
            return Err(filesystem::invalid());
        }
        let filesystem = Filesystem::open(&config.local_root)?;
        if config
            .expected_root_identity
            .as_ref()
            .is_some_and(|expected| filesystem.root_identity().ok().as_ref() != Some(expected))
        {
            return Err(filesystem::stale());
        }
        Ok(Box::new(PersistingOperator {
            config,
            inventory: self.inventory.clone(),
            filesystem,
            remote: self.google.clone(),
            gate: tokio::sync::Mutex::new(()),
        }))
    }
}
struct PersistingOperator {
    config: OperatorConfig,
    inventory: Arc<Inventory>,
    filesystem: Filesystem,
    remote: Arc<dyn RemoteExecutor>,
    gate: tokio::sync::Mutex<()>,
}
pub(super) fn now() -> DateTime {
    DateTime {
        unix_millis: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(i64::MAX as u128) as i64,
    }
}
impl PersistingOperator {
    fn write(
        &self,
        request: &ExecuteOperationRequest,
        revision: Option<u64>,
        effect: PersistedEffect,
    ) -> InventoryWriteResult {
        let phase = match &effect {
            PersistedEffect::Pending(_) => Phase::RecordIntent,
            PersistedEffect::OperationConfirmed
            | PersistedEffect::BaselineConfirmed { .. }
            | PersistedEffect::ItemRegistered { .. }
            | PersistedEffect::ConvergenceConfirmed { .. } => Phase::Confirm,
            _ => Phase::PersistEffect,
        };
        activity::persist(phase, |context| {
            self.inventory.write_with_context(
                InventoryWrite {
                    operation_id: request.execution_id.clone(),
                    item_id: request.operation.item_ref.clone(),
                    identity: request.identity.clone(),
                    expected_revision: revision,
                    effect,
                    recorded_at: now(),
                },
                context,
            )
        })
    }
    /// Only corresponding, equivalent current replicas may replace a synchronization baseline.
    #[cfg(test)]
    fn convergence(&self, effect: &AppliedEffect) -> Option<(Baseline, Option<ParentAssociation>)> {
        self.convergence_with_context(effect, None)
    }

    fn convergence_with_context(
        &self,
        effect: &AppliedEffect,
        context: Option<&ActivityContext>,
    ) -> Option<(Baseline, Option<ParentAssociation>)> {
        if !effect.verified {
            return None;
        }
        let local = effect.local.as_ref()?;
        let remote = effect.remote.as_ref()?;
        if !super::evidence::equivalent(local, remote) {
            return None;
        }
        let (Evidence::Known(local_parent), Evidence::Known(remote_parents)) =
            (&local.parent, &remote.parents)
        else {
            return None;
        };
        let parent = self.confirmed_parent_with_context(local_parent, remote_parents, context)?;
        Some((
            Baseline {
                state: BaselineState::Synced,
                local: Some(local.clone()),
                remote: Some(remote.clone()),
                confirmed_at: Some(now()),
                projection: Some(if crate::projections::workspace(remote) {
                    ProjectionKind::GoogleWorkspaceShortcut
                } else {
                    ProjectionKind::Native
                }),
            },
            parent,
        ))
    }
    /// Initial child effects require an existing confirmed parent association before dispatch.
    fn confirmed_parent_with_context(
        &self,
        local_parent: &std::path::Path,
        remote_parents: &[RemoteId],
        context: Option<&ActivityContext>,
    ) -> Option<Option<ParentAssociation>> {
        if remote_parents.len() != 1 {
            return None;
        }
        let parent = if local_parent.as_os_str().is_empty()
            && remote_parents[0] == self.config.remote_root_id
        {
            None
        } else {
            let lookup = self.inventory.lookup_with_context(
                InventoryLookup {
                    account_id: self.config.account_id.clone(),
                    sync_root_id: self.config.sync_root_id.clone(),
                    item_id: None,
                    remote_id: Some(remote_parents[0].clone()),
                    local_identity: Evidence::NotCollected,
                    local_path: Some(local_parent.to_path_buf()),
                },
                context,
            );
            if lookup.status != InventoryLookupStatus::Found {
                return None;
            }
            let snapshot = lookup.snapshot?;
            let expected = snapshot.baseline.local.as_ref()?;
            if expected.kind != Evidence::Known(FileKind::Directory)
                || !self.filesystem.observe(local_parent).is_ok_and(|current| {
                    current.kind == expected.kind
                        && current.identity == snapshot.identity.local_identity
                })
                || snapshot.local_path.as_deref() != Some(local_parent)
                || snapshot.baseline.state != BaselineState::Synced
                || snapshot.pending_operations.iter().any(|op| {
                    matches!(
                        op.state,
                        OperationState::Pending
                            | OperationState::Running
                            | OperationState::AppliedUnconfirmed
                    )
                })
            {
                return None;
            }
            Some(ParentAssociation {
                item_id: snapshot.identity.item_id?,
                revision: snapshot.revision,
                remote_id: remote_parents[0].clone(),
                local_path: local_parent.to_path_buf(),
            })
        };
        Some(parent)
    }
    #[cfg(test)]
    fn baseline(&self, effect: &AppliedEffect) -> Option<Baseline> {
        self.convergence(effect).map(|(baseline, _)| baseline)
    }

    async fn metadata(
        &self,
        r: &ExecuteOperationRequest,
        mut result: OperationResult,
        span: &Span,
    ) -> OperationResult {
        let action = r.operation.action;
        if action == OperationAction::Adopt && !self.config.initial_association_authorized {
            result.error = Some(filesystem::stale().failure());
            return result;
        }
        let effect = match self
            .remote
            .execute(&self.config, r, &self.filesystem, None)
            .await
        {
            Ok(effect) if effect.verified => effect,
            _ => {
                result.error = Some(filesystem::stale().failure());
                return result;
            }
        };
        let persisted = if action == OperationAction::RegisterIdentity {
            if r.expected_revision != Some(0)
                || !matches!((&effect.local,&effect.remote),
                (Some(local),Some(remote)) if (local.state == FileState::Present && remote.state == FileState::Absent) || (local.state == FileState::Absent && remote.state == FileState::Present))
            {
                result.error = Some(filesystem::stale().failure());
                return result;
            }
            PersistedEffect::ItemRegistered {
                local_path: effect
                    .local
                    .as_ref()
                    .filter(|s| s.state == FileState::Present)
                    .and_then(|s| s.path.clone()),
            }
        } else {
            let inventory_context = span.activity_context();
            let Some((baseline, parent)) =
                self.convergence_with_context(&effect, inventory_context.as_ref())
            else {
                result.error = Some(filesystem::stale().failure());
                return result;
            };
            PersistedEffect::ConvergenceConfirmed {
                baseline,
                effect: Box::new(effect.clone()),
                parent,
            }
        };
        // No external effect is dispatched: this single transaction is the whole metadata action.
        let mut current = r.clone();
        if let Some(local) = &effect.local {
            current.identity.local_identity = local.identity.clone();
        }
        current.identity.remote_id = effect.remote_id.clone();
        span.phase(Phase::Confirm, State::Running);
        let receipt = self.write(&current, r.expected_revision, persisted);
        span.receipt(receipt.status);
        if receipt.status == WriteStatus::Written {
            span.effect(Effect::Verified, true);
        }
        result.status = if receipt.status == WriteStatus::Written {
            OperationResultStatus::Succeeded
        } else {
            OperationResultStatus::Skipped
        };
        result.inventory_write = Some(receipt);
        result.effect = Some(effect);
        result.completed_at = Some(now());
        result
    }
    /// Refresh only an already synchronized regular-file slot after a complete, unambiguous rescan.
    fn validate_replacement(
        &self,
        r: &ExecuteOperationRequest,
        stored: &InventorySnapshot,
    ) -> Result<(), OperatorError> {
        use crate::detectors::{verify_local_scope, LocalVerification};
        use std::os::unix::fs::MetadataExt;
        if (!matches!(
            r.operation.action,
            OperationAction::UpdateRemoteContent | OperationAction::ConfirmBaseline
        ) && r.operation.conflict_id.is_none())
            || stored.baseline.state != BaselineState::Synced
            || stored.local_path != r.local.path
            || !stored.baseline.local.as_ref().is_some_and(|old| {
                old.path == r.local.path && old.kind == Evidence::Known(FileKind::Regular)
            })
            || !stored.baseline.remote.as_ref().is_some_and(|old| {
                r.remote.unchanged_binary(old) || r.operation.conflict_id.is_some()
            })
            || r.local.kind != Evidence::Known(FileKind::Regular)
        {
            return Err(filesystem::stale());
        }
        let path = r.local.path.as_deref().ok_or_else(filesystem::stale)?;
        if self
            .filesystem
            .read(path)?
            .metadata()
            .map_err(filesystem::error)?
            .nlink()
            != 1
        {
            return Err(filesystem::stale());
        }
        self.filesystem.check(&r.local)?;
        let entries = activity::measured(Phase::VerifyLocalScope, State::Running, || {
            verify_local_scope(
                self.config.local_root.clone(),
                self.config.sync_root_id.clone(),
                LocalVerification::ExpandLimit,
            )
            .map_err(|_| filesystem::stale())
        })?;
        if entries
            .values()
            .any(|e| e.identity == stored.identity.local_identity)
            || entries
                .values()
                .filter(|e| e.identity == r.local.identity)
                .count()
                != 1
        {
            return Err(filesystem::stale());
        }
        Ok(())
    }
    /// A changed path requires a unique persisted incarnation, complete coverage and no hardlink alias.
    fn validate_relocation(
        &self,
        r: &ExecuteOperationRequest,
        stored: &InventorySnapshot,
    ) -> Result<(), OperatorError> {
        use crate::detectors::{verify_local_scope, LocalVerification};
        use std::os::unix::fs::MetadataExt;
        if !matches!(&r.local.identity, Evidence::Known(i) if i.birth_ns.is_some())
            || r.local.identity != stored.identity.local_identity
        {
            return Err(filesystem::stale());
        }
        self.filesystem.check(&r.local)?;
        let path = r.local.path.as_deref().ok_or_else(filesystem::stale)?;
        if r.local.kind == Evidence::Known(FileKind::Regular)
            && self
                .filesystem
                .read(path)?
                .metadata()
                .map_err(filesystem::error)?
                .nlink()
                != 1
        {
            return Err(filesystem::stale());
        }
        let entries = activity::measured(Phase::VerifyLocalScope, State::Running, || {
            verify_local_scope(
                self.config.local_root.clone(),
                self.config.sync_root_id.clone(),
                LocalVerification::ExpandLimit,
            )
            .map_err(|_| filesystem::stale())
        })?;
        if entries
            .values()
            .filter(|e| e.identity == r.local.identity)
            .count()
            != 1
            || stored
                .local_path
                .as_ref()
                .is_some_and(|old| entries.contains_key(old))
        {
            return Err(filesystem::stale());
        }
        Ok(())
    }
    fn validate(
        &self,
        r: &ExecuteOperationRequest,
        context: Option<&ActivityContext>,
    ) -> Result<(), OperatorError> {
        let op = &r.operation;
        self.filesystem.validate_root()?;
        if r.identity.account_id != self.config.account_id
            || r.identity.sync_root_id != self.config.sync_root_id
            || r.identity.item_id.as_ref() != Some(&op.item_ref)
            || r.execution_id.0.is_empty()
            || op.step == 0
            || op.remote_id != r.identity.remote_id
            || r.remote.id != op.remote_id
            || op
                .remote_id
                .as_ref()
                .is_some_and(|id| id == &self.config.remote_root_id)
            || op
                .depends_on
                .iter()
                .any(|s| *s >= op.step || !r.completed_steps.contains(s))
        {
            return Err(filesystem::invalid());
        }
        if r.local.state == FileState::Present
            && (!matches!(
                r.local.kind,
                Evidence::Known(FileKind::Regular | FileKind::Directory)
            ) || !matches!(&r.local.name,Evidence::Known(name) if crate::projections::valid_name(name)))
        {
            return Err(OperatorError::new(OperatorErrorKind::UnsupportedOperation));
        }
        if r.remote.state == FileState::Present
            && !matches!(&r.remote.name,Evidence::Known(name) if crate::projections::valid_name(name))
        {
            return Err(OperatorError::new(OperatorErrorKind::UnsupportedOperation));
        }
        if matches!(
            op.action,
            OperationAction::TrashRemote | OperationAction::RemoveLocal
        ) && !self.config.trash_propagation_authorized
        {
            return Err(filesystem::stale());
        }
        let pre = op.preconditions.as_ref().ok_or_else(filesystem::stale)?;
        if !matches!(
            pre.local_unchanged,
            Evidence::Known(true) | Evidence::NotApplicable
        ) || !matches!(
            pre.remote_unchanged,
            Evidence::Known(true) | Evidence::NotApplicable
        ) || !matches!(
            pre.destination_free,
            Evidence::Known(true) | Evidence::NotApplicable
        ) {
            return Err(filesystem::stale());
        }
        if let Some(version) = pre.observed_remote_version {
            if op.remote_id.is_some() && r.remote.version != Evidence::Known(version) {
                return Err(filesystem::stale());
            }
        }
        if pre.local_unchanged == Evidence::Known(true)
            && !matches!(r.local.state, FileState::Present | FileState::Absent)
        {
            return Err(filesystem::stale());
        }
        if let Some(path) = &op.local_path {
            self.filesystem.entry(path)?;
        }
        if r.local.state == FileState::Present && op.local_path != r.local.path {
            return Err(filesystem::invalid());
        }
        if let Evidence::Known(mime) = &r.remote.mime_type {
            match crate::projections::kind(mime) {
                crate::projections::RemoteKind::Workspace => {
                    if !matches!(
                        op.action,
                        OperationAction::RegisterIdentity
                            | OperationAction::CreateLocalShortcut
                            | OperationAction::RenameLocal
                            | OperationAction::MoveLocal
                            | OperationAction::ConfirmBaseline
                    ) {
                        return Err(filesystem::stale());
                    }
                    if r.local.state == FileState::Present
                        && !r
                            .remote
                            .id
                            .as_ref()
                            .is_some_and(|id| crate::projections::content_matches(&r.local, id))
                    {
                        return Err(filesystem::stale());
                    }
                }
                crate::projections::RemoteKind::RemoteShortcut
                | crate::projections::RemoteKind::Unsupported => {
                    return Err(OperatorError::new(OperatorErrorKind::UnsupportedOperation))
                }
                _ => {}
            }
        }
        // A second unresolved execution must be recovered before any new mutation.
        let lookup = self.inventory.lookup_with_context(
            InventoryLookup {
                account_id: r.identity.account_id.clone(),
                sync_root_id: r.identity.sync_root_id.clone(),
                item_id: r.identity.item_id.clone(),
                remote_id: None,
                local_identity: Evidence::NotCollected,
                local_path: None,
            },
            context,
        );
        match lookup.status {
            InventoryLookupStatus::Found => {
                let snapshot = lookup.snapshot.ok_or_else(filesystem::stale)?;
                if matches!(
                    op.action,
                    OperationAction::TrashRemote | OperationAction::RemoveLocal
                ) {
                    self.validate_trash(r, &snapshot, context)?;
                }
                if crate::projections::workspace(&r.remote)
                    && r.local.state == FileState::Present
                    && snapshot.baseline.projection != Some(ProjectionKind::GoogleWorkspaceShortcut)
                {
                    return Err(filesystem::stale());
                }
                if snapshot.baseline.projection == Some(ProjectionKind::GoogleWorkspaceShortcut)
                    && !crate::projections::workspace(&r.remote)
                {
                    return Err(filesystem::stale());
                }
                self.validate_consent(r, &snapshot)?;
                if r.local.state == FileState::Present && snapshot.local_path != r.local.path {
                    self.validate_relocation(r, &snapshot)?;
                }
                if matches!(r.local.identity, Evidence::Known(_))
                    && snapshot.identity.local_identity != r.local.identity
                {
                    self.validate_replacement(r, &snapshot)?;
                }
                if (snapshot.identity.remote_id != r.identity.remote_id
                    && !(op.action == OperationAction::Adopt
                        && snapshot.identity.remote_id.is_none()))
                    || (op.action == OperationAction::ConfirmBaseline
                        && snapshot.baseline.state != BaselineState::Synced
                        && op.conflict_id.is_none())
                    || (op.action == OperationAction::Adopt
                        && matches!(snapshot.identity.local_identity, Evidence::Known(_))
                        && snapshot.identity.local_identity != r.identity.local_identity)
                    || r.expected_revision != Some(snapshot.revision)
                    || snapshot.pending_operations.iter().any(|p| {
                        p.id == r.execution_id
                            || matches!(
                                p.state,
                                OperationState::Pending
                                    | OperationState::Running
                                    | OperationState::AppliedUnconfirmed
                            )
                    })
                {
                    return Err(filesystem::stale());
                }
            }
            InventoryLookupStatus::NotFound if r.expected_revision == Some(0) => {
                if matches!(
                    op.action,
                    OperationAction::TrashRemote | OperationAction::RemoveLocal
                ) {
                    return Err(filesystem::stale());
                }
                if matches!(
                    op.action,
                    OperationAction::RegisterIdentity | OperationAction::Adopt
                ) {
                    let existing = self.inventory.lookup_with_context(
                        InventoryLookup {
                            account_id: r.identity.account_id.clone(),
                            sync_root_id: r.identity.sync_root_id.clone(),
                            item_id: None,
                            remote_id: r.identity.remote_id.clone(),
                            local_path: if r.local.state == FileState::Present {
                                r.local.path.clone()
                            } else {
                                None
                            },
                            local_identity: if r.local.state == FileState::Present {
                                r.local.identity.clone()
                            } else {
                                Evidence::NotCollected
                            },
                        },
                        context,
                    );
                    if existing.status != InventoryLookupStatus::NotFound {
                        return Err(filesystem::stale());
                    }
                }
            }
            _ => return Err(filesystem::stale()),
        }
        Ok(())
    }
}
impl Operator for PersistingOperator {
    fn record_conflict<'a>(
        &'a self,
        file: &'a crate::detectors::File,
        reason: String,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), OperatorError>> + Send + 'a>>
    {
        Box::pin(async move {
            let mut span = if file.identity.account_id == self.config.account_id
                && file.identity.sync_root_id == self.config.sync_root_id
            {
                Span::execution(
                    &self.config,
                    Kind::RecordConflict,
                    None,
                    file.identity.item_id.as_ref(),
                    Phase::Receive,
                )
            } else {
                Span::default()
            };
            let result = span
                .within(async {
                    span.phase(Phase::Acquire, State::Waiting(Wait::OperatorGate));
                    let _guard = self.gate.lock().await;
                    span.phase(Phase::RecordConflict, State::Running);
                    self.record_conflict(file, reason)
                })
                .await;
            span.result(&result);
            result
        })
    }
    fn recover_with<'a>(
        &'a self,
        checking: interfaces::RecoveryObserver<'a>,
    ) -> interfaces::RecoveryFuture<'a> {
        self.recover_future(checking)
    }
    fn execute<'a>(&'a self, r: ExecuteOperationRequest) -> OperationFuture<'a> {
        Box::pin(async move {
            let same_scope = r.identity.account_id == self.config.account_id
                && r.identity.sync_root_id == self.config.sync_root_id
                && r.identity.item_id.as_ref() == Some(&r.operation.item_ref);
            let mut span = if same_scope {
                Span::execution(
                    &self.config,
                    Kind::Execute(r.operation.action),
                    Some(&r.execution_id),
                    Some(&r.operation.item_ref),
                    Phase::Receive,
                )
            } else {
                Span::default()
            };
            let result = span.within(self.execute_observed(r, &span)).await;
            span.finish(match result.status {
                OperationResultStatus::Succeeded => Outcome::Complete,
                OperationResultStatus::Failed => Outcome::Failed,
                OperationResultStatus::Unconfirmed => Outcome::Unconfirmed,
                OperationResultStatus::Skipped => Outcome::Skipped,
            });
            result
        })
    }
}

impl PersistingOperator {
    async fn execute_observed(&self, r: ExecuteOperationRequest, span: &Span) -> OperationResult {
        span.phase(Phase::Acquire, State::Waiting(Wait::OperatorGate));
        let _guard = self.gate.lock().await;
        let mut result = OperationResult {
            operation_id: r.execution_id.clone(),
            item_id: r.operation.item_ref.clone(),
            action: r.operation.action,
            status: OperationResultStatus::Skipped,
            effect: None,
            completed_at: None,
            error: None,
            inventory_write: None,
        };
        span.phase(Phase::Acquire, State::Waiting(Wait::Inventory));
        let inventory_context = span.activity_context();
        let _lease = match self
            .inventory
            .execution_lease_with_context(inventory_context.as_ref())
        {
            Ok(lease) => lease,
            Err(_) => {
                result.error = Some(OperatorError::new(OperatorErrorKind::Unavailable).failure());
                return result;
            }
        };
        span.phase(Phase::Validate, State::Running);
        let inventory_context = span.activity_context();
        if let Err(error) = self.validate(&r, inventory_context.as_ref()) {
            result.error = Some(error.failure());
            return result;
        }
        if matches!(
            r.operation.action,
            OperationAction::RegisterIdentity
                | OperationAction::Adopt
                | OperationAction::ConfirmBaseline
        ) {
            span.phase(Phase::VerifyEffect, State::Running);
            return self.metadata(&r, result, span).await;
        }
        span.phase(Phase::Prepare, State::Running);
        let record = match self
            .prepare_with_context(&r, inventory_context.as_ref())
            .await
        {
            Ok(record) => record,
            Err(error) => {
                result.error = Some(error.failure());
                return result;
            }
        };
        let mut pending = PendingOperation {
            id: r.execution_id.clone(),
            item_id: r.operation.item_ref.clone(),
            action: r.operation.action,
            state: OperationState::Pending,
            remote_id: record
                .reserved_remote_id
                .clone()
                .or_else(|| r.operation.remote_id.clone()),
            applied_effect: None,
            idempotency_key: Some(r.execution_id.0.clone()),
            recovery: Some(Box::new(record.clone())),
            recovery_diagnostic: None,
            created_at: now(),
            updated_at: now(),
        };
        span.phase(Phase::RecordIntent, State::Running);
        let receipt = self.write(
            &r,
            r.expected_revision,
            PersistedEffect::Pending(pending.clone()),
        );
        result.inventory_write = Some(receipt.clone());
        if receipt.status != WriteStatus::Written {
            result.error = Some(OperatorError::new(OperatorErrorKind::Unavailable).failure());
            return result;
        }
        pending.state = OperationState::Running;
        let receipt = self.write(&r, receipt.revision, PersistedEffect::Pending(pending));
        result.inventory_write = Some(receipt.clone());
        if receipt.status != WriteStatus::Written {
            result.error = Some(OperatorError::new(OperatorErrorKind::Unavailable).failure());
            return result;
        }
        let journal = Journal::new(
            self.inventory.clone(),
            r.clone(),
            receipt.revision.unwrap(),
            record,
        );
        span.phase(Phase::Dispatch, State::Running);
        span.effect(Effect::Uncertain, false);
        let effect = self
            .remote
            .execute(&self.config, &r, &self.filesystem, Some(&journal))
            .await;
        let (persisted, status, observed) = match effect {
            Ok(effect) => {
                result.effect = Some(effect.clone());
                (
                    PersistedEffect::OperationApplied(effect.clone()),
                    if effect.verified {
                        OperationResultStatus::Succeeded
                    } else {
                        OperationResultStatus::Unconfirmed
                    },
                    if effect.verified {
                        Effect::Verified
                    } else {
                        Effect::Applied
                    },
                )
            }
            Err(error) => {
                result.error = Some(error.failure());
                if error.effect == ExecutionEffect::NotApplied {
                    (
                        PersistedEffect::OperationFailed,
                        OperationResultStatus::Failed,
                        Effect::NotApplied,
                    )
                } else {
                    let effect = AppliedEffect {
                        local: None,
                        remote: None,
                        remote_id: r.operation.remote_id.clone(),
                        verified: false,
                    };
                    result.effect = Some(effect.clone());
                    (
                        PersistedEffect::OperationApplied(effect),
                        OperationResultStatus::Unconfirmed,
                        Effect::Uncertain,
                    )
                }
            }
        };
        span.effect(observed, false);
        let mut recorded = r.clone();
        if let Some(effect) = result.effect.as_ref().filter(|effect| effect.verified) {
            if let Some(local) = &effect.local {
                if matches!(local.identity, Evidence::Known(_)) {
                    recorded.identity.local_identity = local.identity.clone();
                }
            }
        }
        // Replacement can change the inode; subsequent confirmation must not restore the
        // pre-execution identity over the verified effect's identity.
        span.phase(Phase::PersistEffect, State::Running);
        let mut receipt = self.write(&recorded, journal.revision(), persisted);
        span.effect(
            observed,
            observed != Effect::NotApplied && receipt.status == WriteStatus::Written,
        );
        if receipt.status == WriteStatus::Written && status == OperationResultStatus::Succeeded {
            span.phase(Phase::Confirm, State::Running);
            let inventory_context = span.activity_context();
            let confirmed = match result
                .effect
                .as_ref()
                .and_then(|e| self.convergence_with_context(e, inventory_context.as_ref()))
            {
                Some((baseline, parent)) => PersistedEffect::BaselineConfirmed { baseline, parent },
                None => PersistedEffect::OperationConfirmed,
            };
            receipt = self.write(&recorded, receipt.revision, confirmed);
        }
        result.status = if receipt.status == WriteStatus::Written {
            status
        } else {
            OperationResultStatus::Unconfirmed
        };
        if receipt.status != WriteStatus::Written {
            result.error = Some(
                OperatorError::new(OperatorErrorKind::Unavailable)
                    .uncertain()
                    .failure(),
            );
        }
        span.receipt(receipt.status);
        result.inventory_write = Some(receipt);
        result.completed_at = Some(now());
        result
    }
}
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(super) async fn test_response(
    request: &ExecuteOperationRequest,
    work: impl std::future::Future<Output = ()>,
) {
    activity::test_response(request, work).await;
}
