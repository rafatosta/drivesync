//! Table-driven B/L/R oracles, including non-conflict operations and missing proof.
use super::*;
fn content() -> SideDifference {
    SideDifference::Present {
        content: ContentComparison::Different,
        name: AttributeChange::Unchanged,
        parent: AttributeChange::Unchanged,
    }
}
fn location(name: Option<&str>, parent: Option<&str>) -> SideDifference {
    SideDifference::Present {
        content: ContentComparison::Equal,
        name: name.map_or(AttributeChange::Unchanged, |s| {
            AttributeChange::Changed(s.into())
        }),
        parent: parent.map_or(AttributeChange::Unchanged, |s| {
            AttributeChange::Changed(s.into())
        }),
    }
}
fn input(local: SideDifference, remote: SideDifference) -> DifferenceInput {
    DifferenceInput {
        base: BaseEvidence::Confirmed,
        local,
        remote,
        current_content: ContentComparison::Different,
    }
}
#[test]
fn c01_c03_c04_and_p01_three_way_matrix() {
    use DifferenceKind as K;
    use DifferenceReason as R;
    use Disposition as D;
    let u = SideDifference::unchanged();
    let d = SideDifference::Deleted;
    let rows = [
        (
            u.clone(),
            u.clone(),
            K::Unchanged,
            D::Synchronize,
            R::NoChange,
        ),
        (
            content(),
            u.clone(),
            K::LocalChanged,
            D::Synchronize,
            R::LocalOnly,
        ),
        (
            u.clone(),
            content(),
            K::RemoteChanged,
            D::Synchronize,
            R::RemoteOnly,
        ),
        (
            content(),
            content(),
            K::Conflict,
            D::Preserve,
            R::DivergentContent,
        ),
        (
            content(),
            d.clone(),
            K::Conflict,
            D::Preserve,
            R::DeleteModify,
        ),
        (
            d.clone(),
            content(),
            K::Conflict,
            D::Preserve,
            R::DeleteModify,
        ),
        (
            u.clone(),
            d.clone(),
            K::Unknown,
            D::Preserve,
            R::DeletionRequiresPolicy,
        ),
        (
            d.clone(),
            u,
            K::Unknown,
            D::Preserve,
            R::DeletionRequiresPolicy,
        ),
        (d.clone(), d, K::Equivalent, D::Reconcile, R::BothDeleted),
    ];
    for (local, remote, kind, disposition, reason) in rows {
        let evidence = input(local, remote);
        let expected = DifferenceDecision {
            kind,
            disposition,
            reason,
        };
        for _ in 0..3 {
            assert_eq!(classify_differences(&evidence), expected);
        }
        let encoded = serde_json::to_string(&evidence).unwrap();
        let reopened = serde_json::from_str(&encoded).unwrap();
        assert_eq!(classify_differences(&reopened), expected);
    }
}
#[test]
fn c09_c12_independent_changes_are_conciliable_in_both_directions() {
    for delta in [
        location(Some("renamed.txt"), None),
        location(None, Some("new-parent")),
    ] {
        for (l, r) in [(delta.clone(), content()), (content(), delta)] {
            let decision = classify_differences(&input(l, r));
            assert_eq!(decision.kind, DifferenceKind::Conciliable);
            assert_eq!(decision.disposition, Disposition::Reconcile);
            assert_eq!(decision.reason, DifferenceReason::IndependentChanges);
        }
    }
    let combined = input(location(Some("name"), None), location(None, Some("parent")));
    assert_eq!(
        classify_differences(&combined).kind,
        DifferenceKind::Conciliable
    );
}
#[test]
fn c21_equal_content_and_matching_locations_converge_without_false_conflict() {
    let mut evidence = input(content(), content());
    evidence.current_content = ContentComparison::Equal;
    assert_eq!(
        classify_differences(&evidence).kind,
        DifferenceKind::Equivalent
    );
    for side in [
        location(Some("same-name"), None),
        location(None, Some("same-parent")),
    ] {
        assert_eq!(
            classify_differences(&input(side.clone(), side)).kind,
            DifferenceKind::Equivalent
        );
    }
    let divergent = input(location(Some("a"), None), location(Some("b"), None));
    assert_eq!(
        classify_differences(&divergent).reason,
        DifferenceReason::DivergentLocation
    );
}
#[test]
fn unknown_content_is_not_a_proven_conflict_and_identity_overrides_equal_bytes() {
    let mut evidence = input(content(), content());
    evidence.current_content = ContentComparison::Unknown;
    assert_eq!(
        classify_differences(&evidence).reason,
        DifferenceReason::ContentNotCompared
    );
    assert_eq!(
        classify_differences(&evidence).kind,
        DifferenceKind::Unknown
    );
    evidence.current_content = ContentComparison::Equal;
    evidence.local = SideDifference::Replaced;
    assert_eq!(
        classify_differences(&evidence).reason,
        DifferenceReason::IdentityChanged
    );
}
#[test]
fn absence_offline_and_permission_failure_never_prove_deletion() {
    for side in [
        SideDifference::Absent,
        SideDifference::Unknown,
        SideDifference::Unavailable(ObservationFailure::Offline),
        SideDifference::Unavailable(ObservationFailure::AccessDenied),
        SideDifference::Unavailable(ObservationFailure::NotListed),
        SideDifference::Unavailable(ObservationFailure::ReadFailed),
    ] {
        for (l, r) in [(side.clone(), content()), (content(), side)] {
            let decision = classify_differences(&input(l, r));
            assert_eq!(decision.kind, DifferenceKind::Unknown);
            assert_ne!(decision.reason, DifferenceReason::DeleteModify);
            assert_ne!(decision.disposition, Disposition::Synchronize);
        }
    }
}
#[test]
fn no_base_equal_preexisting_files_do_not_authorize_adoption() {
    let mut evidence = input(SideDifference::unchanged(), SideDifference::unchanged());
    evidence.base = BaseEvidence::Absent;
    evidence.current_content = ContentComparison::Equal;
    assert_eq!(
        classify_differences(&evidence).reason,
        DifferenceReason::MissingBase
    );
    evidence.local = SideDifference::Absent;
    assert_eq!(
        classify_differences(&evidence).kind,
        DifferenceKind::RemoteChanged
    );
    evidence.base = BaseEvidence::Invalid;
    assert_eq!(
        classify_differences(&evidence).reason,
        DifferenceReason::InvalidBase
    );
}
#[test]
fn incomplete_attributes_and_content_never_authorize_reconciliation() {
    for side in [
        SideDifference::Present {
            content: ContentComparison::Unknown,
            name: AttributeChange::Unchanged,
            parent: AttributeChange::Unchanged,
        },
        SideDifference::Present {
            content: ContentComparison::Equal,
            name: AttributeChange::Unknown,
            parent: AttributeChange::Unchanged,
        },
        location(Some(""), None),
    ] {
        assert_eq!(
            classify_differences(&input(side, content())).disposition,
            Disposition::Unknown
        );
    }
}

#[test]
fn c08_c11_confirmed_deletion_conflicts_with_location_changes_in_both_directions() {
    for modified in [
        location(Some("new-name"), None),
        location(None, Some("new-parent")),
    ] {
        for (local, remote) in [
            (modified.clone(), SideDifference::Deleted),
            (SideDifference::Deleted, modified),
        ] {
            let result = classify_differences(&input(local, remote));
            assert_eq!(result.reason, DifferenceReason::DeleteModify);
            assert_eq!(result.kind, DifferenceKind::Conflict);
            assert_eq!(result.disposition, Disposition::Preserve);
        }
    }
}
