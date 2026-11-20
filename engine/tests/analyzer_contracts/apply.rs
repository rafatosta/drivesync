//! Structural Apply artifact tests; behavior is specified separately.
use drivesync_engine::{
    analyzer::{AnalysisItemResult, AnalysisResult, AnalysisSkipReason, ApplyOutcome, DecisionTarget, NonEmpty},
    inventory::{AnalysisRequirement, SyncItemId},
    operations::ExecutionId,
};

#[test]
fn pending_execution_is_distinct_from_recovery() {
    let id = ExecutionId::new("operation-1".into()).unwrap();
    let pending = ApplyOutcome::PendingExecution { operations: NonEmpty::new(id.clone(), vec![]) };
    let recovery = ApplyOutcome::PendingRecovery { operations: NonEmpty::new(id, vec![]) };
    assert_ne!(pending, recovery);
}

#[test]
fn mixed_terminal_receipt_preserves_target_outcomes() {
    let items = vec![
        AnalysisItemResult { target: DecisionTarget::Item { id: SyncItemId("one".into()), revision: 7 }, outcome: ApplyOutcome::ConflictRecorded },
        AnalysisItemResult { target: DecisionTarget::Synchronization, outcome: ApplyOutcome::EvidenceRequested { requirements: NonEmpty::new(AnalysisRequirement::ObserveLocal, vec![]) } },
    ];
    let result = AnalysisResult::Completed(items.clone());
    assert_eq!(result, AnalysisResult::Completed(items));
}

#[test]
fn duplicate_and_obsolete_are_distinct() {
    assert_ne!(AnalysisResult::Skipped(AnalysisSkipReason::Duplicate), AnalysisResult::Skipped(AnalysisSkipReason::Obsolete));
}
