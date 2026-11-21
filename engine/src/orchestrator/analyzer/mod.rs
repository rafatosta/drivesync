//! Purely validates and analyzes a consolidated File without performing I/O.

mod models;

use crate::{
    detectors::context::File,
    inventory::models::{BaselineState, OperationAction},
    models::{Evidence, FileState, ItemId},
};
use std::collections::BTreeMap;

pub(crate) use models::{
    AttributeEvidence, AttributeName, AttributePairChange, ChangeStatus, Changeset,
    ComparisonStatus, Decision, DecisionStatus, ExecutionAssessment, ExecutionCheck,
    ExecutionStatus, Operation, OperationPreconditions, Validation, ValidationCheck,
    ValidationCheckStatus, ValidationStatus,
};

/// Analyzes one File using only its consolidated evidence and persisted baseline.
pub(crate) fn analyze(file: &File) -> Decision {
    let validation = validate(file);
    let changeset = compare(file);
    if validation.status != ValidationStatus::Valid {
        let status = match validation.status {
            ValidationStatus::InvalidInput => DecisionStatus::InvalidInput,
            ValidationStatus::Conflict => DecisionStatus::Conflict,
            ValidationStatus::Defer => DecisionStatus::Defer,
            ValidationStatus::Valid => unreachable!(),
        };
        let reason = validation.checks.last().map_or_else(
            || "Input is not actionable.".to_owned(),
            |check| check.detail.clone(),
        );
        return decision(
            status,
            &reason,
            validation,
            changeset,
            Vec::new(),
            ExecutionStatus::NotExecutable,
            Vec::new(),
        );
    }

    if file.baseline.state == BaselineState::NeverSynced {
        return analyze_unbased(file, validation, changeset);
    }
    if changeset.status == ComparisonStatus::Blocked {
        return decision(
            DecisionStatus::Defer,
            "The current observations cannot be compared with the baseline conclusively.",
            validation,
            changeset,
            Vec::new(),
            ExecutionStatus::NotExecutable,
            Vec::new(),
        );
    }

    let local_changed = side_changed(&changeset, true);
    let remote_changed = side_changed(&changeset, false);
    if !local_changed && !remote_changed {
        return decision(
            DecisionStatus::NoOp,
            "Both observed sides match the confirmed baseline.",
            validation,
            changeset,
            Vec::new(),
            ExecutionStatus::NotExecutable,
            Vec::new(),
        );
    }
    if local_changed && remote_changed {
        if current_states_diverge(file) {
            return decision(
                DecisionStatus::Conflict,
                "Both sides changed and their current values differ.",
                validation,
                changeset,
                Vec::new(),
                ExecutionStatus::NotExecutable,
                Vec::new(),
            );
        }
        return decision(
            DecisionStatus::Defer,
            "Both sides changed; the Analyzer does not merge concurrent edits.",
            validation,
            changeset,
            Vec::new(),
            ExecutionStatus::NotExecutable,
            Vec::new(),
        );
    }

    let from_local = local_changed;
    let mut operations = plan_unilateral_changes(file, &changeset, from_local);
    if operations.is_empty() {
        return decision(
            DecisionStatus::Defer,
            "The observed change has no safe, fully specified operation plan.",
            validation,
            changeset,
            Vec::new(),
            ExecutionStatus::NotExecutable,
            Vec::new(),
        );
    }
    for (index, operation) in operations.iter_mut().enumerate() {
        operation.step = (index + 1) as u64;
        operation.depends_on = if index == 0 {
            Vec::new()
        } else {
            vec![index as u64]
        };
    }
    let (execution, checks) = assess_execution(file, &operations);
    let decision_status = if execution == ExecutionStatus::Executable {
        DecisionStatus::Ready
    } else {
        DecisionStatus::Defer
    };
    decision(
        decision_status,
        if decision_status == DecisionStatus::Ready {
            "A unilateral change has a complete plan and satisfied current preconditions."
        } else {
            "A candidate plan is blocked by an unmet execution precondition."
        },
        validation,
        changeset,
        if decision_status == DecisionStatus::Ready {
            operations
        } else {
            Vec::new()
        },
        execution,
        checks,
    )
}

