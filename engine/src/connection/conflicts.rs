//! Public conflict review and consent, scoped to the selected account and configured root.
use super::*;
use crate::{
    conflicts::*,
    inventory::{models::*, Inventory, InventoryConfig},
    models::Id,
};
impl Engine {
    /// Reads durable conflicts after the selected scope has been configured, including after restart.
    pub async fn list_conflicts(
        &self,
        account_id: crate::AccountId,
    ) -> Result<Vec<ConflictSummary>, ConflictError> {
        let inner = self.inner.clone();
        self.runtime()
            .spawn(async move {
                let sessions = inner.synchronizations.lock().await;
                let session = sessions.get(&account_id).ok_or(ConflictError::NotStarted)?;
                let config = session.orchestrator.config();
                let root = config.roots.local_root.clone();
                let inventory = Inventory::open(InventoryConfig {
                    directory: inner.synchronization_directory.clone(),
                    account_id: account_id.clone(),
                })
                .map_err(|_| ConflictError::Unavailable)?;
                let snapshots = inventory
                    .enumerate(InventoryScopeQuery {
                        account_id,
                        sync_root_id: config.sync_root_id.clone(),
                    })
                    .map_err(|_| ConflictError::Unavailable)?;
                Ok(snapshots
                    .into_iter()
                    .filter(|s| {
                        !s.local_path
                            .as_deref()
                            .is_some_and(crate::projections::libreoffice_lock_path)
                    })
                    .filter_map(|s| {
                        let c = s.prior_conflict.filter(|c| !c.resolved)?;
                        let archive_path = root.join(".drivesync/conflicts").join(&c.id.0);
                        let can_resolve = crate::conflicts::resolvable(&c.local, &c.remote);
                        Some(ConflictSummary {
                            id: c.id,
                            item_id: c.item_id,
                            revision: s.revision,
                            local_path: s.local_path,
                            reason: c.reason,
                            local: ConflictVersion {
                                size: observed(c.local.size),
                                checksum: observed(c.local.checksum),
                                modified_at: observed(c.local.modified_at),
                                provider_version: None,
                            },
                            remote: ConflictVersion {
                                size: observed(c.remote.size),
                                checksum: observed(c.remote.checksum),
                                modified_at: observed(c.remote.modified_time),
                                provider_version: observed(c.remote.version),
                            },
                            archive_path,
                            can_resolve,
                            resolution_pending: c.resolution.is_some(),
                        })
                    })
                    .collect())
            })
            .await
            .map_err(|_| ConflictError::Unavailable)?
    }
    /// Checks the exact conflict identity, including records retired by verified convergence.
    pub async fn conflict_resolution_status(
        &self,
        account_id: crate::AccountId,
        conflict_id: Id,
    ) -> Result<ConflictResolutionStatus, ConflictError> {
        let inner = self.inner.clone();
        self.runtime()
            .spawn(async move {
                let sessions = inner.synchronizations.lock().await;
                let session = sessions.get(&account_id).ok_or(ConflictError::NotStarted)?;
                let inventory = Inventory::open(InventoryConfig {
                    directory: inner.synchronization_directory.clone(),
                    account_id: account_id.clone(),
                })
                .map_err(|_| ConflictError::Unavailable)?;
                let rows = inventory
                    .enumerate(InventoryScopeQuery {
                        account_id,
                        sync_root_id: session.orchestrator.config().sync_root_id.clone(),
                    })
                    .map_err(|_| ConflictError::Unavailable)?;
                for row in rows {
                    let Some(conflict) = row.prior_conflict.filter(|c| c.id == conflict_id) else {
                        continue;
                    };
                    return Ok(if conflict.resolved {
                        ConflictResolutionStatus::Resolved
                    } else if conflict.resolution.is_none() {
                        ConflictResolutionStatus::Stale
                    } else if row.pending_operations.iter().any(|operation| {
                        operation.state == OperationState::Failed
                            && operation.recovery.as_ref().is_some_and(|record| {
                                record.command.conflict_id.as_ref() == Some(&conflict_id)
                            })
                    }) {
                        ConflictResolutionStatus::Failed
                    } else {
                        ConflictResolutionStatus::Applying
                    });
                }
                Ok(ConflictResolutionStatus::Stale)
            })
            .await
            .map_err(|_| ConflictError::Unavailable)?
    }

    /// Records one explicit choice for a reviewed revision. Actual I/O rechecks both replicas.
    pub async fn resolve_conflict(
        &self,
        request: ResolveConflictRequest,
    ) -> Result<(), ConflictError> {
        let inner = self.inner.clone();
        self.runtime()
            .spawn(async move {
                let sessions = inner.synchronizations.lock().await;
                let session = sessions
                    .get(&request.account_id)
                    .filter(|session| !session.orchestrator.is_finished())
                    .ok_or(ConflictError::NotStarted)?;
                let inventory = Inventory::open(InventoryConfig {
                    directory: inner.synchronization_directory.clone(),
                    account_id: request.account_id.clone(),
                })
                .map_err(|_| ConflictError::Unavailable)?;
                let _lease = inventory
                    .execution_lease()
                    .map_err(|_| ConflictError::Unavailable)?;
                let rows = inventory
                    .enumerate(InventoryScopeQuery {
                        account_id: request.account_id.clone(),
                        sync_root_id: session.orchestrator.config().sync_root_id.clone(),
                    })
                    .map_err(|_| ConflictError::Unavailable)?;
                let row = rows
                    .into_iter()
                    .find(|s| {
                        s.prior_conflict
                            .as_ref()
                            .is_some_and(|c| c.id == request.conflict_id && !c.resolved)
                    })
                    .ok_or(ConflictError::NotFound)?;
                if row.revision != request.expected_revision
                    || row.pending_operations.iter().any(|p| {
                        matches!(
                            p.state,
                            OperationState::Pending
                                | OperationState::Running
                                | OperationState::AppliedUnconfirmed
                        )
                    })
                {
                    return Err(ConflictError::Stale);
                }
                let mut conflict = row.prior_conflict.unwrap();
                if conflict.resolution.is_some() {
                    return Err(ConflictError::Stale);
                }
                if !crate::conflicts::resolvable(&conflict.local, &conflict.remote) {
                    return Err(ConflictError::Unsupported);
                }
                conflict.resolution = Some(Box::new(ConflictResolution {
                    choice: request.choice,
                    expected_local: conflict.local.clone(),
                    expected_remote: conflict.remote.clone(),
                }));
                conflict.updated_at = crate::orchestrator::timestamp();
                let result = inventory.write(InventoryWrite {
                    operation_id: Id(format!("choice-{}", conflict.id.0)),
                    item_id: conflict.item_id.clone(),
                    identity: row.identity,
                    expected_revision: Some(request.expected_revision),
                    effect: PersistedEffect::ConflictUpdated(Some(conflict)),
                    recorded_at: crate::orchestrator::timestamp(),
                });
                if result.status != WriteStatus::Written {
                    return Err(ConflictError::Stale);
                }
                session.orchestrator.request_refresh();
                Ok(())
            })
            .await
            .map_err(|_| ConflictError::Unavailable)?
    }
}
