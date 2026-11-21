//! Execution with fixed scope, optimistic persistence, and explicit uncertain effects.
mod filesystem;
mod google;
mod interfaces;
pub(super) mod models;
use crate::{
    accounts::{Accounts, GetAccountRequest},
    inventory::{models::*, Inventory},
    models::*,
};
use filesystem::Filesystem;
#[cfg(test)]
use interfaces::EffectFuture;
use interfaces::RemoteExecutor;
#[allow(unused_imports)]
pub(super) use interfaces::{OperationFuture, Operator, OperatorFactory};
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
    /// Only equal, fully resolved replicas may replace a synchronized baseline.
    fn baseline(&self, effect: &AppliedEffect) -> Option<Baseline> {
        if !effect.verified {
            return None;
        }
        let local = effect.local.as_ref()?;
        let remote = effect.remote.as_ref()?;
        let absent = local.state == FileState::Absent && remote.trashed == Evidence::Known(true);
        let same_name =
            matches!((&local.name,&remote.name),(Evidence::Known(a),Evidence::Known(b)) if a==b);
        let root_parent = local.parent == Evidence::Known(Path::new())
            && remote.parents == Evidence::Known(vec![self.config.remote_root_id.clone()]);
        let content = match (&local.kind, &remote.mime_type) {
            (Evidence::Known(FileKind::Directory), Evidence::Known(mime))
                if mime == "application/vnd.google-apps.folder" =>
            {
                true
            }
            (Evidence::Known(FileKind::Regular), Evidence::Known(mime))
                if !mime.starts_with("application/vnd.google-apps.") =>
            {
                matches!((&local.checksum,&remote.checksum),(Evidence::Known(a),Evidence::Known(b)) if a==b)
            }
            _ => false,
        };
        if !absent
            && !(local.state == FileState::Present
                && remote.state == FileState::Present
                && remote.trashed == Evidence::Known(false)
                && same_name
                && root_parent
                && content)
        {
            return None;
        }
        Some(Baseline {
            state: BaselineState::Synced,
            local: Some(local.clone()),
            remote: Some(remote.clone()),
            confirmed_at: Some(now()),
            projection: Some(ProjectionKind::Native),
        })
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
                if snapshot.identity.remote_id != r.identity.remote_id
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
            InventoryLookupStatus::NotFound if r.expected_revision == Some(0) => {}
            _ => return Err(filesystem::stale()),
        }
        Ok(())
    }
}
impl Operator for PersistingOperator {
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
            if let Err(error) = self.validate(&r) {
                result.error = Some(error.failure());
                return result;
            }
            let mut pending = PendingOperation {
                id: r.execution_id.clone(),
                item_id: r.operation.item_ref.clone(),
                action: r.operation.action,
                state: OperationState::Pending,
                remote_id: r.operation.remote_id.clone(),
                applied_effect: None,
                idempotency_key: Some(r.execution_id.0.clone()),
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
            let effect = self
                .remote
                .execute(&self.config, &r, &self.filesystem)
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
            let mut receipt = self.write(&r, receipt.revision, persisted);
            if receipt.status == WriteStatus::Written && status == OperationResultStatus::Succeeded
            {
                let confirmed = match result.effect.as_ref().and_then(|e| self.baseline(e)) {
                    Some(baseline) => PersistedEffect::BaselineConfirmed(baseline),
                    None => PersistedEffect::OperationConfirmed,
                };
                receipt = self.write(&r, receipt.revision, confirmed);
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