fn decision(
    status: DecisionStatus,
    reason: &str,
    validation: Validation,
    changeset: Changeset,
    operations: Vec<Operation>,
    execution_status: ExecutionStatus,
    execution_checks: Vec<ExecutionCheck>,
) -> Decision {
    let issues = execution_checks
        .iter()
        .filter(|check| !check.ok)
        .map(|check| check.detail.clone())
        .collect();
    Decision {
        status,
        reason: reason.into(),
        validation,
        changeset,
        operations,
        execution: ExecutionAssessment {
            status: execution_status,
            checks: execution_checks,
            issues,
        },
    }
}

fn validate(file: &File) -> Validation {
    let mut checks = Vec::new();
    if file.identity.account_id.0.trim().is_empty()
        || file.identity.sync_root_id.0.trim().is_empty()
    {
        push_check(
            &mut checks,
            ValidationCheckStatus::InvalidInput,
            "identity.scope",
            "Account and synchronization root identifiers must be present.",
        );
    } else {
        push_check(
            &mut checks,
            ValidationCheckStatus::Pass,
            "identity.scope",
            "Account and synchronization root are present.",
        );
    }

    let lookup_check = match file.inventory_lookup {
        crate::inventory::models::InventoryLookupStatus::Found => {
            if file.identity.item_id.is_some()
                && file.inventory_revision.is_some_and(|revision| revision > 0)
            {
                (
                    ValidationCheckStatus::Pass,
                    "A unique Inventory record and revision are available.",
                )
            } else {
                (
                    ValidationCheckStatus::InvalidInput,
                    "Found Inventory results require an item identity and revision.",
                )
            }
        }
        crate::inventory::models::InventoryLookupStatus::NotFound => {
            if file.baseline.state == BaselineState::NeverSynced
                && file.identity.item_id.is_none()
                && file.inventory_revision.is_none()
            {
                (
                    ValidationCheckStatus::Pass,
                    "Inventory confirmed that no baseline record exists.",
                )
            } else {
                (
                    ValidationCheckStatus::InvalidInput,
                    "NotFound must carry the explicit NeverSynced baseline state.",
                )
            }
        }
        crate::inventory::models::InventoryLookupStatus::Ambiguous => (
            ValidationCheckStatus::Conflict,
            "Inventory keys identify conflicting records.",
        ),
        crate::inventory::models::InventoryLookupStatus::Unavailable
        | crate::inventory::models::InventoryLookupStatus::Error => (
            ValidationCheckStatus::Defer,
            "Inventory did not return a conclusive result.",
        ),
    };
    push_check(
        &mut checks,
        lookup_check.0,
        "inventory.lookup",
        lookup_check.1,
    );

    if file.baseline.state == BaselineState::Synced
        && (file.baseline.local.is_none() || file.baseline.remote.is_none())
    {
        push_check(
            &mut checks,
            ValidationCheckStatus::InvalidInput,
            "baseline.missing_side",
            "A Synced baseline must contain both local and remote states.",
        );
    }

    if let Some(conflict) = &file.prior_conflict {
        if !conflict.resolved {
            push_check(
                &mut checks,
                ValidationCheckStatus::Conflict,
                "inventory.prior_conflict",
                "An unresolved conflict is already recorded for this item.",
            );
        }
    }
    if file.pending_operations.iter().any(|operation| {
        matches!(
            operation.state,
            crate::inventory::models::OperationState::Pending
                | crate::inventory::models::OperationState::Running
                | crate::inventory::models::OperationState::AppliedUnconfirmed
        )
    }) {
        push_check(
            &mut checks,
            ValidationCheckStatus::Defer,
            "operation.pending",
            "An operation is pending or has an unconfirmed effect.",
        );
    }
    if file
        .pending_operations
        .iter()
        .any(|operation| file.identity.item_id.as_ref() != Some(&operation.item_id))
        || file
            .prior_conflict
            .as_ref()
            .is_some_and(|conflict| file.identity.item_id.as_ref() != Some(&conflict.item_id))
    {
        push_check(
            &mut checks,
            ValidationCheckStatus::InvalidInput,
            "identity.inventory_item_mismatch",
            "Persisted operations or conflict do not belong to the File item identity.",
        );
    }

    if let (Evidence::Known(identity), Evidence::Known(observed)) =
        (&file.identity.local_identity, &file.local.identity)
    {
        if identity != observed {
            push_check(
                &mut checks,
                ValidationCheckStatus::Conflict,
                "identity.local_mismatch",
                "The observed local identity differs from the correlated identity.",
            );
        }
    }
    if let (Some(identity), Some(observed)) = (&file.identity.remote_id, &file.remote.id) {
        if identity != observed {
            push_check(
                &mut checks,
                ValidationCheckStatus::Conflict,
                "identity.remote_mismatch",
                "The observed remote identity differs from the correlated identity.",
            );
        }
    }

    if file.local.state == FileState::Absent
        && file.observations.local_coverage
            != crate::detectors::observation::ObservationCoverage::Complete
    {
        push_check(
            &mut checks,
            ValidationCheckStatus::Defer,
            "observation.local_absence",
            "Local absence is not supported by complete local coverage.",
        );
    }
    if file.remote.state == FileState::Absent
        && file.observations.remote_coverage
            != crate::detectors::observation::ObservationCoverage::Complete
    {
        push_check(
            &mut checks,
            ValidationCheckStatus::Defer,
            "observation.remote_absence",
            "Remote absence is not supported by complete remote coverage.",
        );
    }

    if checks.is_empty() {
        push_check(
            &mut checks,
            ValidationCheckStatus::Pass,
            "input.present",
            "Required input checks passed.",
        );
    }
    let status = if checks
        .iter()
        .any(|check| check.status == ValidationCheckStatus::InvalidInput)
    {
        ValidationStatus::InvalidInput
    } else if checks
        .iter()
        .any(|check| check.status == ValidationCheckStatus::Conflict)
    {
        ValidationStatus::Conflict
    } else if checks
        .iter()
        .any(|check| check.status == ValidationCheckStatus::Defer)
    {
        ValidationStatus::Defer
    } else {
        ValidationStatus::Valid
    };
    Validation { status, checks }
}

