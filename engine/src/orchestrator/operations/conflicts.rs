//! Durable conflict facts and dispatch authorization. Choices never bypass ordinary preconditions.
use super::*;
use crate::{conflicts, detectors::File};
impl PersistingOperator {
    pub(super) fn record_conflict(&self, file: &File, reason: String) -> Result<(), OperatorError> {
        if file.identity.account_id != self.config.account_id
            || file.identity.sync_root_id != self.config.sync_root_id
        {
            return Err(filesystem::invalid());
        }
        use crate::detectors::ObservationCoverage;
        if file.observations.local_coverage != ObservationCoverage::Complete
            || file.observations.remote_coverage != ObservationCoverage::Complete
            || !file.environment.local_root_available
            || !file.environment.remote_available
        {
            return Err(filesystem::stale());
        }
        if file.identity.item_id.is_some()
            && (matches!(
                file.local.state,
                FileState::Unknown | FileState::Unavailable
            ) || matches!(
                file.remote.state,
                FileState::Unknown | FileState::Unavailable
            ))
        {
            return if file.prior_conflict.is_some() {
                Ok(())
            } else {
                Err(filesystem::stale())
            };
        }
        let _lease = activity::measured(Phase::Acquire, State::Waiting(Wait::Inventory), || {
            self.inventory
                .execution_lease()
                .map_err(|_| filesystem::stale())
        })?;
        let mut identity = file.identity.clone();
        let mut local = file.local.clone();
        let mut remote = file.remote.clone();
        if identity.item_id.is_none() {
            if file.association_candidates.len() != 1 {
                return Err(filesystem::stale());
            }
            local = file.association_candidates[0].local.clone();
            remote = file.association_candidates[0].remote.clone();
            identity.item_id = file.registration_id.clone();
            identity.local_identity = local.identity.clone();
            identity.remote_id = remote.id.clone();
        }
        let item = identity.item_id.clone().ok_or_else(filesystem::stale)?;
        if file.prior_conflict.as_ref().is_some_and(|c| {
            !c.resolved
                && conflicts::local_matches(&c.local, &local)
                && conflicts::remote_matches(&c.remote, &remote)
        }) {
            return Ok(());
        }
        if file.pending_operations.iter().any(|p| {
            matches!(
                p.state,
                OperationState::Pending
                    | OperationState::Running
                    | OperationState::AppliedUnconfirmed
            )
        }) {
            return Err(filesystem::stale());
        }
        let id = crate::orchestrator::execution_id().map_err(|_| filesystem::stale())?;
        let result = activity::persist(Phase::RecordConflict, || {
            self.inventory.write(InventoryWrite {
                operation_id: id.clone(),
                item_id: item.clone(),
                identity,
                expected_revision: Some(file.inventory_revision.unwrap_or(0)),
                recorded_at: now(),
                effect: PersistedEffect::ConflictUpdated(Some(ConflictState {
                    id,
                    item_id: item,
                    reason,
                    resolved: false,
                    resolution: None,
                    local,
                    remote,
                    observed_revision: file.inventory_revision.unwrap_or(0),
                    updated_at: now(),
                })),
            })
        });
        if result.status != WriteStatus::Written {
            return Err(filesystem::stale());
        }
        Ok(())
    }
    pub(super) fn validate_consent(
        &self,
        r: &ExecuteOperationRequest,
        snapshot: &InventorySnapshot,
    ) -> Result<(), OperatorError> {
        let Some(conflict) = snapshot.prior_conflict.as_ref().filter(|c| !c.resolved) else {
            return if r.operation.conflict_id.is_none() {
                Ok(())
            } else {
                Err(filesystem::stale())
            };
        };
        if r.operation.action == OperationAction::ConfirmBaseline
            && r.operation.conflict_id.is_none()
            && crate::orchestrator::evidence::equivalent(&r.local, &r.remote)
        {
            return Ok(());
        }
        let resolution = conflict.resolution.as_ref().ok_or_else(filesystem::stale)?;
        if r.operation.conflict_id.as_ref() != Some(&conflict.id)
            || !conflicts::local_matches(&resolution.expected_local, &r.local)
            || !conflicts::remote_matches(&resolution.expected_remote, &r.remote)
            || !conflicts::resolvable(&r.local, &r.remote)
        {
            return Err(filesystem::stale());
        }
        use crate::conflicts::ConflictChoice;
        let allowed = match resolution.choice {
            ConflictChoice::KeepLocal => matches!(
                r.operation.action,
                OperationAction::RenameRemote
                    | OperationAction::MoveRemote
                    | OperationAction::UpdateRemoteContent
                    | OperationAction::ConfirmBaseline
            ),
            ConflictChoice::KeepRemote => matches!(
                r.operation.action,
                OperationAction::RenameLocal
                    | OperationAction::MoveLocal
                    | OperationAction::UpdateLocalContent
                    | OperationAction::ConfirmBaseline
            ),
        };
        if matches!(
            r.operation.action,
            OperationAction::RenameLocal | OperationAction::RenameRemote
        ) {
            let desired = if resolution.choice == crate::conflicts::ConflictChoice::KeepLocal {
                &r.local.name
            } else {
                &r.remote.name
            };
            if !matches!(desired,Evidence::Known(name) if r.operation.name.as_ref()==Some(name)) {
                return Err(filesystem::stale());
            }
        }
        if !allowed {
            return Err(filesystem::stale());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{request, setup, Sandbox};
    use super::*;
    use crate::inventory::InventoryConfig;
    use sha2::Digest;
    #[test]
    fn durable_conflict_and_exact_choice_survive_reopen_and_reject_untagged_or_stale_dispatch() {
        let s = Sandbox::new();
        let executor = setup(&s);
        std::fs::write(executor.config.local_root.join("file"), b"local").unwrap();
        let mut r = request(&executor, OperationAction::UpdateRemoteContent);
        r.remote.mime_type = Evidence::Known("application/octet-stream".into());
        r.remote.size = Evidence::Known(6);
        r.remote.checksum = Evidence::Known(Checksum {
            algorithm: "sha256".into(),
            value: format!("{:x}", sha2::Sha256::digest(b"remote")),
        });
        let mut file = crate::orchestrator::analyzer::tests::fixture();
        file.identity = r.identity.clone();
        file.local = r.local.clone();
        file.remote = r.remote.clone();
        file.inventory_revision = Some(0);
        file.prior_conflict = None;
        executor.record_conflict(&file, "Content".into()).unwrap();
        let reopened = Inventory::open(InventoryConfig {
            directory: s.0.join("inventory"),
            account_id: r.identity.account_id.clone(),
        })
        .unwrap();
        let query = InventoryScopeQuery {
            account_id: r.identity.account_id.clone(),
            sync_root_id: r.identity.sync_root_id.clone(),
        };
        let row = reopened.enumerate(query.clone()).unwrap().remove(0);
        let mut conflict = row.prior_conflict.clone().unwrap();
        assert!(!conflict.resolved);
        assert_eq!(conflict.local, file.local);
        assert_eq!(conflict.remote, file.remote);
        assert!(executor.validate_consent(&r, &row).is_err());
        conflict.resolution = Some(Box::new(ConflictResolution {
            choice: crate::conflicts::ConflictChoice::KeepLocal,
            expected_local: r.local.clone(),
            expected_remote: r.remote.clone(),
        }));
        let id = conflict.id.clone();
        assert_eq!(
            reopened
                .write(InventoryWrite {
                    operation_id: Id("choice".into()),
                    item_id: r.operation.item_ref.clone(),
                    identity: r.identity.clone(),
                    expected_revision: Some(row.revision),
                    effect: PersistedEffect::ConflictUpdated(Some(conflict)),
                    recorded_at: now()
                })
                .status,
            WriteStatus::Written
        );
        let row = reopened.enumerate(query).unwrap().remove(0);
        assert!(executor.validate_consent(&r, &row).is_err());
        r.operation.conflict_id = Some(id);
        assert!(executor.validate_consent(&r, &row).is_ok());
        r.remote.version = Evidence::Known(99);
        assert!(executor.validate_consent(&r, &row).is_err());
    }
}

#[cfg(test)]
mod initial_tests {
    use super::super::tests::{request, setup, Sandbox};
    use super::*;
    #[test]
    fn unique_initial_collision_is_persisted_without_adoption_or_transfer() {
        let s = Sandbox::new();
        let executor = setup(&s);
        std::fs::write(executor.config.local_root.join("file"), b"local").unwrap();
        let r = request(&executor, OperationAction::UpdateRemoteContent);
        let mut file = crate::orchestrator::analyzer::tests::fixture();
        file.identity = r.identity.clone();
        file.identity.item_id = None;
        file.registration_id = Some(ItemId("initial-collision".into()));
        file.inventory_revision = None;
        file.inventory_lookup = InventoryLookupStatus::NotFound;
        file.baseline = Baseline::never_synced();
        file.local = r.local.clone();
        file.local.state = FileState::Unknown;
        file.remote = r.remote.clone();
        file.association_candidates = vec![crate::detectors::AssociationCandidate {
            local: r.local.clone(),
            remote: r.remote.clone(),
            parents_correspond: Evidence::Known(true),
        }];
        executor
            .record_conflict(&file, "Initial collision".into())
            .unwrap();
        let row = executor
            .inventory
            .enumerate(InventoryScopeQuery {
                account_id: r.identity.account_id,
                sync_root_id: r.identity.sync_root_id,
            })
            .unwrap()
            .remove(0);
        assert_eq!(row.baseline.state, BaselineState::NeverSynced);
        assert!(row.pending_operations.is_empty());
        let c = row.prior_conflict.unwrap();
        assert_eq!(c.local.state, FileState::Present);
        assert_eq!(c.remote.state, FileState::Present);
        assert!(!c.resolved);
        assert!(c.resolution.is_none());
    }
}
