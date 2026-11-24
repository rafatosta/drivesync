//! Workspace replicas are read-only derived links, isolated from binary reconciliation.
use super::*;
use crate::{inventory::models::ProjectionKind, projections};
pub(super) fn analyze(
    file: &File,
    validation: Validation,
    changeset: Changeset,
) -> Option<Decision> {
    let projected = file.baseline.projection == Some(ProjectionKind::GoogleWorkspaceShortcut);
    if !projected && file.baseline.state == BaselineState::Synced {
        return None;
    }
    if !projections::workspace(&file.remote) && !projected {
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
            "Workspace projection requires complete current scope evidence.",
            DecisionStatus::Defer,
        ));
    }
    if !projections::workspace(&file.remote) {
        return Some(blocked(
            "The projection remote kind changed or is not fully observed.",
            if matches!(file.remote.mime_type, Evidence::Known(_)) {
                DecisionStatus::Unsupported
            } else {
                DecisionStatus::Defer
            },
        ));
    }
    if file.baseline.state == BaselineState::NeverSynced {
        return Some(
            initial_remote_plan(file, validation.clone(), changeset.clone()).unwrap_or_else(|| {
                blocked(
                    "Workspace projection requires an empty slot and a confirmed parent.",
                    DecisionStatus::Defer,
                )
            }),
        );
    }
    if !projected {
        return Some(blocked(
            "Existing native replicas are not converted into Workspace projections.",
            DecisionStatus::Unsupported,
        ));
    }
    let (Some(id), Some(old_local), Some(old_remote), Evidence::Known(name), Evidence::Known(mime)) = (
        &file.remote.id,
        &file.baseline.local,
        &file.baseline.remote,
        &file.remote.name,
        &file.remote.mime_type,
    ) else {
        return Some(blocked(
            "Projection evidence is incomplete.",
            DecisionStatus::Defer,
        ));
    };
    if file.local.identity != old_local.identity || !projections::content_matches(&file.local, id) {
        return Some(blocked(
            "Edited, replaced or absent local projections are preserved and never uploaded.",
            DecisionStatus::Unsupported,
        ));
    }
    let Some(name) = projections::local_name(name, mime) else {
        return Some(blocked(
            "Projection name is not representable.",
            DecisionStatus::Unsupported,
        ));
    };
    let corresponding = parents_correspond(file);
    if (file.local.name != old_local.name && file.local.name != Evidence::Known(name.clone()))
        || (file.local.parent != old_local.parent && !corresponding)
    {
        return Some(blocked(
            "Local projection location edits do not rename or move the remote document.",
            DecisionStatus::Unsupported,
        ));
    }
    let Some(item) = file.identity.item_id.clone() else {
        return Some(blocked(
            "Projection identity is missing.",
            DecisionStatus::Defer,
        ));
    };
    let mut operations = Vec::new();
    if file.local.name != Evidence::Known(name.clone()) {
        let Some(mut op) = operation_for(file, item.clone(), OperationAction::RenameLocal, false)
        else {
            return Some(blocked(
                "Projection rename destination is unresolved.",
                DecisionStatus::Defer,
            ));
        };
        op.name = Some(name);
        operations.push(op);
    }
    if !corresponding {
        if file.remote.parents == old_remote.parents {
            return Some(blocked(
                "The projection parent association is unresolved.",
                DecisionStatus::Defer,
            ));
        }
        let Some(op) = operation_for(file, item, OperationAction::MoveLocal, false) else {
            return Some(blocked(
                "Projection move destination is unresolved or occupied.",
                DecisionStatus::Defer,
            ));
        };
        operations.push(op);
    }
    if operations.is_empty() {
        if file.remote.version != old_remote.version || file.local.path != old_local.path {
            return Some(metadata_plan(
                file,
                &file.local,
                &file.remote,
                OperationAction::ConfirmBaseline,
                validation,
                changeset,
            ));
        }
        return Some(blocked(
            "Workspace projection matches its confirmed remote metadata.",
            DecisionStatus::NoOp,
        ));
    }
    for (i, op) in operations.iter_mut().enumerate() {
        op.step = i as u64 + 1;
        op.depends_on = if i == 0 { vec![] } else { vec![i as u64] };
    }
    let (execution, checks) = assess_execution(file, &operations);
    Some(decision(
        if execution == ExecutionStatus::Executable {
            DecisionStatus::Ready
        } else {
            DecisionStatus::Defer
        },
        "Update only the derived link location from verified remote metadata.",
        validation,
        changeset,
        if execution == ExecutionStatus::Executable {
            operations
        } else {
            vec![]
        },
        execution,
        checks,
    ))
}