fn compare(file: &File) -> Changeset {
    let mut attributes = BTreeMap::new();
    let mut warnings = Vec::new();
    if file.baseline.state != BaselineState::Synced {
        warnings.push(validation_check(
            ValidationCheckStatus::Defer,
            "baseline.unavailable_for_comparison",
            "Attribute comparison requires a confirmed synchronized baseline.",
        ));
        return Changeset {
            status: ComparisonStatus::Blocked,
            baseline: file.baseline.state,
            attributes,
            evidence: vec![format!("Baseline state: {:?}.", file.baseline.state)],
            warnings,
        };
    }
    let (Some(baseline_local), Some(baseline_remote)) =
        (&file.baseline.local, &file.baseline.remote)
    else {
        warnings.push(validation_check(
            ValidationCheckStatus::InvalidInput,
            "baseline.missing_side",
            "A Synced baseline must contain both local and remote states.",
        ));
        return Changeset {
            status: ComparisonStatus::Blocked,
            baseline: file.baseline.state,
            attributes,
            evidence: Vec::new(),
            warnings,
        };
    };

    add_attribute(
        &mut attributes,
        AttributeName::Existence,
        compare_state(baseline_local.state, file.local.state, "local presence"),
        compare_state(baseline_remote.state, file.remote.state, "remote presence"),
    );
    add_attribute(
        &mut attributes,
        AttributeName::Kind,
        compare_evidence(&baseline_local.kind, &file.local.kind, "local kind"),
        compare_evidence(
            &baseline_remote.mime_type,
            &file.remote.mime_type,
            "remote MIME type",
        ),
    );
    add_attribute(
        &mut attributes,
        AttributeName::Content,
        compare_content(
            &baseline_local.checksum,
            &baseline_local.size,
            &file.local.checksum,
            &file.local.size,
            &file.local.kind,
            "local content",
        ),
        compare_content(
            &baseline_remote.checksum,
            &baseline_remote.size,
            &file.remote.checksum,
            &file.remote.size,
            &file.local.kind,
            "remote content",
        ),
    );
    add_attribute(
        &mut attributes,
        AttributeName::Name,
        compare_evidence(&baseline_local.name, &file.local.name, "local name"),
        compare_evidence(&baseline_remote.name, &file.remote.name, "remote name"),
    );
    add_attribute(
        &mut attributes,
        AttributeName::Parent,
        compare_evidence(&baseline_local.parent, &file.local.parent, "local parent"),
        compare_evidence(
            &baseline_remote.parents,
            &file.remote.parents,
            "remote parent",
        ),
    );
    add_attribute(
        &mut attributes,
        AttributeName::Trashed,
        not_applicable("Local trash state is not defined by this contract.".into()),
        compare_evidence(
            &baseline_remote.trashed,
            &file.remote.trashed,
            "remote trash state",
        ),
    );

    for (name, change) in &attributes {
        for (side, evidence) in [("local", &change.local), ("remote", &change.remote)] {
            if evidence.status == ChangeStatus::Indeterminate {
                warnings.push(validation_check(
                    ValidationCheckStatus::Defer,
                    &format!("comparison.{name:?}.{side}"),
                    &evidence.basis,
                ));
            }
        }
    }
    let evidence = attributes
        .iter()
        .map(|(name, change)| {
            format!(
                "{name:?}: local {:?}, remote {:?}.",
                change.local.status, change.remote.status
            )
        })
        .collect();
    let status = if warnings.is_empty() {
        ComparisonStatus::Comparable
    } else {
        ComparisonStatus::Blocked
    };
    Changeset {
        status,
        baseline: file.baseline.state,
        attributes,
        evidence,
        warnings,
    }
}

