//! Execution with fixed scope, optimistic persistence, and explicit uncertain effects.
mod filesystem;
mod google;
mod interfaces;
mod journal;
pub(super) mod models;
mod recovery;
use crate::{
    accounts::{Accounts, GetAccountRequest},
    inventory::{models::*, Inventory},
    models::*,
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
        self.inventory.write(InventoryWrite {
            operation_id: request.execution_id.clone(),
            item_id: request.operation.item_ref.clone(),
            identity: request.identity.clone(),
            expected_revision: revision,
            effect,
            recorded_at: now(),
        })
    }
    /// Only corresponding, equivalent current replicas may replace a synchronization baseline.
    fn convergence(&self, effect: &AppliedEffect) -> Option<(Baseline, Option<ParentAssociation>)> {
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
        let parent = self.confirmed_parent(local_parent, remote_parents)?;
        Some((
            Baseline {
                state: BaselineState::Synced,
                local: Some(local.clone()),
                remote: Some(remote.clone()),
                confirmed_at: Some(now()),
                projection: Some(ProjectionKind::Native),
            },
            parent,
        ))
    }
    /// Initial child effects require an existing confirmed parent association before dispatch.
    fn confirmed_parent(
        &self,
        local_parent: &std::path::Path,
        remote_parents: &[RemoteId],
    ) -> Option<Option<ParentAssociation>> {
        if remote_parents.len() != 1 {
            return None;
        }
        let parent = if local_parent.as_os_str().is_empty()
            && remote_parents[0] == self.config.remote_root_id
        {
            None
        } else {
            let lookup = self.inventory.lookup(InventoryLookup {
                account_id: self.config.account_id.clone(),
                sync_root_id: self.config.sync_root_id.clone(),
                item_id: None,
                remote_id: Some(remote_parents[0].clone()),
                local_identity: Evidence::NotCollected,
                local_path: Some(local_parent.to_path_buf()),
            });
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
            let Some((baseline, parent)) = self.convergence(&effect) else {
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
        let receipt = self.write(&current, r.expected_revision, persisted);
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
        use crate::detectors::observation::{
            remote::interfaces::CancellationToken, scanner::Scanner, ObservationCoverage,
        };
        use std::os::unix::fs::MetadataExt;
        if !matches!(
            r.operation.action,
            OperationAction::UpdateRemoteContent | OperationAction::ConfirmBaseline
        ) || stored.baseline.state != BaselineState::Synced
            || stored.local_path != r.local.path
            || !stored.baseline.local.as_ref().is_some_and(|old| {
                old.path == r.local.path && old.kind == Evidence::Known(FileKind::Regular)
            })
            || !stored
                .baseline
                .remote
                .as_ref()
                .is_some_and(|old| r.remote.unchanged_binary(old))
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
        let mut scanner = Scanner::open(
            self.config.local_root.clone(),
            self.config.sync_root_id.clone(),
        )
        .map_err(|_| filesystem::stale())?;
        let scan = loop {
            let scan = scanner
                .scan(&CancellationToken::new())
                .map_err(|_| filesystem::stale())?;
            if scan.coverage == ObservationCoverage::Complete {
                break scan;
            }
            if !scan.issues.iter().any(|i| {
                i.kind == crate::detectors::observation::scanner::ScanIssueKind::LimitReached
            }) || !scanner.expand_limit()
            {
                return Err(filesystem::stale());
            }
        };
        if scan.coverage != ObservationCoverage::Complete
            || scan
                .entries
                .values()
                .any(|e| e.identity == stored.identity.local_identity)
            || scan
                .entries
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
        use crate::detectors::observation::{
            remote::interfaces::CancellationToken, scanner::Scanner, ObservationCoverage,
        };
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
        let mut scanner = Scanner::open(
            self.config.local_root.clone(),
            self.config.sync_root_id.clone(),
        )
        .map_err(|_| filesystem::stale())?;
        let scan = loop {
            let scan = scanner
                .scan(&CancellationToken::new())
                .map_err(|_| filesystem::stale())?;
            if scan.coverage == ObservationCoverage::Complete {
                break scan;
            }
            if !scan.issues.iter().any(|i| {
                i.kind == crate::detectors::observation::scanner::ScanIssueKind::LimitReached
            }) || !scanner.expand_limit()
            {
                return Err(filesystem::stale());
            }
        };
        if scan
            .entries
            .values()
            .filter(|e| e.identity == r.local.identity)
            .count()
            != 1
            || stored
                .local_path
                .as_ref()
                .is_some_and(|old| scan.entries.contains_key(old))
        {
            return Err(filesystem::stale());
        }
        Ok(())
    }
    fn validate(&self, r: &ExecuteOperationRequest) -> Result<(), OperatorError> {
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
        // A second unresolved execution must be recovered before any new mutation.
        let lookup = self.inventory.lookup(InventoryLookup {
            account_id: r.identity.account_id.clone(),
            sync_root_id: r.identity.sync_root_id.clone(),
            item_id: r.identity.item_id.clone(),
            remote_id: None,
            local_identity: Evidence::NotCollected,
            local_path: None,
        });
        match lookup.status {
            InventoryLookupStatus::Found => {
                let snapshot = lookup.snapshot.ok_or_else(filesystem::stale)?;
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
                        && snapshot.baseline.state != BaselineState::Synced)
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
                    OperationAction::RegisterIdentity | OperationAction::Adopt
                ) {
                    let existing = self.inventory.lookup(InventoryLookup {
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
                    });
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
    fn recover<'a>(
        &'a self,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<Vec<RecoveryResult>, OperatorError>>
                + Send
                + 'a,
        >,
    > {
        self.recover_future()
    }
    fn execute<'a>(&'a self, r: ExecuteOperationRequest) -> OperationFuture<'a> {
        Box::pin(async move {
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
            let _lease = match self.inventory.execution_lease() {
                Ok(lease) => lease,
                Err(_) => {
                    result.error =
                        Some(OperatorError::new(OperatorErrorKind::Unavailable).failure());
                    return result;
                }
            };
            if let Err(error) = self.validate(&r) {
                result.error = Some(error.failure());
                return result;
            }
            if matches!(
                r.operation.action,
                OperationAction::RegisterIdentity
                    | OperationAction::Adopt
                    | OperationAction::ConfirmBaseline
            ) {
                return self.metadata(&r, result).await;
            }
            let record = match self.prepare(&r).await {
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
            let effect = self
                .remote
                .execute(&self.config, &r, &self.filesystem, Some(&journal))
                .await;
            let (persisted, status) = match effect {
                Ok(effect) => {
                    result.effect = Some(effect.clone());
                    (
                        PersistedEffect::OperationApplied(effect.clone()),
                        if effect.verified {
                            OperationResultStatus::Succeeded
                        } else {
                            OperationResultStatus::Unconfirmed
                        },
                    )
                }
                Err(error) => {
                    result.error = Some(error.failure());
                    if error.effect == ExecutionEffect::NotApplied {
                        (
                            PersistedEffect::OperationFailed,
                            OperationResultStatus::Failed,
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
                        )
                    }
                }
            };
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
            let mut receipt = self.write(&recorded, journal.revision(), persisted);
            if receipt.status == WriteStatus::Written && status == OperationResultStatus::Succeeded
            {
                let confirmed = match result.effect.as_ref().and_then(|e| self.convergence(e)) {
                    Some((baseline, parent)) => {
                        PersistedEffect::BaselineConfirmed { baseline, parent }
                    }
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
            result.inventory_write = Some(receipt);
            result.completed_at = Some(now());
            result
        })
    }
}
#[cfg(test)]
mod tests;
