//! Structural Decision artifact tests (behavior is in decision_behavior.rs).
use drivesync_engine::{
    analyzer::{AnalysisDisposition, DecisionBlock, DecisionOutcome, NonEmpty},
    inventory::{AnalysisIntent, AnalysisRequirement, OperationDirection, OperationKind},
};

#[test]
fn empty_payload_cannot_be_constructed() {
    assert!(NonEmpty::<AnalysisIntent>::from_vec(vec![]).is_none());
    assert!(NonEmpty::<AnalysisRequirement>::from_vec(vec![]).is_none());
}

#[test]
fn nonempty_preserves_order() {
    let values = NonEmpty::new(AnalysisRequirement::ObserveLocal, vec![AnalysisRequirement::ObserveRemote]);
    assert_eq!(values.as_slice(), &[AnalysisRequirement::ObserveLocal, AnalysisRequirement::ObserveRemote]);
    assert_eq!(values.into_vec().len(), 2);
}

#[test]
fn semantic_outcomes_map_to_distinct_handled_dispositions() {
    let cases = [
        (DecisionOutcome::NoAction, AnalysisDisposition::NoAction),
        (DecisionOutcome::EstablishBaseline, AnalysisDisposition::BaselineEstablished),
        (DecisionOutcome::Obsolete, AnalysisDisposition::Obsolete),
        (DecisionOutcome::Blocked { reason: DecisionBlock::UnsupportedKind }, AnalysisDisposition::Blocked),
        (DecisionOutcome::ActionSelected {
            intents: NonEmpty::new(AnalysisIntent { direction: OperationDirection::Download, kind: OperationKind::Create }, vec![]),
        }, AnalysisDisposition::ActionSelected),
        (DecisionOutcome::Conflict { requirements: vec![] }, AnalysisDisposition::Conflict),
        (DecisionOutcome::NeedsEvidence {
            requirements: NonEmpty::new(AnalysisRequirement::VerifyContent, vec![]),
        }, AnalysisDisposition::NeedsEvidence),
    ];
    for (outcome, expected) in cases {
        assert_eq!(outcome.disposition(), expected);
    }
}