fn add_attribute(
    attributes: &mut BTreeMap<AttributeName, AttributePairChange>,
    name: AttributeName,
    local: AttributeEvidence,
    remote: AttributeEvidence,
) {
    attributes.insert(name, AttributePairChange { local, remote });
}

fn compare_state(before: FileState, current: FileState, label: &str) -> AttributeEvidence {
    if matches!(before, FileState::Unknown | FileState::Unavailable)
        || matches!(current, FileState::Unknown | FileState::Unavailable)
    {
        return indeterminate(format!("{label} is not fully observed."));
    }
    if before == current {
        unchanged(format!("{label} is {current:?} in both observations."))
    } else {
        changed(format!("{label} changed from {before:?} to {current:?}."))
    }
}

fn compare_evidence<T: PartialEq>(
    before: &Evidence<T>,
    current: &Evidence<T>,
    label: &str,
) -> AttributeEvidence {
    match (before, current) {
        (Evidence::Known(before), Evidence::Known(current)) if before == current => {
            unchanged(format!("{label} is equal to the baseline."))
        }
        (Evidence::Known(_), Evidence::Known(_)) => {
            changed(format!("{label} differs from the baseline."))
        }
        (Evidence::NotApplicable, Evidence::NotApplicable) => {
            not_applicable(format!("{label} does not apply."))
        }
        _ => indeterminate(format!(
            "{label} cannot be compared from the available evidence."
        )),
    }
}

fn compare_content(
    baseline_checksum: &Evidence<crate::models::Checksum>,
    baseline_size: &Evidence<u64>,
    current_checksum: &Evidence<crate::models::Checksum>,
    current_size: &Evidence<u64>,
    kind: &Evidence<crate::models::FileKind>,
    label: &str,
) -> AttributeEvidence {
    if *kind == Evidence::Known(crate::models::FileKind::Directory) {
        return not_applicable(format!("{label} is not binary content for a directory."));
    }
    if let (Evidence::Known(before), Evidence::Known(current)) =
        (baseline_checksum, current_checksum)
    {
        if before.algorithm == current.algorithm {
            return if before.value == current.value {
                unchanged(format!("{label} checksum is unchanged."))
            } else {
                changed(format!("{label} checksum changed."))
            };
        }
    }
    if let (Evidence::Known(before), Evidence::Known(current)) = (baseline_size, current_size) {
        if before != current {
            return changed(format!("{label} size changed; content differs."));
        }
    }
    indeterminate(format!(
        "{label} has no comparable checksum or conclusive size change."
    ))
}

fn side_changed(changeset: &Changeset, local: bool) -> bool {
    changeset.attributes.values().any(|change| {
        let evidence = if local { &change.local } else { &change.remote };
        evidence.status == ChangeStatus::Changed
    })
}

fn current_states_diverge(file: &File) -> bool {
    file.local.state != file.remote.state
        || known_values_differ(&file.local.name, &file.remote.name)
        || match (&file.local.checksum, &file.remote.checksum) {
            (Evidence::Known(local), Evidence::Known(remote)) => {
                local.algorithm == remote.algorithm && local.value != remote.value
            }
            _ => false,
        }
        || match (&file.local.size, &file.remote.size) {
            (Evidence::Known(local), Evidence::Known(remote)) => local != remote,
            _ => false,
        }
}

