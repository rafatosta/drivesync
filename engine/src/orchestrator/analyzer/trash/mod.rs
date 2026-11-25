//! Explicit, recoverable trash propagation. Missing provider access is never deletion evidence.
use super::*;
pub(super) fn analyze(
    file: &File,
    validation: Validation,
    changeset: Changeset,
) -> Option<Decision> {
    if file.baseline.state != BaselineState::Synced
        || (file.local.state == FileState::Present
            && file.remote.state == FileState::Present
            && file.remote.trashed != Evidence::Known(true))
    {
        return None;
    }
    if !conflicts::attributes(file).is_empty() {
        return None;
    }
    let blocked = |reason: &str, status| {
        decision(
            status,
            reason,
            validation.clone(),
            changeset.clone(),
            vec![],
            ExecutionStatus::NotExecutable,
            vec![],
        )
    };
    if !complete(file) {
        return Some(blocked(
            "Trash propagation requires complete available source rounds.",
            DecisionStatus::Defer,
        ));
    }
    let (Some(old_local), Some(old_remote)) = (&file.baseline.local, &file.baseline.remote) else {
        return Some(blocked(
            "Trash propagation requires a confirmed baseline.",
            DecisionStatus::Defer,
        ));
    };
    if file.local.state == FileState::Absent
        && file.remote.state == FileState::Absent
        && file.remote.trashed == Evidence::Known(true)
        && old_local.state == FileState::Absent
        && old_remote.state == FileState::Absent
    {
        return Some(blocked(
            "Confirmed trash tombstone remains absent on both sides.",
            DecisionStatus::NoOp,
        ));
    }
    if !file.environment.trash_propagation_authorized {
        return Some(blocked(
            "Trash propagation is disabled for this session.",
            DecisionStatus::Defer,
        ));
    }
    if file.baseline.projection != Some(crate::inventory::models::ProjectionKind::Native) {
        return Some(blocked(
            "Projection removal does not authorize document trash propagation.",
            DecisionStatus::Unsupported,
        ));
    }
    if old_local.kind == Evidence::Known(crate::models::FileKind::Directory)
        && file.environment.trash_tree_safe != Evidence::Known(true)
    {
        return Some(blocked("Automatic folder trash requires a completely observed empty tree without registered descendants.", DecisionStatus::Defer));
    }
    if !parents_correspond(file) {
        return Some(blocked(
            "Trash propagation requires the original verified parent association.",
            DecisionStatus::Defer,
        ));
    }
    let action = if file.local.state == FileState::Absent
        && file.remote.state == FileState::Present
        && super::super::evidence::trash_source_matches(&file.remote, old_remote)
        && old_local.state == FileState::Present
    {
        OperationAction::TrashRemote
    } else if file.remote.state == FileState::Absent
        && file.remote.trashed == Evidence::Known(true)
        && file.local.state == FileState::Present
        && crate::conflicts::local_matches(&file.local, old_local)
        && old_remote.id == file.remote.id
        && old_remote.name == file.remote.name
        && old_remote.parents == file.remote.parents
        && old_remote.mime_type == file.remote.mime_type
        && old_remote.checksum == file.remote.checksum
        && old_remote.size == file.remote.size
    {
        OperationAction::RemoveLocal
    } else {
        return Some(blocked("Absence, access loss, scope exit or concurrent changes do not prove a safe trash operation.", DecisionStatus::Defer));
    };
    if action == OperationAction::TrashRemote
        && remote_capability(file, action) != Evidence::Known(true)
    {
        return Some(blocked(
            "Provider trash capability is missing or denied.",
            DecisionStatus::Defer,
        ));
    }
    let mut plan = metadata_plan(
        file,
        &file.local,
        &file.remote,
        action,
        validation,
        changeset,
    );
    plan.reason =
        "Propagate only verified recoverable trash under explicit session authorization.".into();
    if let Some(op) = plan.operations.first_mut() {
        op.preconditions.as_mut().unwrap().baseline_remote_version = known(&old_remote.version);
    }
    Some(plan)
}
