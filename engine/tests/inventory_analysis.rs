//! Durable analysis contracts through the public inventory API and real temporary SQLite.
use drivesync_engine::{
    inventory::*,
    models::{self as m, AccountId, SyncId},
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "inventory-analysis-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn request(&self) -> OpenInventoryRequest {
        OpenInventoryRequest {
            local_root: self.0.clone(),
            expected: SyncIdentity {
                sync_id: SyncId("sync".into()),
                account_id: AccountId("account".into()),
                remote_root_id: "root".into(),
            },
            create_if_missing: true,
        }
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn remote_file(event: &str, id: &str, name: m::MetadataValue<String>) -> m::File {
    let remote = m::RemoteFile {
        reference: m::RemoteFileRef {
            account_id: AccountId("account".into()),
            file_id: m::RemoteFileId(id.into()),
        },
        provider: m::ProviderId("google".into()),
        name: name.clone(),
        kind: m::MetadataValue::Known(m::RemoteEntryKind::File),
        mime_type: m::MetadataValue::NotCollected,
        parent_ids: m::MetadataValue::Known(vec![m::RemoteFileId("root".into())]),
        trashed: m::MetadataValue::Known(false),
        version: m::MetadataValue::Known("1".into()),
        created_time: m::MetadataValue::NotCollected,
        modified_time: m::MetadataValue::NotCollected,
        size: m::MetadataValue::Known(u64::MAX),
        md5_checksum: m::MetadataValue::NotCollected,
        sha256_checksum: m::MetadataValue::NotCollected,
        provider_data: m::MetadataValue::NotCollected,
    };
    let reference = remote.reference.clone();
    m::File {
        id: m::FileId(event.into()),
        sync_id: SyncId("sync".into()),
        observed_at: m::Timestamp { unix_millis: 10 },
        scope: m::FileScope::Remote(reference.clone()),
        coverage: m::ObservationCoverage::Partial,
        event: m::FileEvent {
            action: m::FileEventAction::StateChanged,
            local: None,
            local_presence: None,
            remote: Some(m::RemoteObservation {
                source: m::RemoteObservationSource::Changes,
                removed: Some(false),
                time: None,
                change_type: None,
                drive_id: None,
            }),
            uncertainty: None,
            previous: None,
            current: Some(m::FileLocation::Remote {
                reference,
                parent_id: Some(m::RemoteFileId("root".into())),
                name: if let m::MetadataValue::Known(name) = name {
                    Some(name)
                } else {
                    None
                },
            }),
        },
        data: Some(m::FileData::Remote(remote)),
    }
}
fn remote_request(
    file: m::File,
    target: ObservationTarget,
    revision: Option<u64>,
) -> RecordRemoteObservation {
    RecordRemoteObservation {
        selector: target,
        expected_revision: revision,
        observation: RemoteObservation {
            observed: file,
            presence: PresenceState::Present,
            proof: ObservationProof {
                source: ObservationSource::RemoteChange,
                observed_at: m::Timestamp { unix_millis: 10 },
                scan_or_generation: None,
                complete_scope: false,
            },
        },
    }
}

fn setup() -> (Tree, InventoryManager, SyncItem) {
    let tree = Tree::new();
    let inventory = InventoryManager::open(tree.request()).unwrap().inventory;
    let item = inventory
        .record_remote(remote_request(
            remote_file("event", "remote", m::MetadataValue::Known("file".into())),
            ObservationTarget::NewItem,
            None,
        ))
        .unwrap()
        .item;
    (tree, inventory, item)
}
fn request(item: SyncItem) -> RecordAnalysisRequest {
    RecordAnalysisRequest {
        expected_sync_id: SyncId("sync".into()),
        expected_item: item,
        recorded_at: m::Timestamp { unix_millis: 100 },
        evidence: AnalysisEvidence {
            baseline_confirmed: false,
            local_scope_complete: false,
            remote_scope_complete: true,
            local_content: EvidenceRelation::Unknown,
            remote_content: EvidenceRelation::Unknown,
            current_content: EvidenceRelation::Unknown,
            local_parent: None,
            current_parents: EvidenceRelation::Unknown,
        },
        assessment: NewAnalysisAssessment {
            plan: None,
            disposition: AnalysisDisposition::PendingEvidence,
            findings: vec![AnalysisFinding {
                origin: AnalysisOrigin::Conflicts,
                dimension: AnalysisDimension::Presence,
                status: AnalysisFindingStatus::Unknown,
                reason: AnalysisReason::InsufficientEvidence,
            }],
            requirements: vec![AnalysisRequirement::ObserveLocal],
            intents: vec![AnalysisIntent {
                direction: OperationDirection::Download,
                kind: OperationKind::Create,
            }],
        },
    }
}
fn operations(inventory: &InventoryManager, item: &SyncItemId) -> Vec<SyncOperation> {
    inventory
        .list_operations(ListOperationsRequest {
            filter: OperationFilter {
                item_id: Some(item.clone()),
                states: vec![],
                direction: None,
                eligible_before: None,
            },
            pagination: Pagination::default(),
        })
        .unwrap()
        .page
        .entries
}
fn current(inventory: &InventoryManager, id: &SyncItemId) -> SyncItem {
    inventory
        .get_item(GetItemRequest {
            selector: ItemSelector::Id(id.clone()),
        })
        .unwrap()
        .item
        .unwrap()
}
fn analyses(inventory: &InventoryManager, current_only: bool) -> Vec<RecordedAnalysis> {
    inventory
        .list_analyses(ListAnalysesRequest {
            filter: AnalysisFilter {
                current_only,
                ..Default::default()
            },
            pagination: Pagination::default(),
        })
        .unwrap()
        .page
        .entries
}
#[test]
fn analysis_and_blocked_operations_survive_reopen_and_exact_retries_do_not_duplicate() {
    let (tree, inventory, item) = setup();
    let mut req = request(item.clone());
    let recorded = inventory.record_analysis(req.clone()).unwrap().analysis;
    assert_eq!(recorded.analyzed_item_revision, item.revision);
    assert_eq!(recorded.resulting_item_revision, item.revision + 1);
    assert!(!recorded.stale);
    assert_eq!(recorded.input_fingerprint.len(), 32);
    assert_eq!(recorded.decisions.len(), 1);
    let op = operations(&inventory, &item.id).remove(0);
    assert_eq!(op.id, recorded.decisions[0].operation_id);
    assert_eq!(op.state, OperationState::Blocked);
    assert_eq!(
        op.last_error_kind.as_deref(),
        Some("AnalyzerEvidencePending")
    );
    assert_eq!(op.base_item_revision, item.revision + 1);
    assert_eq!(current(&inventory, &item.id).baseline, item.baseline);
    req.recorded_at.unix_millis += 1;
    assert_eq!(
        inventory.record_analysis(req.clone()).unwrap().analysis,
        recorded
    );
    req.expected_item = current(&inventory, &item.id);
    assert_eq!(inventory.record_analysis(req).unwrap().analysis, recorded);
    drop(inventory);
    let reopened = InventoryManager::open(tree.request()).unwrap().inventory;
    assert_eq!(
        reopened
            .get_analysis(GetAnalysisRequest {
                selector: AnalysisSelector::Id(recorded.id.clone())
            })
            .unwrap()
            .analysis,
        Some(recorded)
    );
    assert_eq!(operations(&reopened, &item.id).len(), 1);
    assert_eq!(analyses(&reopened, false).len(), 1);
}
#[test]
fn generic_queue_apis_cannot_release_incomplete_analysis_commands() {
    let (_tree, inventory, item) = setup();
    let result = inventory.record_analysis(request(item.clone())).unwrap();
    let op = &result.analysis.decisions[0].operation_id;
    assert_eq!(
        inventory
            .update_operation(UpdateOperationRequest {
                operation_id: op.clone(),
                expected_state: OperationState::Blocked,
                expected_attempt_count: 0,
                transition: OperationTransition::Requeue {
                    reason: "retry".into()
                }
            })
            .unwrap_err()
            .kind,
        InventoryError::InvalidTransition
    );
    assert_eq!(
        inventory
            .claim_operation(ClaimOperationRequest {
                operation_id: op.clone(),
                expected_state: OperationState::Blocked,
                claimed_at: m::Timestamp { unix_millis: 101 }
            })
            .unwrap_err()
            .kind,
        InventoryError::InvalidTransition
    );
    assert_eq!(
        operations(&inventory, &item.id)[0].state,
        OperationState::Blocked
    );
}
#[test]
fn observation_revision_changes_mark_receipts_stale_without_deleting_history() {
    let (_tree, inventory, item) = setup();
    let req = request(item.clone());
    let old = inventory.record_analysis(req.clone()).unwrap().analysis;
    inventory
        .update_item_status(UpdateItemStatusRequest {
            item_id: item.id.clone(),
            expected_revision: old.resulting_item_revision,
            change: ItemStatusChange {
                status: SyncStatus::Error,
                conflict_reason: FieldUpdate::Clear,
            },
        })
        .unwrap();
    let receipt = inventory
        .get_analysis(GetAnalysisRequest {
            selector: AnalysisSelector::LatestForItem(item.id.clone()),
        })
        .unwrap()
        .analysis
        .unwrap();
    assert!(receipt.stale);
    assert!(analyses(&inventory, true).is_empty());
    assert_eq!(analyses(&inventory, false).len(), 1);
    let retry = inventory.record_analysis(req).unwrap().analysis;
    assert_eq!(retry.id, old.id);
    assert!(retry.stale);
    assert_eq!(operations(&inventory, &item.id).len(), 1);
}
#[test]
fn scope_revision_and_snapshot_mismatches_have_no_partial_writes() {
    let (_tree, inventory, item) = setup();
    let mut req = request(item.clone());
    req.expected_sync_id = SyncId("other".into());
    assert_eq!(
        inventory.record_analysis(req).unwrap_err().kind,
        InventoryError::InvalidIdentity
    );
    let mut req = request(item.clone());
    req.expected_item.remote.as_mut().unwrap().name = m::MetadataValue::Known("forged".into());
    assert_eq!(
        inventory.record_analysis(req).unwrap_err().kind,
        InventoryError::VersionMismatch
    );
    let mut req = request(item.clone());
    req.expected_item.revision += 1;
    assert_eq!(
        inventory.record_analysis(req).unwrap_err().kind,
        InventoryError::VersionMismatch
    );
    assert!(analyses(&inventory, false).is_empty());
    assert!(operations(&inventory, &item.id).is_empty());
    assert_eq!(current(&inventory, &item.id), item);
}
#[test]
fn injected_decision_insert_failure_rolls_back_receipt_status_revision_and_queue() {
    let (tree, inventory, item) = setup();
    let db = rusqlite::Connection::open(tree.0.join(".drivesync/inventory.sqlite3")).unwrap();
    db.execute_batch("CREATE TRIGGER fail_decision BEFORE INSERT ON analysis_decisions BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert!(inventory.record_analysis(request(item.clone())).is_err());
    assert_eq!(current(&inventory, &item.id), item);
    assert!(analyses(&inventory, false).is_empty());
    assert!(operations(&inventory, &item.id).is_empty());
    db.execute_batch("DROP TRIGGER fail_decision;").unwrap();
    assert!(
        !inventory
            .record_analysis(request(item))
            .unwrap()
            .analysis
            .stale
    );
}
#[test]
fn two_connections_concurrently_record_one_receipt_and_one_operation() {
    let (tree, inventory, item) = setup();
    let other = InventoryManager::open(tree.request()).unwrap().inventory;
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let req = request(item.clone());
    let (a, b) = std::thread::scope(|scope| {
        let barrier2 = barrier.clone();
        let req2 = req.clone();
        let a = scope.spawn(|| {
            barrier.wait();
            inventory.record_analysis(req)
        });
        let b = scope.spawn(move || {
            barrier2.wait();
            other.record_analysis(req2)
        });
        (
            a.join().unwrap().unwrap().analysis,
            b.join().unwrap().unwrap().analysis,
        )
    });
    assert_eq!(a, b);
    assert_eq!(analyses(&inventory, false).len(), 1);
    assert_eq!(operations(&inventory, &item.id).len(), 1);
}
#[test]
fn running_operation_prevents_reanalysis_from_invalidating_its_revision() {
    let (_tree, inventory, item) = setup();
    let op = inventory
        .enqueue_operation(EnqueueOperationRequest {
            item_id: item.id.clone(),
            expected_item_revision: item.revision,
            spec: NewSyncOperation {
                direction: OperationDirection::Download,
                kind: OperationKind::Create,
                expected_remote_version: None,
                created_at: m::Timestamp { unix_millis: 10 },
                deduplication_key: None,
            },
        })
        .unwrap()
        .operation;
    inventory
        .claim_operation(ClaimOperationRequest {
            operation_id: op.id,
            expected_state: OperationState::Pending,
            claimed_at: m::Timestamp { unix_millis: 11 },
        })
        .unwrap();
    assert_eq!(
        inventory
            .record_analysis(request(item.clone()))
            .unwrap_err()
            .kind,
        InventoryError::InvalidTransition
    );
    assert_eq!(current(&inventory, &item.id), item);
    assert!(analyses(&inventory, false).is_empty());
}
#[test]
fn conflict_facts_are_durable_without_inventing_actions_or_confirming_baseline() {
    let (tree, inventory, item) = setup();
    let mut req = request(item.clone());
    req.assessment.disposition = AnalysisDisposition::PendingUserDecision;
    req.assessment.findings[0].status = AnalysisFindingStatus::Divergent;
    req.assessment.findings[0].reason = AnalysisReason::ConcurrentChanges;
    req.assessment.intents.clear();
    let record = inventory.record_analysis(req).unwrap().analysis;
    assert_eq!(current(&inventory, &item.id).status, SyncStatus::Conflict);
    assert!(operations(&inventory, &item.id).is_empty());
    assert_eq!(current(&inventory, &item.id).baseline, item.baseline);
    drop(inventory);
    let reopened = InventoryManager::open(tree.request()).unwrap().inventory;
    assert_eq!(analyses(&reopened, true), vec![record]);
}
#[test]
fn changed_analysis_of_the_same_input_revision_is_rejected_and_filters_bind_cursors() {
    let (_tree, inventory, item) = setup();
    let req = request(item.clone());
    inventory.record_analysis(req.clone()).unwrap();
    let mut changed = req;
    changed.evidence.remote_scope_complete = false;
    assert_eq!(
        inventory.record_analysis(changed).unwrap_err().kind,
        InventoryError::VersionMismatch
    );
    let mut next = request(current(&inventory, &item.id));
    next.evidence.remote_scope_complete = false;
    inventory.record_analysis(next).unwrap();
    let first = inventory
        .list_analyses(ListAnalysesRequest {
            filter: AnalysisFilter::default(),
            pagination: Pagination {
                limit: 1,
                after: None,
            },
        })
        .unwrap();
    assert_eq!(first.page.entries.len(), 1);
    assert!(first.page.entries[0].stale);
    let cursor = first.page.next_cursor.unwrap();
    let last = inventory
        .list_analyses(ListAnalysesRequest {
            filter: AnalysisFilter::default(),
            pagination: Pagination {
                limit: 1,
                after: Some(cursor.clone()),
            },
        })
        .unwrap();
    assert_eq!(last.page.entries.len(), 1);
    assert!(!last.page.entries[0].stale);
    assert_eq!(
        inventory
            .list_analyses(ListAnalysesRequest {
                filter: AnalysisFilter {
                    current_only: true,
                    ..Default::default()
                },
                pagination: Pagination {
                    limit: 1,
                    after: Some(cursor)
                }
            })
            .unwrap_err()
            .kind,
        InventoryError::InvalidCursor
    );
}
#[test]
fn invalid_assessments_and_duplicate_requirements_never_persist() {
    let (_tree, inventory, item) = setup();
    let mut req = request(item.clone());
    req.assessment.intents[0].kind = OperationKind::Delete;
    assert_eq!(
        inventory.record_analysis(req).unwrap_err().kind,
        InventoryError::InvalidArgument
    );
    let mut req = request(item.clone());
    req.assessment
        .requirements
        .push(AnalysisRequirement::ObserveLocal);
    assert_eq!(
        inventory.record_analysis(req).unwrap_err().kind,
        InventoryError::InvalidArgument
    );
    let mut req = request(item.clone());
    req.assessment.disposition = AnalysisDisposition::NoChanges;
    assert_eq!(
        inventory.record_analysis(req).unwrap_err().kind,
        InventoryError::InvalidArgument
    );
    assert!(analyses(&inventory, false).is_empty());
    assert!(operations(&inventory, &item.id).is_empty());
}

#[test]
fn typed_plan_is_atomic_durable_and_invalidated_by_related_observations() {
    let (tree, inv, item) = setup();
    let related = inv
        .record_remote(remote_request(
            remote_file(
                "parent-event",
                "parent",
                m::MetadataValue::Known("parent".into()),
            ),
            ObservationTarget::NewItem,
            None,
        ))
        .unwrap()
        .item;
    let mut req = request(item.clone());
    req.assessment.plan = Some(AnalysisPlan {
        restore_request: None,
        namespaces: vec![],
        guards: vec![
            ItemGuard::for_item(&item).unwrap(),
            ItemGuard::for_item(&related).unwrap(),
        ],
        steps: vec![PlannedStep {
            depends_on_steps: vec![],
            method: PlannedMethod::Download,
            command: None,
            blocked_by: vec![PlanBlock::Parent],
            depends_on: vec![related.id.clone()],
        }],
        manual: vec![],
    });
    let sql = rusqlite::Connection::open(tree.0.join(".drivesync/inventory.sqlite3")).unwrap();
    sql.execute_batch("CREATE TRIGGER fail_plan BEFORE INSERT ON analysis_plan_steps BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert!(inv.record_analysis(req.clone()).is_err());
    assert_eq!(current(&inv, &item.id).revision, item.revision);
    assert!(analyses(&inv, false).is_empty());
    sql.execute_batch("DROP TRIGGER fail_plan;").unwrap();
    let record = inv.record_analysis(req.clone()).unwrap().analysis;
    assert_eq!(record.plan, req.assessment.plan);
    assert_eq!(inv.record_analysis(req).unwrap().analysis, record);
    let mut change = remote_request(
        remote_file(
            "parent-change",
            "parent",
            m::MetadataValue::Known("different".into()),
        ),
        ObservationTarget::Existing(related.id),
        Some(related.revision),
    );
    change.observation.proof.observed_at = m::Timestamp { unix_millis: 11 };
    inv.record_remote(change).unwrap();
    assert!(
        inv.get_analysis(GetAnalysisRequest {
            selector: AnalysisSelector::Id(record.id.clone())
        })
        .unwrap()
        .analysis
        .unwrap()
        .stale
    );
    assert!(analyses(&inv, true).is_empty());
    drop(inv);
    drop(sql);
    let inv = InventoryManager::open(tree.request()).unwrap().inventory;
    assert_eq!(
        inv.get_analysis(GetAnalysisRequest {
            selector: AnalysisSelector::Id(record.id)
        })
        .unwrap()
        .analysis
        .unwrap()
        .plan,
        record.plan
    );
}

#[test]
fn a_new_destination_child_invalidates_a_plan_without_changing_its_source() {
    use drivesync_engine::operations::DirectoryRef;
    let (_tree, inv, item) = setup();
    let directory = DirectoryRef::Remote {
        provider_id: m::ProviderId("google".into()),
        reference: m::RemoteFileRef {
            account_id: AccountId("account".into()),
            file_id: m::RemoteFileId("root".into()),
        },
    };
    let mut req = request(item.clone());
    req.assessment.plan = Some(AnalysisPlan {
        restore_request: None,
        namespaces: vec![
            inv.get_namespace(GetNamespaceRequest { directory })
                .unwrap()
                .guard,
        ],
        guards: vec![ItemGuard::for_item(&item).unwrap()],
        steps: vec![PlannedStep {
            method: PlannedMethod::Download,
            command: None,
            blocked_by: vec![PlanBlock::Evidence],
            depends_on: vec![],
            depends_on_steps: vec![],
        }],
        manual: vec![],
    });
    let receipt = inv.record_analysis(req).unwrap().analysis;
    inv.record_remote(remote_request(
        remote_file(
            "new-child",
            "other",
            m::MetadataValue::Known("other".into()),
        ),
        ObservationTarget::NewItem,
        None,
    ))
    .unwrap();
    let after = inv
        .get_analysis(GetAnalysisRequest {
            selector: AnalysisSelector::Id(receipt.id),
        })
        .unwrap()
        .analysis
        .unwrap();
    assert!(after.stale);
    assert_eq!(
        current(&inv, &item.id).revision,
        receipt.resulting_item_revision
    );
}

#[test]
fn cross_account_commands_and_cyclic_step_dependencies_never_persist() {
    use drivesync_engine::operations::*;
    let (_tree, inv, item) = setup();
    let mut req = request(item.clone());
    req.assessment.plan = Some(AnalysisPlan {
        restore_request: None,
        namespaces: vec![],
        guards: vec![ItemGuard::for_item(&item).unwrap()],
        steps: vec![PlannedStep {
            method: PlannedMethod::Download,
            command: Some(OperationCommand::Download {
                source: ResourceRef::Remote {
                    provider_id: m::ProviderId("google".into()),
                    reference: m::RemoteFileRef {
                        account_id: AccountId("other-account".into()),
                        file_id: m::RemoteFileId("remote".into()),
                    },
                },
                destination: DirectoryRef::LocalRoot,
                name: ItemName::Local("file".into()),
            }),
            blocked_by: vec![PlanBlock::Evidence],
            depends_on: vec![],
            depends_on_steps: vec![],
        }],
        manual: vec![],
    });
    assert_eq!(
        inv.record_analysis(req.clone()).unwrap_err().kind,
        InventoryError::InvalidIdentity
    );
    let step = &mut req.assessment.plan.as_mut().unwrap().steps[0];
    step.command = None;
    step.depends_on_steps = vec![0];
    assert_eq!(
        inv.record_analysis(req).unwrap_err().kind,
        InventoryError::InvalidArgument
    );
    assert_eq!(current(&inv, &item.id), item);
    assert!(analyses(&inv, false).is_empty());
}

#[test]
fn conservative_context_epoch_changes_for_facts_but_not_analysis_diagnostics() {
    use drivesync_engine::operations::DirectoryRef;
    let (_tree, inv, item) = setup();
    let context = || {
        inv.get_namespace(GetNamespaceRequest {
            directory: DirectoryRef::LocalRoot,
        })
        .unwrap()
        .guard
    };
    let before = context();
    let receipt = inv.record_analysis(request(item.clone())).unwrap().analysis;
    assert_eq!(context(), before);
    inv.update_item_status(UpdateItemStatusRequest {
        item_id: item.id.clone(),
        expected_revision: receipt.resulting_item_revision,
        change: ItemStatusChange {
            status: SyncStatus::Error,
            conflict_reason: FieldUpdate::Keep,
        },
    })
    .unwrap();
    assert_eq!(context(), before);
    let item = current(&inv, &item.id);
    inv.record_remote(remote_request(
        remote_file(
            "new-metadata",
            "remote",
            m::MetadataValue::Known("renamed".into()),
        ),
        ObservationTarget::Existing(item.id),
        Some(item.revision),
    ))
    .unwrap();
    assert_ne!(context(), before);
}