fn known_values_differ<T: PartialEq>(left: &Evidence<T>, right: &Evidence<T>) -> bool {
    matches!((left, right), (Evidence::Known(left), Evidence::Known(right)) if left != right)
}

fn analyze_unbased(file: &File, validation: Validation, changeset: Changeset) -> Decision {
    if file.local.state == FileState::Present && file.remote.state == FileState::Present {
        let equivalent = matches!(file.identity.local_identity, Evidence::Known(_))
            && file.identity.remote_id.is_some()
            && known_equal(&file.local.name, &file.remote.name)
            && known_equal(&file.local.size, &file.remote.size)
            && known_equal(&file.local.checksum, &file.remote.checksum);
        if equivalent && file.initial_association_authorized {
            return decision(
                DecisionStatus::Adopt,
                "Both sides have matching observed metadata and initial association is authorized.",
                validation,
                changeset,
                Vec::new(),
                ExecutionStatus::NotExecutable,
                Vec::new(),
            );
        }
        return decision(
            DecisionStatus::Defer,
            "An unbased item cannot be associated without matching evidence and explicit authorization.",
            validation,
            changeset,
            Vec::new(),
            ExecutionStatus::NotExecutable,
            Vec::new(),
        );
    }
    decision(
        DecisionStatus::Defer,
        "A new item needs a stable logical item identity and an explicitly verified destination before planning creation.",
        validation,
        changeset,
        Vec::new(),
        ExecutionStatus::NotExecutable,
        Vec::new(),
    )
}

fn known_equal<T: PartialEq>(left: &Evidence<T>, right: &Evidence<T>) -> bool {
    matches!((left, right), (Evidence::Known(left), Evidence::Known(right)) if left == right)
}

fn plan_unilateral_changes(file: &File, changeset: &Changeset, from_local: bool) -> Vec<Operation> {
    let Some(item_ref) = file.identity.item_id.clone() else {
        return Vec::new();
    };
    let mut operations = Vec::new();
    for (attribute, change) in &changeset.attributes {
        let changed = if from_local {
            change.local.status == ChangeStatus::Changed
        } else {
            change.remote.status == ChangeStatus::Changed
        };
        if !changed {
            continue;
        }
        let action = match (*attribute, from_local) {
            (AttributeName::Content, true) if file.remote.state == FileState::Present => {
                OperationAction::UpdateRemoteContent
            }
            (AttributeName::Content, false) if file.local.state == FileState::Present => {
                OperationAction::UpdateLocalContent
            }
            (AttributeName::Name, true) => OperationAction::RenameRemote,
            (AttributeName::Name, false) => OperationAction::RenameLocal,
            (AttributeName::Parent, true) => OperationAction::MoveRemote,
            (AttributeName::Parent, false) => OperationAction::MoveLocal,
            _ => return Vec::new(),
        };
        let operation = operation_for(file, item_ref.clone(), action, from_local);
        let Some(operation) = operation else {
            return Vec::new();
        };
        operations.push(operation);
    }
    operations
}

fn operation_for(
    file: &File,
    item_ref: ItemId,
    action: OperationAction,
    from_local: bool,
) -> Option<Operation> {
    let (remote_id, local_path, name, parent_id, parent) = match action {
        OperationAction::UpdateRemoteContent
        | OperationAction::RenameRemote
        | OperationAction::MoveRemote => (
            file.identity.remote_id.clone(),
            file.local.path.clone(),
            if from_local {
                known(&file.local.name)
            } else {
                known(&file.remote.name)
            },
            if action == OperationAction::MoveRemote {
                if file.environment.destination_remote_matches != Evidence::Known(Vec::new()) {
                    return None;
                }
                match &file.environment.destination_remote_parent_id {
                    Evidence::Known(parent) => Some(parent.clone()),
                    _ => return None,
                }
            } else {
                None
            },
            None,
        ),
        OperationAction::UpdateLocalContent
        | OperationAction::RenameLocal
        | OperationAction::MoveLocal => (
            file.identity.remote_id.clone(),
            file.local.path.clone(),
            if from_local {
                known(&file.local.name)
            } else {
                known(&file.remote.name)
            },
            None,
            if action == OperationAction::MoveLocal {
                if file.environment.destination_local_occupied != Evidence::Known(false) {
                    return None;
                }
                match &file.environment.destination_local_parent {
                    Evidence::Known(parent) => Some(parent.clone()),
                    _ => return None,
                }
            } else {
                known(&file.local.parent)
            },
        ),
        _ => return None,
    };
    Some(Operation {
        step: 0,
        action,
        item_ref,
        depends_on: Vec::new(),
        remote_id,
        local_path,
        name,
        parent_id,
        parent,
        preconditions: Some(OperationPreconditions {
            observed_remote_version: known(&file.remote.version),
            baseline_remote_version: file
                .baseline
                .remote
                .as_ref()
                .and_then(|remote| known(&remote.version)),
            local_unchanged: Evidence::Known(true),
            remote_unchanged: Evidence::Known(true),
            destination_free: if matches!(
                action,
                OperationAction::MoveLocal | OperationAction::MoveRemote
            ) {
                Evidence::Known(true)
            } else {
                Evidence::NotApplicable
            },
        }),
    })
}

