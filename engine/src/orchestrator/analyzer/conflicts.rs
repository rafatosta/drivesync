//! Conflict decisions are attribute-specific; consent is tied to current evidence, never a boolean.
use super::*;
use crate::{
    conflicts::{self, ConflictChoice},
    inventory::models::ConflictState,
};

pub(super) fn authorized(file: &File) -> bool {
    file.prior_conflict
        .as_ref()
        .filter(|c| !c.resolved)
        .and_then(|c| c.resolution.as_ref())
        .is_some_and(|r| {
            conflicts::resolvable(&file.local, &file.remote)
                && conflicts::local_matches(&r.expected_local, &file.local)
                && conflicts::remote_matches(&r.expected_remote, &file.remote)
        })
}
pub(super) fn initial_incompatible(
    local: &crate::inventory::models::LocalState,
    remote: &crate::inventory::models::RemoteState,
) -> bool {
    matches!((&local.kind,remote_kind(&remote.mime_type)),(Evidence::Known(a),Evidence::Known(b)) if a!=&b)
        || matches!((&local.checksum,&remote.checksum),(Evidence::Known(a),Evidence::Known(b)) if a.algorithm==b.algorithm && a.value!=b.value)
        || matches!((&local.size,&remote.size),(Evidence::Known(a),Evidence::Known(b)) if a!=b)
}
pub(crate) fn attributes(file: &File) -> Vec<String> {
    if !complete(file) {
        return vec![];
    }
    let changes = compare(file);
    let changed = |name, local| {
        changes.attributes.get(&name).is_some_and(|c| {
            if local {
                c.local.status == ChangeStatus::Changed
            } else {
                c.remote.status == ChangeStatus::Changed
            }
        })
    };
    let mut result = Vec::new();
    for (attribute, equal) in [
        (AttributeName::Name, file.local.name == file.remote.name),
        (
            AttributeName::Content,
            file.local.checksum == file.remote.checksum && file.local.size == file.remote.size,
        ),
        (AttributeName::Parent, parents_correspond(file)),
    ] {
        if changed(attribute, true) && changed(attribute, false) && !equal {
            result.push(format!("{attribute:?}"));
        }
    }
    let kind_conflict = matches!((&file.local.kind,remote_kind(&file.remote.mime_type)),(Evidence::Known(local),Evidence::Known(remote)) if local!=&remote);
    if kind_conflict && (changed(AttributeName::Kind, true) || changed(AttributeName::Kind, false))
    {
        result.push("Kind".into());
    }
    if (file.remote.trashed == Evidence::Known(true) || file.remote.state == FileState::Absent)
        && side_changed(&changes, true)
    {
        result.push("Existence/Trashed".into());
    }
    if file.local.state == FileState::Absent && side_changed(&changes, false) {
        result.push("Existence".into());
    }
    result
}
pub(super) fn selected(file: &File, validation: Validation, mut changeset: Changeset) -> Decision {
    let ConflictState {
        id,
        resolution: Some(resolution),
        ..
    } = file.prior_conflict.as_ref().unwrap()
    else {
        unreachable!()
    };
    let from_local = resolution.choice == ConflictChoice::KeepLocal;
    changeset.status = ComparisonStatus::Comparable;
    changeset.attributes.clear();
    changeset.warnings.clear();
    for (attribute, equal) in [
        (AttributeName::Name, file.local.name == file.remote.name),
        (AttributeName::Parent, parents_correspond(file)),
        (
            AttributeName::Content,
            file.local.kind == Evidence::Known(crate::models::FileKind::Directory)
                || (file.local.checksum == file.remote.checksum
                    && file.local.size == file.remote.size),
        ),
    ] {
        let yes = changed("Explicit selected replica differs from its counterpart.".into());
        let no = unchanged("No replacement required for this attribute.".into());
        changeset.attributes.insert(
            attribute,
            AttributePairChange {
                local: if !equal && from_local {
                    yes.clone()
                } else {
                    no.clone()
                },
                remote: if !equal && !from_local { yes } else { no },
            },
        );
    }
    let mut result = if changeset.attributes.values().all(|p| {
        p.local.status != ChangeStatus::Changed && p.remote.status != ChangeStatus::Changed
    }) {
        metadata_plan(
            file,
            &file.local,
            &file.remote,
            OperationAction::ConfirmBaseline,
            validation,
            changeset,
        )
    } else {
        let mut operations = plan_unilateral_changes(file, &changeset, from_local);
        for (index, op) in operations.iter_mut().enumerate() {
            op.step = index as u64 + 1;
            op.depends_on = if index == 0 {
                vec![]
            } else {
                vec![index as u64]
            };
        }
        let (execution, checks) = assess_execution(file, &operations);
        let ready = !operations.is_empty() && execution == ExecutionStatus::Executable;
        decision(if ready{DecisionStatus::Ready}else{DecisionStatus::Defer},"Revalidate the explicit conflict choice and preserve both original contents before applying it.",validation,changeset,if ready{operations}else{vec![]},execution,checks)
    };
    for op in &mut result.operations {
        op.conflict_id = Some(id.clone());
    }
    result
}

/// Disjoint attributes can converge without authorizing an incompatible overwrite.
pub(super) fn independent(
    file: &File,
    validation: Validation,
    mut changeset: Changeset,
) -> Decision {
    if file.local.checksum == file.remote.checksum && file.local.size == file.remote.size {
        if let Some(content) = changeset.attributes.get_mut(&AttributeName::Content) {
            content.local = unchanged("Equivalent current content.".into());
            content.remote = content.local.clone();
        }
    }
    let mut local = plan_unilateral_changes(file, &changeset, true);
    let remote = plan_unilateral_changes(file, &changeset, false);
    if (side_changed(&changeset, true) && local.is_empty())
        || (side_changed(&changeset, false) && remote.is_empty())
    {
        return decision(
            DecisionStatus::Defer,
            "Concurrent attributes include an unsupported change.",
            validation,
            changeset,
            vec![],
            ExecutionStatus::NotExecutable,
            vec![],
        );
    }
    local.extend(remote);
    local.sort_by_key(|op| match op.action {
        OperationAction::RenameLocal | OperationAction::RenameRemote => 0,
        OperationAction::MoveLocal | OperationAction::MoveRemote => 1,
        _ => 2,
    });
    for (i, op) in local.iter_mut().enumerate() {
        op.step = i as u64 + 1;
        op.depends_on = if i == 0 { vec![] } else { vec![i as u64] };
    }
    let (execution, checks) = assess_execution(file, &local);
    let ready = !local.is_empty() && execution == ExecutionStatus::Executable;
    decision(
        if ready {
            DecisionStatus::Ready
        } else {
            DecisionStatus::Defer
        },
        "Merge disjoint attributes using verified intermediate receipts.",
        validation,
        changeset,
        if ready { local } else { vec![] },
        execution,
        checks,
    )
}
