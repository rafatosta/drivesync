//! Read-only convergence attestation for a captured horizon, independent of GUI/queue idleness.
use super::*;
use crate::{
    inventory::models::{BaselineState, OperationState},
    models::LocalIdentity,
    synchronization::SynchronizationAudit,
};
use std::os::unix::fs::MetadataExt;
impl Context {
    pub(crate) fn audit(
        &self,
        epoch: u64,
        expected: &LocalIdentity,
    ) -> Result<SynchronizationAudit, ContextError> {
        let _round = self
            .rounds
            .lock()
            .map_err(|_| ContextError::InvalidCapture)?;
        let _execution = self
            .inventory
            .execution_lease()
            .map_err(|_| ContextError::InventoryUnavailable)?;
        let captures = self
            .captures
            .lock()
            .map_err(|_| ContextError::InvalidCapture)?;
        let snapshots = self.snapshots()?;
        let local = captures.locals();
        let remote = captures.remotes();
        let root = std::fs::symlink_metadata(&self.config.local_root.0)
            .map_err(|_| ContextError::InvalidCapture)?;
        let root_matches = root.is_dir()
            && root.dev() == expected.device_id
            && root.ino() == expected.inode
            && crate::models::birth_ns(&root) == expected.birth_ns;
        if !root_matches {
            return Err(ContextError::InvalidCapture);
        }
        let coverage = [&captures.local, &captures.remote].iter().all(|capture| {
            capture.as_ref().is_some_and(|c| {
                c.coverage == ObservationCoverage::Complete && c.collect_epoch >= epoch
            })
        });
        let mut audit = SynchronizationAudit {
            roots: crate::synchronization::StartSynchronizationRequest {
                account_id: self.config.account_id.clone(),
                local_root: self.config.local_root.0.clone(),
                remote_root_id: self.config.remote_root_id.clone(),
            },
            initial_complete: false,
            coverage_complete: coverage,
            epoch,
            local_entries: local.len(),
            remote_entries: remote.len(),
            inventory_items: snapshots.len(),
            converged_items: 0,
            unconfirmed_operations: 0,
            unresolved_conflicts: 0,
        };
        let mut owned_local = std::collections::BTreeSet::new();
        let mut owned_remote = std::collections::BTreeSet::new();
        for snapshot in &snapshots {
            audit.unconfirmed_operations += snapshot
                .pending_operations
                .iter()
                .filter(|p| {
                    matches!(
                        p.state,
                        OperationState::Pending
                            | OperationState::Running
                            | OperationState::AppliedUnconfirmed
                    )
                })
                .count();
            audit.unresolved_conflicts += usize::from(
                snapshot
                    .prior_conflict
                    .as_ref()
                    .is_some_and(|c| !c.resolved),
            );
            if snapshot.baseline.state != BaselineState::Synced {
                continue;
            }
            let (Some(old_local), Some(old_remote), Some(id)) = (
                &snapshot.baseline.local,
                &snapshot.baseline.remote,
                &snapshot.identity.remote_id,
            ) else {
                continue;
            };
            let Some(current_remote) = remote.get(id).map(|r| remote_state((*r).clone())) else {
                continue;
            };
            let Some(path) = &snapshot.local_path else {
                continue;
            };
            let local_matches = if old_local.state == FileState::Absent {
                !local.contains_key(path)
                    && !local
                        .values()
                        .any(|l| l.identity == snapshot.identity.local_identity)
            } else {
                local.get(path).is_some_and(|l| {
                    let mut content = local_state((*l).clone());
                    content.modified_at = old_local.modified_at.clone();
                    content.changed_at = old_local.changed_at.clone();
                    crate::conflicts::local_matches(&content, old_local)
                        && l.identity == snapshot.identity.local_identity
                })
            };
            let mut remote_expected = old_remote.clone();
            remote_expected.version = current_remote.version.clone();
            let remote_matches = crate::conflicts::remote_matches(
                &remote_expected,
                &current_remote,
            ) && matches!((&current_remote.version,&old_remote.version),(Evidence::Known(now),Evidence::Known(before)) if now>=before&&*before>0);
            if local_matches && remote_matches {
                audit.converged_items += 1;
                owned_remote.insert(id.clone());
                if old_local.state == FileState::Present {
                    owned_local.insert(path.clone());
                }
            }
        }
        let digest_valid = |checksum: &Evidence<crate::models::Checksum>| matches!(checksum,Evidence::Known(c) if c.algorithm=="sha256"&&c.value.len()==64&&c.value.bytes().all(|b|b.is_ascii_hexdigit()));
        let representable=local.values().all(|l| matches!(&l.name,Evidence::Known(n) if crate::projections::valid_name(n))&&matches!(l.kind,Evidence::Known(crate::models::FileKind::Regular|crate::models::FileKind::Directory))&&(l.kind!=Evidence::Known(crate::models::FileKind::Regular)||digest_valid(&l.checksum)))
            && remote.values().all(|r| matches!((&r.name,&r.mime_type),(Evidence::Known(n),Evidence::Known(m)) if crate::projections::local_name(n,m).is_some()&&match crate::projections::kind(m) {crate::projections::RemoteKind::Binary=>digest_valid(&r.checksum)&&matches!(r.size,Evidence::Known(_)),crate::projections::RemoteKind::Directory|crate::projections::RemoteKind::Workspace=>true,_=>false}));
        // Every observed item must have exactly one proven replica owner; names never supply identity.
        let local_owned =
            owned_local.len() == local.len() && local.keys().all(|p| owned_local.contains(p));
        let remote_owned = owned_remote.len() == remote.len()
            && remote.iter().all(|(id, r)| {
                owned_remote.contains(id)
                    && matches!(r.state, FileState::Present | FileState::Absent)
            });
        audit.initial_complete = representable
            && coverage
            && local_owned
            && remote_owned
            && audit.converged_items == audit.inventory_items
            && audit.unconfirmed_operations == 0
            && audit.unresolved_conflicts == 0;
        Ok(audit)
    }
}