fn assess_execution(
    file: &File,
    operations: &[Operation],
) -> (ExecutionStatus, Vec<ExecutionCheck>) {
    let mut checks = Vec::new();
    for operation in operations {
        match operation.action {
            OperationAction::UpdateRemoteContent
            | OperationAction::RenameRemote
            | OperationAction::MoveRemote => {
                execution_check(
                    &mut checks,
                    file.environment.remote_available,
                    "remote.available",
                    "Remote provider must be available.",
                );
                execution_check(
                    &mut checks,
                    operation.remote_id.is_some(),
                    "remote.identity",
                    "A remote item identity is required.",
                );
                execution_check(
                    &mut checks,
                    known(&file.remote.version).is_some(),
                    "remote.version",
                    "A remote version is required for conditional writes.",
                );
                capability_check(
                    &mut checks,
                    remote_capability(file, operation.action),
                    "remote.capability",
                    "The provider must permit this remote operation.",
                );
                if operation.action == OperationAction::UpdateRemoteContent {
                    execution_check(
                        &mut checks,
                        file.environment.local_root_available,
                        "local.available",
                        "The local source must be available.",
                    );
                    execution_check(
                        &mut checks,
                        operation.local_path.is_some(),
                        "local.path",
                        "A known local source path is required.",
                    );
                    capability_check(
                        &mut checks,
                        local_capability(file, operation.action),
                        "local.capability",
                        "The filesystem must permit reading the local source.",
                    );
                } else {
                    execution_check(
                        &mut checks,
                        file.environment.local_root_available,
                        "local.available",
                        "The local source scope must be available.",
                    );
                    execution_check(
                        &mut checks,
                        operation.local_path.is_some(),
                        "local.path",
                        "A known local source path is required.",
                    );
                    execution_check(
                        &mut checks,
                        matches!(file.local.identity, Evidence::Known(_)),
                        "local.identity",
                        "A local identity is required to recheck the source.",
                    );
                }
            }
            OperationAction::UpdateLocalContent
            | OperationAction::RenameLocal
            | OperationAction::MoveLocal => {
                execution_check(
                    &mut checks,
                    file.environment.local_root_available,
                    "local.available",
                    "The local synchronization root must be available.",
                );
                execution_check(
                    &mut checks,
                    file.environment.remote_available,
                    "remote.available",
                    "The remote source must be available for revalidation.",
                );
                execution_check(
                    &mut checks,
                    operation.remote_id.is_some(),
                    "remote.identity",
                    "A remote identity is required to recheck the source.",
                );
                execution_check(
                    &mut checks,
                    known(&file.remote.version).is_some(),
                    "remote.version",
                    "A remote version is required to recheck the source.",
                );
                execution_check(
                    &mut checks,
                    operation.local_path.is_some(),
                    "local.path",
                    "A known local item path is required.",
                );
                execution_check(
                    &mut checks,
                    matches!(file.local.identity, Evidence::Known(_)),
                    "local.identity",
                    "A local identity is required to recheck the target.",
                );
                capability_check(
                    &mut checks,
                    local_capability(file, operation.action),
                    "local.capability",
                    "The filesystem must permit this local operation.",
                );
                if operation.action == OperationAction::UpdateLocalContent {
                    capability_check(
                        &mut checks,
                        remote_download_capability(file),
                        "remote.download_capability",
                        "The provider must permit downloading this item.",
                    );
                }
            }
            _ => execution_check(
                &mut checks,
                false,
                "operation.unsupported",
                "This operation is not supported by the current Analyzer plan.",
            ),
        }
    }
    let status = if checks.iter().all(|check| check.ok) {
        ExecutionStatus::Executable
    } else if checks
        .iter()
        .any(|check| check.code.ends_with("_denied") || check.code.ends_with("_unsupported"))
    {
        ExecutionStatus::NotExecutable
    } else {
        ExecutionStatus::Blocked
    };
    (status, checks)
}

