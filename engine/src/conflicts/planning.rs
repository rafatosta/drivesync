//! Conservative adapters between B/L/R decisions and supported download operations.
use super::*;
use crate::{ChangeKind, ConfirmedBase, ConflictReason, LocalObservation, PlanAction};

fn present(content: ContentComparison) -> SideDifference {
    SideDifference::Present {
        content,
        name: AttributeChange::Unchanged,
        parent: AttributeChange::Unchanged,
    }
}
/// Converts the download flow's scoped evidence without inventing name/move history.
/// A remote revision change is a possible content change until the collector proves
/// equality/difference. The initial blocker is not itself a proven content conflict.
pub(crate) fn binary_decision(
    base: Option<&ConfirmedBase>,
    local: &LocalObservation,
    remote_version: u64,
) -> PlanAction {
    use ContentComparison as C;
    if remote_version == 0
        || base.is_some_and(|b| {
            b.remote_version == 0
                || crate::planner::fingerprint_parts(&b.local_fingerprint).is_none()
                || remote_version < b.remote_version
        })
    {
        return PlanAction::DeferredUnknown;
    }
    let remote_changed = base.is_some_and(|b| b.remote_version != remote_version);
    let local_difference = match local {
        LocalObservation::Unknown => SideDifference::Unknown,
        LocalObservation::Absent => SideDifference::Absent,
        LocalObservation::Directory | LocalObservation::SymlinkOrSpecial => {
            SideDifference::Unsupported
        }
        LocalObservation::UnmanagedFile => {
            return PlanAction::Blocked(ConflictReason::ExistingLocalData);
        }
        LocalObservation::Verified(value) => match base {
            None => SideDifference::unchanged(),
            Some(base) => {
                let Some((hash, dev, ino)) = crate::planner::fingerprint_parts(value) else {
                    return PlanAction::DeferredUnknown;
                };
                let Some((bh, bd, bi)) = crate::planner::fingerprint_parts(&base.local_fingerprint)
                else {
                    return PlanAction::DeferredUnknown;
                };
                if (dev, ino) != (bd, bi) {
                    SideDifference::Replaced
                } else {
                    present(if hash.eq_ignore_ascii_case(bh) {
                        C::Equal
                    } else {
                        C::Different
                    })
                }
            }
        },
    };
    let input = DifferenceInput {
        base: if base.is_some() {
            BaseEvidence::Confirmed
        } else {
            BaseEvidence::Absent
        },
        local: local_difference,
        remote: present(if remote_changed {
            C::Different
        } else {
            C::Equal
        }),
        current_content: C::Unknown,
    };
    let result = classify_differences(&input);
    match result.reason {
        DifferenceReason::NoChange => PlanAction::NoOp,
        DifferenceReason::RemoteOnly => PlanAction::Download,
        DifferenceReason::LocalOnly => PlanAction::Blocked(ConflictReason::LocalChanged),
        DifferenceReason::MissingBase => PlanAction::Blocked(ConflictReason::ExistingLocalData),
        DifferenceReason::IdentityChanged => PlanAction::Blocked(if remote_changed {
            ConflictReason::ConcurrentIdentityChanges
        } else {
            ConflictReason::LocalIdentityChanged
        }),
        DifferenceReason::Unsupported => PlanAction::Blocked(ConflictReason::LocalTypeMismatch),
        DifferenceReason::ContentNotCompared => {
            PlanAction::Blocked(ConflictReason::ConcurrentChanges)
        }
        DifferenceReason::ObservedAbsence if !remote_changed => PlanAction::Download,
        DifferenceReason::ObservedAbsence => {
            PlanAction::Blocked(ConflictReason::LocalMissingRemoteChanged)
        }
        _ => PlanAction::DeferredUnknown,
    }
}
/// Refines only same-object concurrent content candidates, after external proof.
/// Unknown proof cannot remain a user-visible proven conflict or authorize a write.
pub(crate) fn refine_content(
    remote_change: ContentComparison,
    comparison: ContentComparison,
) -> PlanAction {
    let result = classify_differences(&DifferenceInput {
        base: BaseEvidence::Confirmed,
        local: present(ContentComparison::Different),
        remote: present(remote_change),
        current_content: comparison,
    });
    match result.kind {
        DifferenceKind::LocalChanged => PlanAction::Blocked(ConflictReason::LocalChanged),
        DifferenceKind::Equivalent => PlanAction::Equivalent,
        DifferenceKind::Conflict => PlanAction::Blocked(ConflictReason::ConcurrentChanges),
        _ => PlanAction::DeferredUnknown,
    }
}
/// Central projection for existing persisted download diagnostics. Structural path
/// checks remain in the planner; operation failure never becomes content precedence.
pub(crate) fn plan_diagnosis(action: &PlanAction, local: &LocalObservation) -> ChangeKind {
    match action {
        PlanAction::Classified { decision, .. } => match decision.kind {
            DifferenceKind::Conciliable | DifferenceKind::Equivalent => ChangeKind::Conciliable,
            DifferenceKind::LocalChanged => ChangeKind::LocalChanged,
            DifferenceKind::RemoteChanged => ChangeKind::RemoteChanged,
            DifferenceKind::Conflict if decision.reason == DifferenceReason::DivergentLocation => {
                ChangeKind::LocationConflict
            }
            DifferenceKind::Conflict => ChangeKind::ConcurrentChanges,
            DifferenceKind::Incompatible => ChangeKind::Unsupported,
            _ => ChangeKind::Unknown,
        },
        PlanAction::Equivalent => ChangeKind::Equivalent,
        PlanAction::NoOp => ChangeKind::Unchanged,
        PlanAction::Download | PlanAction::CreateDirectory | PlanAction::CreateLink => {
            if *local == LocalObservation::Absent {
                ChangeKind::LocalMissing
            } else {
                ChangeKind::RemoteChanged
            }
        }
        PlanAction::DeferredUnknown => ChangeKind::Unknown,
        PlanAction::Skipped(_) => ChangeKind::Unsupported,
        PlanAction::Blocked(reason) => match reason {
            ConflictReason::LocalChanged => ChangeKind::LocalChanged,
            ConflictReason::ConcurrentChanges => ChangeKind::ConcurrentChanges,
            ConflictReason::LocalIdentityChanged => ChangeKind::LocalIdentityChanged,
            ConflictReason::ConcurrentIdentityChanges => ChangeKind::ConcurrentIdentityChanges,
            ConflictReason::ExistingLocalData => ChangeKind::ExistingLocalData,
            ConflictReason::LocalMissingRemoteChanged => ChangeKind::LocalMissingRemoteChanged,
            _ => ChangeKind::Unsupported,
        },
    }
}
