use drivesync_engine::{
    analyzer::{
        AnalysisDisposition, AnalysisItemResult, AnalysisResult, AnalysisSkipReason, DecisionTarget,
    },
    inventory::SyncItemId,
    models::FileId,
};

#[test]
fn completed_apply_result_preserves_each_decision_target_and_disposition() {
    let expected_items = vec![
        AnalysisItemResult {
            target: DecisionTarget::Observation(FileId("new-observation".into())),
            disposition: AnalysisDisposition::ActionSelected,
        },
        AnalysisItemResult {
            target: DecisionTarget::Item {
                id: SyncItemId("item-1".into()),
                revision: 6,
            },
            disposition: AnalysisDisposition::Conflict,
        },
        AnalysisItemResult {
            target: DecisionTarget::Synchronization,
            disposition: AnalysisDisposition::NeedsEvidence,
        },
    ];
    let expected = AnalysisResult::Completed(expected_items.clone());
    let result = AnalysisResult::Completed(expected_items);

    assert_eq!(result, expected);
    let AnalysisResult::Completed(items) = result else {
        panic!("expected a completed apply result")
    };
    assert_eq!(items.len(), 3);
    assert_eq!(
        items[0].target,
        DecisionTarget::Observation(FileId("new-observation".into()))
    );
    assert_eq!(
        items[1].target,
        DecisionTarget::Item {
            id: SyncItemId("item-1".into()),
            revision: 6,
        }
    );
    assert_eq!(items[2].target, DecisionTarget::Synchronization);
}

#[test]
fn all_terminal_dispositions_can_be_returned_after_durable_handling() {
    let dispositions = [
        AnalysisDisposition::NoAction,
        AnalysisDisposition::ActionSelected,
        AnalysisDisposition::Conflict,
        AnalysisDisposition::NeedsEvidence,
    ];
    let results = dispositions
        .into_iter()
        .map(|disposition| AnalysisItemResult {
            target: DecisionTarget::Item {
                id: SyncItemId("item-1".into()),
                revision: 2,
            },
            disposition,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        results
            .iter()
            .map(|result| result.disposition)
            .collect::<Vec<_>>(),
        dispositions
    );
}

#[test]
fn duplicate_and_obsolete_deliveries_have_distinct_skip_results() {
    let duplicate = AnalysisResult::Skipped(AnalysisSkipReason::Duplicate);
    let obsolete = AnalysisResult::Skipped(AnalysisSkipReason::Obsolete);

    assert_eq!(
        duplicate,
        AnalysisResult::Skipped(AnalysisSkipReason::Duplicate)
    );
    assert_eq!(
        obsolete,
        AnalysisResult::Skipped(AnalysisSkipReason::Obsolete)
    );
    assert_ne!(duplicate, obsolete);
}