fn remote_capability(file: &File, action: OperationAction) -> Evidence<bool> {
    match &file.remote.capabilities {
        Evidence::Known(capabilities) => match action {
            OperationAction::UpdateRemoteContent => capabilities.can_modify_content.clone(),
            OperationAction::RenameRemote => capabilities.can_rename.clone(),
            OperationAction::MoveRemote => capabilities.can_move_item_within_drive.clone(),
            _ => Evidence::Unsupported,
        },
        other => unavailable_capability(other),
    }
}

fn local_capability(file: &File, action: OperationAction) -> Evidence<bool> {
    match &file.local.capabilities {
        Evidence::Known(capabilities) => match action {
            OperationAction::UpdateLocalContent => capabilities.can_write.clone(),
            OperationAction::UpdateRemoteContent => capabilities.can_read.clone(),
            OperationAction::RenameLocal | OperationAction::MoveLocal => {
                capabilities.can_rename.clone()
            }
            _ => Evidence::Unsupported,
        },
        other => unavailable_capability(other),
    }
}

fn remote_download_capability(file: &File) -> Evidence<bool> {
    match &file.remote.capabilities {
        Evidence::Known(capabilities) => capabilities.can_download.clone(),
        other => unavailable_capability(other),
    }
}

fn unavailable_capability<T>(evidence: &Evidence<T>) -> Evidence<bool> {
    match evidence {
        Evidence::Known(_) => Evidence::NotCollected,
        Evidence::NotCollected => Evidence::NotCollected,
        Evidence::NotApplicable => Evidence::NotApplicable,
        Evidence::Unsupported => Evidence::Unsupported,
        Evidence::Unavailable => Evidence::Unavailable,
        Evidence::PermissionDenied => Evidence::PermissionDenied,
    }
}

fn capability_check(
    checks: &mut Vec<ExecutionCheck>,
    evidence: Evidence<bool>,
    code: &str,
    detail: &str,
) {
    match evidence {
        Evidence::Known(true) => {
            execution_check(checks, true, code, "Required capability is available.")
        }
        Evidence::Known(false) | Evidence::PermissionDenied => execution_check(
            checks,
            false,
            &format!("{code}_denied"),
            "Required capability is denied.",
        ),
        Evidence::Unsupported | Evidence::NotApplicable => execution_check(
            checks,
            false,
            &format!("{code}_unsupported"),
            "Required capability is not supported.",
        ),
        Evidence::NotCollected | Evidence::Unavailable => {
            execution_check(checks, false, code, detail)
        }
    }
}

fn execution_check(checks: &mut Vec<ExecutionCheck>, ok: bool, code: &str, detail: &str) {
    checks.push(ExecutionCheck {
        ok,
        code: code.into(),
        detail: detail.into(),
    });
}

fn push_check(
    checks: &mut Vec<ValidationCheck>,
    status: ValidationCheckStatus,
    code: &str,
    detail: &str,
) {
    checks.push(validation_check(status, code, detail));
}

fn validation_check(status: ValidationCheckStatus, code: &str, detail: &str) -> ValidationCheck {
    ValidationCheck {
        status,
        code: code.into(),
        detail: detail.into(),
    }
}

fn changed(basis: String) -> AttributeEvidence {
    AttributeEvidence {
        status: ChangeStatus::Changed,
        basis,
    }
}
fn unchanged(basis: String) -> AttributeEvidence {
    AttributeEvidence {
        status: ChangeStatus::Unchanged,
        basis,
    }
}
fn indeterminate(basis: String) -> AttributeEvidence {
    AttributeEvidence {
        status: ChangeStatus::Indeterminate,
        basis,
    }
}
fn not_applicable(basis: String) -> AttributeEvidence {
    AttributeEvidence {
        status: ChangeStatus::NotApplicable,
        basis,
    }
}

fn known<T: Clone>(evidence: &Evidence<T>) -> Option<T> {
    match evidence {
        Evidence::Known(value) => Some(value.clone()),
        _ => None,
    }
}

#[cfg(test)]
pub(super) mod tests;
