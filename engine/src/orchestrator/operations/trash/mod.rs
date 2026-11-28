//! Execution admission for explicit, baseline-bound recoverable trash.
use super::*;
impl PersistingOperator {
    pub(super) fn validate_trash(
        &self,
        r: &ExecuteOperationRequest,
        stored: &InventorySnapshot,
        context: Option<&crate::activity::ActivityContext>,
    ) -> Result<(), OperatorError> {
        if !self.config.trash_propagation_authorized
            || stored.baseline.state != BaselineState::Synced
            || stored.baseline.projection != Some(ProjectionKind::Native)
        {
            return Err(filesystem::stale());
        }
        let (Some(local), Some(remote)) = (&stored.baseline.local, &stored.baseline.remote) else {
            return Err(filesystem::stale());
        };
        if !matches!(&local.identity,Evidence::Known(id) if id.birth_ns.is_some()) {
            return Err(filesystem::stale());
        }
        if local.state != FileState::Present
            || remote.state != FileState::Present
            || remote.trashed != Evidence::Known(false)
        {
            return Err(filesystem::stale());
        }
        self.confirmed_parent_with_context(
            &match &local.parent {
                Evidence::Known(p) => p.clone(),
                _ => return Err(filesystem::stale()),
            },
            &match &remote.parents {
                Evidence::Known(p) => p.clone(),
                _ => return Err(filesystem::stale()),
            },
            context,
        )
        .ok_or_else(filesystem::stale)?;
        if r.operation
            .preconditions
            .as_ref()
            .and_then(|p| p.baseline_remote_version)
            != match remote.version {
                Evidence::Known(v) => Some(v),
                _ => None,
            }
        {
            return Err(filesystem::stale());
        }
        if r.operation.action == OperationAction::TrashRemote {
            if !super::super::evidence::trash_source_matches(&r.remote, remote)
                || r.local.state != FileState::Absent
                || r.local.path != local.path
                || r.local.identity != local.identity
            {
                return Err(filesystem::stale());
            }
            self.filesystem
                .confirm_missing_incarnation(&r.local, &self.config.sync_root_id)?;
        } else {
            let mut expected = remote.clone();
            expected.state = FileState::Absent;
            expected.trashed = Evidence::Known(true);
            expected.version = r.remote.version.clone();
            if !crate::conflicts::local_matches(&r.local, local)
                || !crate::conflicts::remote_matches(&r.remote, &expected)
                || !matches!((&r.remote.version,&remote.version),(Evidence::Known(a),Evidence::Known(b)) if a>b)
            {
                return Err(filesystem::stale());
            }
        }
        if local.kind == Evidence::Known(FileKind::Directory) {
            let path = local.path.as_ref().ok_or_else(filesystem::stale)?;
            if self.inventory.enumerate_with_context(InventoryScopeQuery { account_id:self.config.account_id.clone(),sync_root_id:self.config.sync_root_id.clone() }, context).map_err(|_|filesystem::stale())?.iter().any(|s|s.identity.item_id != stored.identity.item_id && (s.local_path.as_ref().is_some_and(|p| p!=path && p.starts_with(path)) || s.baseline.remote.as_ref().is_some_and(|v| matches!(&v.parents,Evidence::Known(ids) if stored.identity.remote_id.as_ref().is_some_and(|id|ids.contains(id)))))) { return Err(filesystem::stale()); }
        }
        Ok(())
    }
}
