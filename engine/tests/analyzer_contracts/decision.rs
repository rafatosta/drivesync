use super::support::{file, item_context};
use drivesync_engine::{
    analyzer::{
        AnalysisContext, AnalysisDecision, AnalysisDisposition, AnalysisItemContext,
        AnalysisItemDecision, DecisionTarget,
    },
    inventory::{
        AnalysisDimension, AnalysisFinding, AnalysisFindingStatus, AnalysisIntent, AnalysisOrigin,
        AnalysisReason, AnalysisRequirement, OperationDirection, OperationKind, SyncItemId,
    },
    models::{
        AccountId, FileEventAction, FileId, FileScope, LocalRelativePath, RemoteFileId,
        RemoteFileRef,
    },
};

fn context(action: FileEventAction, items: Vec<AnalysisItemContext>) -> AnalysisContext {
    AnalysisContext {
        file: file(action, FileScope::WholeSync),
        items,
    }
}

fn intent(direction: OperationDirection, kind: OperationKind) -> AnalysisIntent {
    AnalysisIntent { direction, kind }
}

#[test]
fn new_remote_file_without_inventory_item_has_download_create_decision_fixture() {
    let remote_file = file(
        FileEventAction::Created,
        FileScope::Remote(RemoteFileRef {
            account_id: AccountId("account-1".into()),
            file_id: RemoteFileId("remote-1".into()),
        }),
    );
    let input = AnalysisContext {
        file: remote_file,
        items: Vec::new(),
    };
    let expected = AnalysisDecision {
        context: input.clone(),
        items: vec![AnalysisItemDecision {
            target: DecisionTarget::Observation(FileId("detector-file-1".into())),
            disposition: AnalysisDisposition::ActionSelected,
            findings: vec![],
            requirements: vec![],
            intents: vec![intent(OperationDirection::Download, OperationKind::Create)],
        }],
    };

    assert_eq!(expected.context, input);
    assert_eq!(expected.items.len(), 1);
    assert_eq!(
        expected.items[0].target,
        DecisionTarget::Observation(FileId("detector-file-1".into()))
    );
    assert_eq!(
        expected.items[0].intents,
        vec![intent(OperationDirection::Download, OperationKind::Create)]
    );
}

#[test]
fn local_edit_with_known_inventory_revision_has_upload_update_decision_fixture() {
    let subject = item_context(9);
    let local_file = file(
        FileEventAction::Modified,
        FileScope::Local(LocalRelativePath::new(b"notes.txt".to_vec()).unwrap()),
    );
    let input = AnalysisContext {
        file: local_file,
        items: vec![subject],
    };
    let expected = AnalysisDecision {
        context: input.clone(),
        items: vec![AnalysisItemDecision {
            target: DecisionTarget::Item {
                id: SyncItemId("inventory-item-1".into()),
                revision: 9,
            },
            disposition: AnalysisDisposition::ActionSelected,
            findings: vec![],
            requirements: vec![],
            intents: vec![intent(
                OperationDirection::Upload,
                OperationKind::UpdateContent,
            )],
        }],
    };

    assert_eq!(expected.context, input);
    assert_eq!(
        expected.items[0].target,
        DecisionTarget::Item {
            id: SyncItemId("inventory-item-1".into()),
            revision: 9,
        }
    );
    assert_eq!(
        expected.items[0].intents[0].direction,
        OperationDirection::Upload
    );
    assert_eq!(
        expected.items[0].intents[0].kind,
        OperationKind::UpdateContent
    );
}

#[test]
fn rename_move_and_deletion_cases_use_semantic_intents() {
    let cases = [
        (
            FileEventAction::Renamed,
            OperationDirection::Upload,
            OperationKind::Rename,
        ),
        (
            FileEventAction::Moved,
            OperationDirection::Download,
            OperationKind::Move,
        ),
        (
            FileEventAction::Deleted,
            OperationDirection::Download,
            OperationKind::Trash,
        ),
    ];

    for (action, direction, kind) in cases {
        let input = context(action, vec![item_context(4)]);
        let expected = AnalysisDecision {
            context: input.clone(),
            items: vec![AnalysisItemDecision {
                target: DecisionTarget::Item {
                    id: SyncItemId("inventory-item-1".into()),
                    revision: 4,
                },
                disposition: AnalysisDisposition::ActionSelected,
                findings: vec![],
                requirements: vec![],
                intents: vec![intent(direction.clone(), kind.clone())],
            }],
        };

        assert_eq!(expected.context.file.event.action, action);
        assert_eq!(expected.items[0].intents, vec![intent(direction, kind)]);
    }
}

#[test]
fn conflict_and_missing_evidence_are_distinct_decision_fixtures() {
    let input = context(FileEventAction::Modified, vec![item_context(5)]);
    let conflict = AnalysisDecision {
        context: input.clone(),
        items: vec![AnalysisItemDecision {
            target: DecisionTarget::Item {
                id: SyncItemId("inventory-item-1".into()),
                revision: 5,
            },
            disposition: AnalysisDisposition::Conflict,
            findings: vec![AnalysisFinding {
                origin: AnalysisOrigin::Conflicts,
                dimension: AnalysisDimension::Content,
                status: AnalysisFindingStatus::Divergent,
                reason: AnalysisReason::ConcurrentChanges,
            }],
            requirements: vec![],
            intents: vec![],
        }],
    };
    let needs_evidence = AnalysisItemDecision {
        target: DecisionTarget::Item {
            id: SyncItemId("inventory-item-1".into()),
            revision: 5,
        },
        disposition: AnalysisDisposition::NeedsEvidence,
        findings: vec![],
        requirements: vec![AnalysisRequirement::VerifyContent],
        intents: vec![],
    };

    assert_eq!(conflict.items[0].disposition, AnalysisDisposition::Conflict);
    assert_eq!(
        conflict.items[0].findings[0].reason,
        AnalysisReason::ConcurrentChanges
    );
    assert_eq!(
        needs_evidence.disposition,
        AnalysisDisposition::NeedsEvidence
    );
    assert_eq!(
        needs_evidence.requirements,
        vec![AnalysisRequirement::VerifyContent]
    );
}

#[test]
fn synchronization_scope_target_is_available_without_fabricating_item_identity() {
    let expected = AnalysisItemDecision {
        target: DecisionTarget::Synchronization,
        disposition: AnalysisDisposition::NeedsEvidence,
        findings: vec![],
        requirements: vec![AnalysisRequirement::ObserveRemote],
        intents: vec![],
    };

    assert_eq!(expected.target, DecisionTarget::Synchronization);
    assert_eq!(
        expected.requirements,
        vec![AnalysisRequirement::ObserveRemote]
    );
}
