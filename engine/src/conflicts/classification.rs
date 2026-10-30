//! Deterministic three-way rules over normalized identity/content/location evidence.
use super::{
    AttributeChange as A, BaseEvidence, ContentComparison as C, DifferenceDecision as Decision,
    DifferenceInput, DifferenceKind as K, DifferenceReason as R, Disposition as D,
    SideDifference as S,
};
fn decision(kind: K, disposition: D, reason: R) -> Decision {
    Decision {
        kind,
        disposition,
        reason,
    }
}
fn incomplete(side: &S) -> bool {
    match side {
        S::Unknown => true,
        S::Present {
            content,
            name,
            parent,
        } => {
            *content == C::Unknown
                || [name, parent].iter().any(|a| {
                    matches!(a, A::Unknown) || matches!(a, A::Changed(target) if target.is_empty())
                })
        }
        _ => false,
    }
}
fn changed(side: &S) -> bool {
    *side != S::unchanged()
}
fn incompatible(a: &A, b: &A) -> bool {
    matches!((a, b), (A::Changed(a), A::Changed(b)) if a != b)
}
/// Classifies scoped differences from confirmed B without I/O or side effects.
///
/// Identity substitution overrides equal content. Missing observations never prove
/// deletion. Independent location/content changes are conciliable candidates;
/// no executor, path safety or preservation guarantee is implied by that result.
/// Incomplete evidence suspends dependent actions. Repeated inputs are idempotent.
pub fn classify_differences(input: &DifferenceInput) -> Decision {
    let DifferenceInput {
        base,
        local: l,
        remote: r,
        current_content,
    } = input;
    if *base == BaseEvidence::Invalid {
        return decision(K::Unknown, D::Unknown, R::InvalidBase);
    }
    for side in [l, r] {
        if let S::Unavailable(cause) = side {
            return decision(K::Unknown, D::Unknown, R::Unavailable(*cause));
        }
    }
    if incomplete(l) || incomplete(r) {
        return decision(K::Unknown, D::Unknown, R::IncompleteEvidence);
    }
    if matches!(l, S::Unsupported) || matches!(r, S::Unsupported) {
        return decision(K::Incompatible, D::Block, R::Unsupported);
    }
    if matches!(l, S::Replaced) || matches!(r, S::Replaced) {
        return decision(K::Incompatible, D::Block, R::IdentityChanged);
    }
    if *base == BaseEvidence::Absent {
        return match (l, r) {
            (S::Absent, S::Absent) => decision(K::Unchanged, D::Synchronize, R::NoChange),
            (S::Absent, S::Present { .. }) => {
                decision(K::RemoteChanged, D::Synchronize, R::RemoteOnly)
            }
            (S::Present { .. }, S::Absent) => {
                decision(K::LocalChanged, D::Synchronize, R::LocalOnly)
            }
            _ => decision(K::Unknown, D::Preserve, R::MissingBase),
        };
    }
    match (l, r) {
        (S::Deleted, S::Deleted) => return decision(K::Equivalent, D::Reconcile, R::BothDeleted),
        (S::Deleted, S::Present { .. }) | (S::Present { .. }, S::Deleted) => {
            let other = if *l == S::Deleted { r } else { l };
            return if changed(other) {
                decision(K::Conflict, D::Preserve, R::DeleteModify)
            } else {
                decision(K::Unknown, D::Preserve, R::DeletionRequiresPolicy)
            };
        }
        (S::Absent, _) | (_, S::Absent) | (S::Deleted, _) | (_, S::Deleted) => {
            return decision(K::Unknown, D::Preserve, R::ObservedAbsence);
        }
        _ => {}
    }
    let (
        S::Present {
            content: lc,
            name: ln,
            parent: lp,
        },
        S::Present {
            content: rc,
            name: rn,
            parent: rp,
        },
    ) = (l, r)
    else {
        return decision(K::Unknown, D::Unknown, R::IncompleteEvidence);
    };
    if incompatible(ln, rn) || incompatible(lp, rp) {
        return decision(K::Conflict, D::Preserve, R::DivergentLocation);
    }
    if *lc == C::Different && *rc == C::Different {
        match current_content {
            C::Different => return decision(K::Conflict, D::Preserve, R::DivergentContent),
            C::Unknown => return decision(K::Unknown, D::Preserve, R::ContentNotCompared),
            C::Equal => {}
        }
    }
    match (changed(l), changed(r)) {
        (false, false) => decision(K::Unchanged, D::Synchronize, R::NoChange),
        (true, false) => decision(K::LocalChanged, D::Synchronize, R::LocalOnly),
        (false, true) => decision(K::RemoteChanged, D::Synchronize, R::RemoteOnly),
        (true, true) if l == r && (*lc == C::Equal || *current_content == C::Equal) => {
            decision(K::Equivalent, D::Reconcile, R::EqualContent)
        }
        _ => decision(K::Conciliable, D::Reconcile, R::IndependentChanges),
    }
}
