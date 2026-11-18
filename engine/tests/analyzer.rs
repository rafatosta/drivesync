//! Public Analyzer integration with real temporary Inventory and no physical operations.
use drivesync_engine::{
    analyzer::*,
    inventory::*,
    models::{self as m, AccountId, SyncId},
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "analyzer-{}-{}",
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

fn local_file(event: &str, name: &[u8]) -> m::File {
    let n = m::MetadataValue::NotCollected;
    let local = m::LocalFile {
        local_id: Some(m::LocalFileId("local".into())),
        relative_path: m::LocalRelativePath::new(b"file".to_vec()).unwrap(),
        kind: m::MetadataValue::Known(m::LocalEntryKind::Regular),
        mode: n.clone(),
        owner: m::LocalOwner {
            uid: m::MetadataValue::NotCollected,
            gid: m::MetadataValue::NotCollected,
        },
        times: m::LocalTimes {
            modified_at: m::MetadataValue::NotCollected,
            changed_at: m::MetadataValue::NotCollected,
            created_at: m::MetadataValue::NotCollected,
        },
        storage: m::LocalStorage {
            size_bytes: m::MetadataValue::Known(42),
            hard_link_count: m::MetadataValue::NotCollected,
            inode: m::MetadataValue::NotCollected,
            filesystem_device: m::MetadataValue::NotCollected,
        },
        flags: m::LocalFileFlags {
            statx_attributes: m::MetadataValue::NotCollected,
            statx_attributes_mask: m::MetadataValue::NotCollected,
        },
        symlink: m::LocalSymlink {
            target: m::MetadataValue::NotApplicable,
        },
    };

    let mut local = local;
    local.relative_path = m::LocalRelativePath::new(name.to_vec()).unwrap();
    let path = local.relative_path.clone();
    m::File {
        id: m::FileId(event.into()),
        sync_id: SyncId("sync".into()),
        observed_at: m::Timestamp { unix_millis: 10 },
        coverage: m::ObservationCoverage::Partial,
        scope: m::FileScope::Local(path.clone()),
        data: Some(m::FileData::Local(local)),
        event: m::FileEvent {
            action: m::FileEventAction::Modified,
            local: Some(m::LocalObservation {
                capture_id: "test".into(),
                mask: libc::IN_MODIFY,
                cookie: 0,
            }),
            local_presence: None,
            remote: None,
            uncertainty: None,
            previous: None,
            current: Some(m::FileLocation::Local(path)),
        },
    }
}
fn record_local(
    inventory: &InventoryManager,
    item: &SyncItem,
    event: &str,
    name: &[u8],
) -> SyncItem {
    inventory
        .record_local(RecordLocalObservation {
            selector: ObservationTarget::Existing(item.id.clone()),
            expected_revision: Some(item.revision),
            observation: LocalObservation {
                observed: local_file(event, name),
                presence: PresenceState::Present,
                proof: ObservationProof {
                    source: ObservationSource::LocalEvent,
                    observed_at: m::Timestamp { unix_millis: 10 },
                    scan_or_generation: None,
                    complete_scope: false,
                },
            },
        })
        .unwrap()
        .item
}
fn setup() -> (Tree, Arc<InventoryManager>, SyncItem) {
    let tree = Tree::new();
    let inventory = Arc::new(InventoryManager::open(tree.request()).unwrap().inventory);
    let item = inventory
        .record_remote(remote_request(
            remote_file("remote-1", "remote", m::MetadataValue::Known("file".into())),
            ObservationTarget::NewItem,
            None,
        ))
        .unwrap()
        .item;
    let item = record_local(&inventory, &item, "local-1", b"file");
    let side = BaselineSide {
        presence: PresenceState::Present,
        name: None,
        local_name_bytes: None,
        parent_identity: None,
        version: None,
        checksum: None,
        size: None,
        modified_at: None,
    };
    let mut local = side.clone();
    local.local_name_bytes = Some(b"file".to_vec());
    let mut remote = side;
    remote.name = Some("file".into());
    let item = inventory
        .confirm_sync(ConfirmSyncRequest {
            item_id: item.id.clone(),
            expected_item_revision: item.revision,
            confirmed: ConfirmedState {
                baseline: SyncBaseline {
                    local,
                    remote,
                    confirmed_at: m::Timestamp { unix_millis: 20 },
                },
                status: SyncStatus::Synced,
                verified_at: m::Timestamp { unix_millis: 20 },
            },
            completed_operation: None,
            expected_operation_attempt: None,
            activity: None,
        })
        .unwrap()
        .item;
    (tree, inventory, item)
}
fn analyzer(tree: &Tree, inventory: Arc<InventoryManager>) -> Analyzer {
    Analyzer::new(AnalyzerConfig {
        provider: drivesync_engine::models::ProviderId("google".into()),
        identity: tree.request().expected,
        dependencies: AnalyzerDependencies {
            inventory,
            execution: None,
        },
    })
    .unwrap()
}
fn request(item: SyncItem) -> AnalyzeRequest {
    AnalyzeRequest {
        observations: Vec::new(),
        directive: None,
        sync_id: SyncId("sync".into()),
        item,
        evidence: AnalysisEvidence {
            baseline_confirmed: true,
            local_scope_complete: true,
            remote_scope_complete: true,
            local_content: EvidenceRelation::Equal,
            remote_content: EvidenceRelation::Equal,
            current_content: EvidenceRelation::Equal,
            local_parent: None,
            current_parents: EvidenceRelation::Equal,
        },
        recorded_at: m::Timestamp { unix_millis: 30 },
    }
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
fn operations(inventory: &InventoryManager) -> Vec<SyncOperation> {
    inventory
        .list_operations(ListOperationsRequest {
            filter: OperationFilter::default(),
            pagination: Pagination::default(),
        })
        .unwrap()
        .page
        .entries
}
fn records(inventory: &InventoryManager) -> Vec<RecordedAnalysis> {
    inventory
        .list_analyses(ListAnalysesRequest {
            filter: AnalysisFilter::default(),
            pagination: Pagination::default(),
        })
        .unwrap()
        .page
        .entries
}
#[tokio::test]
async fn unchanged_evaluation_is_durable_without_confirming_baseline_or_executing() {
    let (tree, inventory, item) = setup();
    let analyzer = analyzer(&tree, inventory.clone());
    let a = analyzer
        .analyze(request(item.clone()))
        .await
        .unwrap()
        .analysis;
    assert_eq!(a.disposition, AnalysisDisposition::NoChanges);
    assert!(a.decisions.is_empty());
    assert!(a.requirements.is_empty());
    assert_eq!(current(&inventory, &item.id).baseline, item.baseline);
    assert_eq!(
        analyzer
            .status(AnalysisStatusRequest {
                item_id: item.id.clone()
            })
            .unwrap()
            .active,
        None
    );
    drop(analyzer);
    drop(inventory);
    let reopened = InventoryManager::open(tree.request()).unwrap().inventory;
    assert_eq!(records(&reopened), vec![a]);
    assert!(operations(&reopened).is_empty());
}
#[tokio::test]
async fn unilateral_changes_persist_ordered_blocked_operations_in_both_directions() {
    for local_changed in [true, false] {
        let (tree, inventory, item) = setup();
        let analyzer = analyzer(&tree, inventory.clone());
        let mut req = request(item.clone());
        req.evidence.current_content = EvidenceRelation::Different;
        if local_changed {
            req.evidence.local_content = EvidenceRelation::Different;
        } else {
            req.evidence.remote_content = EvidenceRelation::Different;
        }
        let out = analyzer.analyze(req).await.unwrap().analysis;
        assert_eq!(out.disposition, AnalysisDisposition::PendingOperations);
        assert_eq!(out.decisions.len(), 1);
        assert_eq!(
            out.decisions[0].intent,
            AnalysisIntent {
                direction: if local_changed {
                    OperationDirection::Upload
                } else {
                    OperationDirection::Download
                },
                kind: OperationKind::UpdateContent
            }
        );
        let ops = operations(&inventory);
        assert_eq!(ops[0].state, OperationState::Blocked);
        assert_eq!(ops[0].base_item_revision, out.resulting_item_revision);
        assert_eq!(
            ops[0].last_error_kind.as_deref(),
            Some("AnalyzerExecutionUnavailable")
        );
        assert_eq!(current(&inventory, &item.id).baseline, item.baseline);
    }
}
#[tokio::test]
async fn conflict_and_independent_rename_are_persisted_together_without_resolution() {
    let (tree, inventory, item) = setup();
    let changed = record_local(&inventory, &item, "local-rename", b"new-name");
    let analyzer = analyzer(&tree, inventory.clone());
    let mut req = request(changed.clone());
    req.evidence.local_content = EvidenceRelation::Different;
    req.evidence.remote_content = EvidenceRelation::Different;
    req.evidence.current_content = EvidenceRelation::Different;
    let out = analyzer.analyze(req).await.unwrap().analysis;
    assert_eq!(out.disposition, AnalysisDisposition::PendingUserDecision);
    assert!(out
        .findings
        .iter()
        .any(|f| f.origin == AnalysisOrigin::Conflicts
            && f.reason == AnalysisReason::ConcurrentChanges));
    assert_eq!(out.decisions.len(), 1);
    assert_eq!(out.decisions[0].intent.kind, OperationKind::Rename);
    assert_eq!(
        operations(&inventory)[0].last_error_kind.as_deref(),
        Some("AnalyzerConflictPending")
    );
    assert_eq!(current(&inventory, &item.id).status, SyncStatus::Conflict);
    assert_eq!(current(&inventory, &item.id).baseline, changed.baseline);
    drop(analyzer);
    drop(inventory);
    let reopened = InventoryManager::open(tree.request()).unwrap().inventory;
    assert_eq!(records(&reopened), vec![out]);
}
#[tokio::test]
async fn missing_evidence_is_durable_and_does_not_become_a_user_conflict() {
    let (tree, inventory, item) = setup();
    let analyzer = analyzer(&tree, inventory.clone());
    let mut req = request(item);
    req.evidence.local_content = EvidenceRelation::Unknown;
    req.evidence.remote_content = EvidenceRelation::Unknown;
    req.evidence.current_content = EvidenceRelation::Unknown;
    let out = analyzer.analyze(req).await.unwrap().analysis;
    assert_eq!(out.disposition, AnalysisDisposition::PendingEvidence);
    assert_eq!(out.requirements, vec![AnalysisRequirement::VerifyContent]);
    assert!(out.decisions.is_empty());
}
#[tokio::test]
async fn exact_retries_and_reanalysis_of_unchanged_current_item_reuse_the_receipt() {
    let (tree, inventory, item) = setup();
    let analyzer = analyzer(&tree, inventory.clone());
    let req = request(item.clone());
    let a = analyzer.analyze(req.clone()).await.unwrap();
    assert_eq!(analyzer.analyze(req).await.unwrap(), a);
    let mut req = request(current(&inventory, &item.id));
    req.recorded_at.unix_millis += 1;
    assert_eq!(analyzer.analyze(req).await.unwrap(), a);
    assert_eq!(records(&inventory).len(), 1);
}
#[tokio::test]
async fn stale_or_adulterated_inputs_and_wrong_scope_do_not_write() {
    let (tree, inventory, item) = setup();
    let analyzer = analyzer(&tree, inventory.clone());
    let mut forged = request(item.clone());
    forged.item.remote.as_mut().unwrap().name = m::MetadataValue::Known("forged".into());
    assert_eq!(
        analyzer.analyze(forged).await.unwrap_err().kind,
        AnalyzerErrorKind::StaleItem
    );
    let mut wrong = request(item.clone());
    wrong.sync_id = SyncId("other".into());
    assert_eq!(
        analyzer.analyze(wrong).await.unwrap_err().kind,
        AnalyzerErrorKind::WrongSyncScope
    );
    assert!(records(&inventory).is_empty());
    assert!(operations(&inventory).is_empty());
    assert_eq!(current(&inventory, &item.id), item);
    assert!(analyzer
        .status(AnalysisStatusRequest { item_id: item.id })
        .unwrap()
        .active
        .is_none());
}
#[tokio::test]
async fn evidence_failure_cleans_transient_status_and_leaves_inventory_unchanged() {
    let (tree, inventory, item) = setup();
    let analyzer = analyzer(&tree, inventory.clone());
    let mut req = request(item.clone());
    req.evidence.current_content = EvidenceRelation::Different;
    assert_eq!(
        analyzer.analyze(req).await.unwrap_err().kind,
        AnalyzerErrorKind::InvalidEvidence
    );
    assert!(analyzer
        .status(AnalysisStatusRequest {
            item_id: item.id.clone()
        })
        .unwrap()
        .active
        .is_none());
    assert_eq!(current(&inventory, &item.id), item);
    assert!(records(&inventory).is_empty());
}
#[tokio::test]
async fn persistence_failure_is_atomic_and_retry_can_record_the_same_analysis() {
    let (tree, inventory, item) = setup();
    let analyzer = analyzer(&tree, inventory.clone());
    let db = rusqlite::Connection::open(tree.0.join(".drivesync/inventory.sqlite3")).unwrap();
    db.execute_batch("CREATE TRIGGER fail_analysis BEFORE INSERT ON analyses BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert_eq!(
        analyzer
            .analyze(request(item.clone()))
            .await
            .unwrap_err()
            .kind,
        AnalyzerErrorKind::InventoryFailure
    );
    assert_eq!(current(&inventory, &item.id), item);
    assert!(records(&inventory).is_empty());
    assert!(analyzer
        .status(AnalysisStatusRequest {
            item_id: item.id.clone()
        })
        .unwrap()
        .active
        .is_none());
    db.execute_batch("DROP TRIGGER fail_analysis;").unwrap();
    assert!(
        !analyzer
            .analyze(request(item))
            .await
            .unwrap()
            .analysis
            .stale
    );
}
#[test]
fn constructor_rejects_a_different_inventory_identity() {
    let (tree, inventory, _item) = setup();
    let mut identity = tree.request().expected;
    identity.account_id = AccountId("other".into());
    assert!(
        matches!(Analyzer::new(AnalyzerConfig{ provider: drivesync_engine::models::ProviderId("google".into()),identity,dependencies:AnalyzerDependencies{inventory,execution:None}}),Err(e)if e.kind==AnalyzerErrorKind::WrongSyncScope)
    );
}
#[test]
fn queued_worker_retains_reservation_when_awaiting_future_is_cancelled() {
    use std::{sync::mpsc, time::Duration};
    let (tree, inventory, item) = setup();
    let analyzer = Arc::new(analyzer(&tree, inventory.clone()));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    let (entered_tx, entered) = mpsc::channel();
    let (release_tx, release) = mpsc::channel();
    let busy = runtime.spawn_blocking(move || {
        entered_tx.send(()).unwrap();
        release.recv().unwrap();
    });
    entered.recv_timeout(Duration::from_secs(2)).unwrap();
    runtime.block_on(async {
        let worker = analyzer.clone();
        let req = request(item.clone());
        let task = tokio::spawn(async move { worker.analyze(req).await });
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if analyzer
                    .status(AnalysisStatusRequest {
                        item_id: item.id.clone(),
                    })
                    .unwrap()
                    .active
                    .is_some()
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            analyzer
                .status(AnalysisStatusRequest {
                    item_id: item.id.clone()
                })
                .unwrap()
                .active
                .unwrap()
                .phase,
            AnalysisPhase::Validating
        );
        assert_eq!(
            analyzer
                .analyze(request(item.clone()))
                .await
                .unwrap_err()
                .kind,
            AnalyzerErrorKind::AlreadyRunning
        );
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(analyzer
            .status(AnalysisStatusRequest {
                item_id: item.id.clone()
            })
            .unwrap()
            .active
            .is_some());
        release_tx.send(()).unwrap();
        busy.await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if analyzer
                    .status(AnalysisStatusRequest {
                        item_id: item.id.clone(),
                    })
                    .unwrap()
                    .active
                    .is_none()
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(analyzer
            .status(AnalysisStatusRequest {
                item_id: item.id.clone()
            })
            .unwrap()
            .active
            .is_none());
        assert_eq!(records(&inventory).len(), 1);
    });
}

#[tokio::test]
async fn confirmed_absence_and_new_remote_file_persist_a_blocked_creation() {
    let tree = Tree::new();
    let inventory = Arc::new(InventoryManager::open(tree.request()).unwrap().inventory);
    let item = inventory
        .record_remote(remote_request(
            remote_file(
                "remote-new",
                "remote",
                m::MetadataValue::Known("file".into()),
            ),
            ObservationTarget::NewItem,
            None,
        ))
        .unwrap()
        .item;
    // Inventory only confirms absence for a previously associated local path.
    let item = record_local(&inventory, &item, "local-before-absence", b"file");
    let mut absent = local_file("local-absent", b"file");
    absent.data = None;
    absent.event.current = None;
    let item = inventory
        .record_local(RecordLocalObservation {
            selector: ObservationTarget::Existing(item.id.clone()),
            expected_revision: Some(item.revision),
            observation: LocalObservation {
                observed: absent,
                presence: PresenceState::AbsentConfirmed,
                proof: ObservationProof {
                    source: ObservationSource::LocalScan,
                    observed_at: m::Timestamp { unix_millis: 20 },
                    scan_or_generation: Some("scan".into()),
                    complete_scope: true,
                },
            },
        })
        .unwrap()
        .item;
    let analyzer = analyzer(&tree, inventory.clone());
    let mut req = request(item);
    req.evidence.baseline_confirmed = false;
    req.evidence.local_content = EvidenceRelation::Unknown;
    req.evidence.remote_content = EvidenceRelation::Unknown;
    req.evidence.current_content = EvidenceRelation::Unknown;
    let out = analyzer.analyze(req).await.unwrap().analysis;
    assert_eq!(out.disposition, AnalysisDisposition::PendingOperations);
    assert_eq!(
        out.decisions[0].intent,
        AnalysisIntent {
            direction: OperationDirection::Download,
            kind: OperationKind::Create
        }
    );
    assert!(out.requirements.is_empty());
    assert!(matches!(
        out.plan.as_ref().unwrap().steps[0].command,
        Some(drivesync_engine::operations::OperationCommand::Download { .. })
    ));
    assert_eq!(operations(&inventory)[0].state, OperationState::Blocked);
}
#[tokio::test]
async fn two_analyzer_instances_share_one_durable_receipt_for_the_same_input() {
    let (tree, inventory, item) = setup();
    let first = analyzer(&tree, inventory.clone());
    let second = analyzer(
        &tree,
        Arc::new(InventoryManager::open(tree.request()).unwrap().inventory),
    );
    let req = request(item);
    let (a, b) = tokio::join!(first.analyze(req.clone()), second.analyze(req));
    assert_eq!(a.unwrap(), b.unwrap());
    assert_eq!(records(&inventory).len(), 1);
}
#[tokio::test]
async fn untrusted_baseline_blocks_decisions_instead_of_inventing_new_creation() {
    let (tree, inventory, item) = setup();
    let analyzer = analyzer(&tree, inventory.clone());
    let mut req = request(item);
    req.evidence.baseline_confirmed = false;
    req.evidence.local_content = EvidenceRelation::Different;
    req.evidence.current_content = EvidenceRelation::Different;
    let out = analyzer.analyze(req).await.unwrap().analysis;
    assert_eq!(out.disposition, AnalysisDisposition::PendingEvidence);
    assert!(out
        .requirements
        .contains(&AnalysisRequirement::VerifyBaseline));
    assert!(out.decisions.is_empty());
    assert!(operations(&inventory).is_empty());
}

#[tokio::test]
async fn initial_remote_observation_persists_download_before_local_binding_without_execution() {
    let tree = Tree::new();
    let inventory = Arc::new(InventoryManager::open(tree.request()).unwrap().inventory);
    let item = inventory
        .record_remote(remote_request(
            remote_file(
                "first-remote",
                "remote",
                m::MetadataValue::Known("file".into()),
            ),
            ObservationTarget::NewItem,
            None,
        ))
        .unwrap()
        .item;
    let a = analyzer(&tree, inventory.clone());
    let mut req = request(item.clone());
    req.evidence.baseline_confirmed = false;
    req.evidence.local_scope_complete = false;
    req.evidence.local_content = EvidenceRelation::Unknown;
    req.evidence.remote_content = EvidenceRelation::Unknown;
    req.evidence.current_content = EvidenceRelation::Unknown;
    let record = a.analyze(req.clone()).await.unwrap().analysis;
    assert_eq!(record.disposition, AnalysisDisposition::PendingEvidence);
    assert_eq!(record.decisions.len(), 1);
    assert_eq!(
        record.decisions[0].intent,
        AnalysisIntent {
            direction: OperationDirection::Download,
            kind: OperationKind::Create
        }
    );
    assert!(record
        .requirements
        .contains(&AnalysisRequirement::ObserveLocal));
    assert!(record
        .requirements
        .contains(&AnalysisRequirement::VerifyIdentity));
    assert_eq!(a.analyze(req).await.unwrap().analysis.id, record.id);
    assert_eq!(operations(&inventory).len(), 1);
    assert_eq!(operations(&inventory)[0].state, OperationState::Blocked);
    assert_eq!(operations(&inventory)[0].attempt_count, 0);
    let after = current(&inventory, &item.id);
    assert_eq!(after.local_presence, PresenceState::Unknown);
    assert!(after.baseline.is_none());
    assert!(!tree.0.join("file").exists());
    drop(a);
    drop(inventory);
    let reopened = InventoryManager::open(tree.request()).unwrap().inventory;
    assert_eq!(
        reopened
            .get_analysis(GetAnalysisRequest {
                selector: AnalysisSelector::LatestForItem(item.id)
            })
            .unwrap()
            .analysis
            .unwrap(),
        record
    );
}

#[test]
fn homonyms_remain_distinct_until_an_explicit_user_association_is_committed() {
    let tree = Tree::new();
    let inv = InventoryManager::open(tree.request()).unwrap().inventory;
    let remote = inv
        .record_remote(remote_request(
            remote_file("r", "remote", m::MetadataValue::Known("file".into())),
            ObservationTarget::NewItem,
            None,
        ))
        .unwrap()
        .item;
    let local = inv
        .record_local(RecordLocalObservation {
            selector: ObservationTarget::NewItem,
            expected_revision: None,
            observation: LocalObservation {
                observed: local_file("l", b"file"),
                presence: PresenceState::Present,
                proof: ObservationProof {
                    source: ObservationSource::LocalEvent,
                    observed_at: m::Timestamp { unix_millis: 10 },
                    scan_or_generation: None,
                    complete_scope: false,
                },
            },
        })
        .unwrap()
        .item;
    assert_ne!(local.id, remote.id);
    let choice = AssociateItemsRequest {
        expected_local: local.clone(),
        expected_remote: remote.clone(),
        decision_id: "user-choice-1".into(),
        decided_at: m::Timestamp { unix_millis: 30 },
    };
    let mut stale = choice.clone();
    stale.expected_remote.revision += 1;
    assert_eq!(
        inv.associate_items(stale).unwrap_err().kind,
        InventoryError::VersionMismatch
    );
    let out = inv.associate_items(choice.clone()).unwrap();
    assert_eq!(out.item.id, local.id);
    assert_eq!(out.item.remote_ref, remote.remote_ref);
    assert!(out.item.baseline.is_none());
    assert_eq!(
        inv.associate_items(choice).unwrap().association,
        out.association
    );
    assert_eq!(
        inv.get_item(GetItemRequest {
            selector: ItemSelector::Id(remote.id)
        })
        .unwrap()
        .item
        .unwrap()
        .id,
        local.id
    );
    assert_eq!(
        inv.list_items(ListItemsRequest {
            filter: Default::default(),
            pagination: Default::default()
        })
        .unwrap()
        .page
        .entries
        .len(),
        1
    );
    assert!(inv
        .get_association(GetAssociationRequest {
            item_id: local.id.clone()
        })
        .unwrap()
        .association
        .is_some());
    drop(inv);
    let inv = InventoryManager::open(tree.request()).unwrap().inventory;
    assert!(inv
        .get_association(GetAssociationRequest { item_id: local.id })
        .unwrap()
        .association
        .is_some());
}

fn fresh_request(item: SyncItem) -> AnalyzeRequest {
    let mut req = request(item);
    req.evidence.baseline_confirmed = false;
    req.evidence.local_scope_complete = req.item.local_presence != PresenceState::Unknown;
    req.evidence.remote_scope_complete = req.item.remote_presence != PresenceState::Unknown;
    req.evidence.local_content = EvidenceRelation::Unknown;
    req.evidence.remote_content = EvidenceRelation::Unknown;
    req.evidence.current_content = EvidenceRelation::Unknown;
    req
}
#[tokio::test]
async fn independent_remote_homonyms_produce_manual_collision_without_association() {
    let tree = Tree::new();
    let inv = Arc::new(InventoryManager::open(tree.request()).unwrap().inventory);
    let first = inv
        .record_remote(remote_request(
            remote_file("a", "a", m::MetadataValue::Known("same".into())),
            ObservationTarget::NewItem,
            None,
        ))
        .unwrap()
        .item;
    let second = inv
        .record_remote(remote_request(
            remote_file("b", "b", m::MetadataValue::Known("same".into())),
            ObservationTarget::NewItem,
            None,
        ))
        .unwrap()
        .item;
    let a = analyzer(&tree, inv.clone());
    for item in [first.clone(), second.clone()] {
        let record = a.analyze(fresh_request(item)).await.unwrap().analysis;
        assert_eq!(record.disposition, AnalysisDisposition::PendingUserDecision);
        let plan = record.plan.unwrap();
        assert!(plan.steps[0].blocked_by.contains(&PlanBlock::Collision));
        assert_eq!(plan.manual[0].reason, ManualReason::DestinationOccupied);
        assert_eq!(plan.manual[0].resources.len(), 2);
        assert!(matches!(
            plan.steps[0].command,
            Some(drivesync_engine::operations::OperationCommand::Download { .. })
        ));
    }
    assert_ne!(
        current(&inv, &first.id).remote_ref,
        current(&inv, &second.id).remote_ref
    );
    assert!(inv
        .get_association(GetAssociationRequest { item_id: first.id })
        .unwrap()
        .association
        .is_none());
    assert!(!tree.0.join("same").exists());
    assert!(operations(&inv)
        .iter()
        .all(|o| o.state == OperationState::Blocked && o.attempt_count == 0));
}
#[tokio::test]
async fn missing_parent_replica_is_an_explicit_identity_dependency() {
    let tree = Tree::new();
    let inv = Arc::new(InventoryManager::open(tree.request()).unwrap().inventory);
    let mut parent = remote_file("p", "parent", m::MetadataValue::Known("folder".into()));
    if let Some(m::FileData::Remote(r)) = &mut parent.data {
        r.kind = m::MetadataValue::Known(m::RemoteEntryKind::Directory)
    }
    let parent = inv
        .record_remote(remote_request(parent, ObservationTarget::NewItem, None))
        .unwrap()
        .item;
    let mut child = remote_file("c", "child", m::MetadataValue::Known("child".into()));
    if let Some(m::FileData::Remote(r)) = &mut child.data {
        r.parent_ids = m::MetadataValue::Known(vec![m::RemoteFileId("parent".into())])
    }
    if let Some(m::FileLocation::Remote { parent_id, .. }) = &mut child.event.current {
        *parent_id = Some(m::RemoteFileId("parent".into()));
    }
    let child = inv
        .record_remote(remote_request(child, ObservationTarget::NewItem, None))
        .unwrap()
        .item;
    let record = analyzer(&tree, inv.clone())
        .analyze(fresh_request(child))
        .await
        .unwrap()
        .analysis;
    let plan = record.plan.unwrap();
    assert_eq!(plan.steps[0].depends_on, vec![parent.id]);
    assert!(plan.steps[0].blocked_by.contains(&PlanBlock::Parent));
    assert!(plan.steps[0].command.is_none());
}
#[tokio::test]
async fn unchanged_survivor_produces_only_authorization_blocked_trash_never_delete() {
    let (tree, inv, item) = setup();
    let mut baseline = item.baseline.clone().unwrap();
    baseline.confirmed_at = m::Timestamp { unix_millis: 21 };
    baseline.remote.parent_identity = Some(BaselineParent::Remote(m::RemoteFileRef {
        account_id: AccountId("account".into()),
        file_id: m::RemoteFileId("root".into()),
    }));
    let item = inv
        .confirm_sync(ConfirmSyncRequest {
            item_id: item.id.clone(),
            expected_item_revision: item.revision,
            confirmed: ConfirmedState {
                baseline,
                status: SyncStatus::Synced,
                verified_at: m::Timestamp { unix_millis: 21 },
            },
            completed_operation: None,
            expected_operation_attempt: None,
            activity: None,
        })
        .unwrap()
        .item;
    let mut observed = local_file("gone", b"file");
    observed.data = None;
    observed.event.current = None;
    let item = inv
        .record_local(RecordLocalObservation {
            selector: ObservationTarget::Existing(item.id.clone()),
            expected_revision: Some(item.revision),
            observation: LocalObservation {
                observed,
                presence: PresenceState::AbsentConfirmed,
                proof: ObservationProof {
                    source: ObservationSource::LocalScan,
                    observed_at: m::Timestamp { unix_millis: 30 },
                    scan_or_generation: Some("scan".into()),
                    complete_scope: true,
                },
            },
        })
        .unwrap()
        .item;
    let mut req = request(item);
    req.evidence.current_content = EvidenceRelation::Unknown;
    let record = analyzer(&tree, inv.clone())
        .analyze(req)
        .await
        .unwrap()
        .analysis;
    assert_eq!(record.decisions.len(), 1);
    assert_eq!(record.decisions[0].intent.kind, OperationKind::Trash);
    let step = &record.plan.as_ref().unwrap().steps[0];
    assert!(matches!(
        step.command,
        Some(drivesync_engine::operations::OperationCommand::Trash { .. })
    ));
    assert!(step.blocked_by.contains(&PlanBlock::UserAuthorization));
    assert!(operations(&inv)
        .iter()
        .all(|o| o.kind != OperationKind::Delete && o.state == OperationState::Blocked));
}
#[tokio::test]
async fn restore_is_explicit_scoped_and_persisted_without_executing() {
    use drivesync_engine::operations::*;
    let tree = Tree::new();
    let inv = Arc::new(InventoryManager::open(tree.request()).unwrap().inventory);
    let mut observed = remote_file("trashed", "remote", m::MetadataValue::Known("file".into()));
    if let Some(m::FileData::Remote(r)) = &mut observed.data {
        r.trashed = m::MetadataValue::Known(true)
    }
    let item = inv
        .record_remote(remote_request(observed, ObservationTarget::NewItem, None))
        .unwrap()
        .item;
    let target = ResourceRef::Remote {
        provider_id: m::ProviderId("google".into()),
        reference: item.remote_ref.clone().unwrap(),
    };
    let a = analyzer(&tree, inv.clone());
    let mut req = fresh_request(item.clone());
    let initial = a.analyze(req.clone()).await.unwrap().analysis;
    assert!(initial.decisions.is_empty());
    req.item = current(&inv, &item.id);
    req.directive = Some(RestoreSelection {
        decision_id: "user-restore-1".into(),
        target: target.clone(),
    });
    let record = a.analyze(req.clone()).await.unwrap().analysis;
    assert_eq!(record.decisions[0].intent.kind, OperationKind::Restore);
    assert_eq!(record.plan.as_ref().unwrap().restore_request, req.directive);
    assert!(
        matches!(&record.plan.as_ref().unwrap().steps[0].command,Some(OperationCommand::Restore{target:t,..}) if *t==target)
    );
    assert_eq!(a.analyze(req).await.unwrap().analysis.id, record.id);
    assert!(current(&inv, &item.id).baseline.is_none());
    assert_eq!(
        current(&inv, &item.id).remote.unwrap().trashed,
        m::MetadataValue::Known(true)
    );
}

#[tokio::test]
async fn multiple_mutations_preserve_commands_but_require_revalidation_after_prior_step() {
    use drivesync_engine::operations::OperationCommand;
    let (tree, inv, item) = setup();
    let item = record_local(&inv, &item, "renamed", b"new-name");
    let mut req = request(item);
    req.evidence.local_content = EvidenceRelation::Different;
    req.evidence.remote_content = EvidenceRelation::Equal;
    req.evidence.current_content = EvidenceRelation::Different;
    let record = analyzer(&tree, inv.clone())
        .analyze(req)
        .await
        .unwrap()
        .analysis;
    let steps = &record.plan.unwrap().steps;
    assert_eq!(steps.len(), 2);
    assert!(matches!(
        steps[0].command,
        Some(OperationCommand::UpdateContent { .. })
    ));
    assert!(matches!(
        steps[1].command,
        Some(OperationCommand::Rename { .. })
    ));
    assert_eq!(steps[1].depends_on_steps, vec![0]);
    assert!(steps[1].blocked_by.contains(&PlanBlock::PriorStep));
    assert!(!steps[0].blocked_by.contains(&PlanBlock::PriorStep));
    assert_eq!(record.disposition, AnalysisDisposition::PendingEvidence);
}
#[tokio::test]
async fn native_documents_are_never_given_an_implicit_binary_download() {
    let tree = Tree::new();
    let inv = Arc::new(InventoryManager::open(tree.request()).unwrap().inventory);
    let mut observed = remote_file("native", "doc", m::MetadataValue::Known("document".into()));
    if let Some(m::FileData::Remote(r)) = &mut observed.data {
        r.mime_type = m::MetadataValue::Known("application/vnd.google-apps.document".into())
    }
    let item = inv
        .record_remote(remote_request(observed, ObservationTarget::NewItem, None))
        .unwrap()
        .item;
    let out = analyzer(&tree, inv.clone())
        .analyze(fresh_request(item))
        .await
        .unwrap()
        .analysis;
    assert!(out
        .requirements
        .contains(&AnalysisRequirement::VerifyCapabilities));
    assert!(out.plan.unwrap().steps[0]
        .blocked_by
        .contains(&PlanBlock::Capabilities));
}

#[tokio::test]
async fn known_remote_trash_is_active_absence_while_unknown_access_never_is() {
    for changed in [false, true] {
        let (tree, inv, item) = setup();
        let mut parent = local_file("local-parent", b"parent");
        if let Some(m::FileData::Local(f)) = &mut parent.data {
            f.local_id = Some(m::LocalFileId("parent-local".into()));
            f.kind = m::MetadataValue::Known(m::LocalEntryKind::Directory)
        }
        let parent = inv
            .record_local(RecordLocalObservation {
                selector: ObservationTarget::NewItem,
                expected_revision: None,
                observation: LocalObservation {
                    observed: parent,
                    presence: PresenceState::Present,
                    proof: ObservationProof {
                        source: ObservationSource::LocalEvent,
                        observed_at: m::Timestamp { unix_millis: 10 },
                        scan_or_generation: None,
                        complete_scope: false,
                    },
                },
            })
            .unwrap()
            .item;
        let item = record_local(&inv, &item, "local-moved", b"parent/file");
        let mut baseline = item.baseline.clone().unwrap();
        baseline.confirmed_at = m::Timestamp { unix_millis: 21 };
        baseline.local.parent_identity = Some(BaselineParent::Local(parent.id.clone()));
        let item = inv
            .confirm_sync(ConfirmSyncRequest {
                item_id: item.id.clone(),
                expected_item_revision: item.revision,
                confirmed: ConfirmedState {
                    baseline,
                    status: SyncStatus::Synced,
                    verified_at: m::Timestamp { unix_millis: 21 },
                },
                completed_operation: None,
                expected_operation_attempt: None,
                activity: None,
            })
            .unwrap()
            .item;
        let mut observed =
            remote_file("in-trash", "remote", m::MetadataValue::Known("file".into()));
        if let Some(m::FileData::Remote(r)) = &mut observed.data {
            r.trashed = m::MetadataValue::Known(true)
        }
        let item = inv
            .record_remote(remote_request(
                observed,
                ObservationTarget::Existing(item.id.clone()),
                Some(item.revision),
            ))
            .unwrap()
            .item;
        let mut req = request(item.clone());
        req.evidence.local_parent = Some(parent.id);
        req.evidence.local_content = if changed {
            EvidenceRelation::Different
        } else {
            EvidenceRelation::Equal
        };
        let a = analyzer(&tree, inv.clone());
        let out = a.analyze(req).await.unwrap().analysis;
        if changed {
            assert_eq!(out.disposition, AnalysisDisposition::PendingUserDecision);
            assert!(out.decisions.is_empty());
            assert!(out
                .findings
                .iter()
                .any(|f| f.reason == AnalysisReason::DeleteVsChange));
        } else {
            assert_eq!(
                out.decisions[0].intent,
                AnalysisIntent {
                    kind: OperationKind::Trash,
                    direction: OperationDirection::Download
                }
            );
            assert!(out.plan.unwrap().steps[0]
                .blocked_by
                .contains(&PlanBlock::UserAuthorization));
        }
        // Losing access cannot reuse the retained trash flag as current proof.
        let item = current(&inv, &item.id);
        let mut unknown = remote_file(
            "unavailable",
            "remote",
            m::MetadataValue::Known("file".into()),
        );
        unknown.data = None;
        unknown.event.current = None;
        let mut unknown = remote_request(
            unknown,
            ObservationTarget::Existing(item.id.clone()),
            Some(item.revision),
        );
        unknown.observation.presence = PresenceState::Unknown;
        let item = inv.record_remote(unknown).unwrap().item;
        let mut req = request(item);
        req.evidence.current_content = EvidenceRelation::Unknown;
        req.evidence.remote_scope_complete = false;
        let out = a.analyze(req).await.unwrap().analysis;
        assert!(out.decisions.is_empty());
        assert!(out
            .requirements
            .contains(&AnalysisRequirement::ObserveRemote));
    }
}

#[tokio::test]
async fn configured_provider_mismatch_cannot_reuse_a_google_plan() {
    let (tree, inv, item) = setup();
    let a = Analyzer::new(AnalyzerConfig {
        provider: m::ProviderId("other".into()),
        identity: tree.request().expected,
        dependencies: AnalyzerDependencies {
            inventory: inv.clone(),
            execution: None,
        },
    })
    .unwrap();
    assert_eq!(
        a.analyze(request(item)).await.unwrap_err().kind,
        AnalyzerErrorKind::WrongSyncScope
    );
    assert!(records(&inv).is_empty());
}

#[tokio::test]
async fn folder_trash_keeps_descendant_identity_and_surviving_state_requirements() {
    let tree = Tree::new();
    let inv = Arc::new(InventoryManager::open(tree.request()).unwrap().inventory);
    let mut observed = remote_file("folder", "folder", m::MetadataValue::Known("folder".into()));
    if let Some(m::FileData::Remote(f)) = &mut observed.data {
        f.kind = m::MetadataValue::Known(m::RemoteEntryKind::Directory)
    }
    let folder = inv
        .record_remote(remote_request(observed, ObservationTarget::NewItem, None))
        .unwrap()
        .item;
    let mut local = local_file("folder-local", b"folder");
    if let Some(m::FileData::Local(f)) = &mut local.data {
        f.kind = m::MetadataValue::Known(m::LocalEntryKind::Directory)
    }
    let folder = inv
        .record_local(RecordLocalObservation {
            selector: ObservationTarget::Existing(folder.id.clone()),
            expected_revision: Some(folder.revision),
            observation: LocalObservation {
                observed: local,
                presence: PresenceState::Present,
                proof: ObservationProof {
                    source: ObservationSource::LocalEvent,
                    observed_at: m::Timestamp { unix_millis: 10 },
                    scan_or_generation: None,
                    complete_scope: false,
                },
            },
        })
        .unwrap()
        .item;
    let local = BaselineSide {
        presence: PresenceState::Present,
        name: None,
        local_name_bytes: Some(b"folder".to_vec()),
        parent_identity: None,
        version: None,
        checksum: None,
        size: None,
        modified_at: None,
    };
    let mut remote = local.clone();
    remote.local_name_bytes = None;
    remote.name = Some("folder".into());
    remote.parent_identity = Some(BaselineParent::Remote(m::RemoteFileRef {
        account_id: AccountId("account".into()),
        file_id: m::RemoteFileId("root".into()),
    }));
    let folder = inv
        .confirm_sync(ConfirmSyncRequest {
            item_id: folder.id.clone(),
            expected_item_revision: folder.revision,
            confirmed: ConfirmedState {
                baseline: SyncBaseline {
                    local,
                    remote,
                    confirmed_at: m::Timestamp { unix_millis: 20 },
                },
                status: SyncStatus::Synced,
                verified_at: m::Timestamp { unix_millis: 20 },
            },
            completed_operation: None,
            expected_operation_attempt: None,
            activity: None,
        })
        .unwrap()
        .item;
    let mut child = remote_file("child", "child", m::MetadataValue::Known("child".into()));
    if let Some(m::FileData::Remote(f)) = &mut child.data {
        f.parent_ids = m::MetadataValue::Known(vec![m::RemoteFileId("folder".into())])
    }
    if let Some(m::FileLocation::Remote { parent_id, .. }) = &mut child.event.current {
        *parent_id = Some(m::RemoteFileId("folder".into()))
    }
    let child = inv
        .record_remote(remote_request(child, ObservationTarget::NewItem, None))
        .unwrap()
        .item;
    let mut local = local_file("folder-gone", b"folder");
    local.data = None;
    local.event.current = None;
    let folder = inv
        .record_local(RecordLocalObservation {
            selector: ObservationTarget::Existing(folder.id.clone()),
            expected_revision: Some(folder.revision),
            observation: LocalObservation {
                observed: local,
                presence: PresenceState::AbsentConfirmed,
                proof: ObservationProof {
                    source: ObservationSource::LocalScan,
                    observed_at: m::Timestamp { unix_millis: 30 },
                    scan_or_generation: Some("scan".into()),
                    complete_scope: true,
                },
            },
        })
        .unwrap()
        .item;
    let mut req = request(folder);
    req.evidence.current_content = EvidenceRelation::Unknown;
    let out = analyzer(&tree, inv.clone())
        .analyze(req)
        .await
        .unwrap()
        .analysis;
    assert_eq!(out.decisions[0].intent.kind, OperationKind::Trash);
    assert!(out
        .requirements
        .contains(&AnalysisRequirement::VerifySurvivingState));
    let plan = out.plan.unwrap();
    assert!(plan.steps[0].depends_on.contains(&child.id));
    assert!(plan.steps[0]
        .blocked_by
        .contains(&PlanBlock::UserAuthorization));
    assert!(current(&inv, &child.id).baseline.is_none());
}

#[tokio::test]
async fn unknown_old_destination_is_evidence_pending_not_a_proven_collision() {
    let (tree, inv, item) = setup();
    let mut missing = local_file("unobserved", b"file");
    missing.data = None;
    missing.event.current = None;
    inv.record_local(RecordLocalObservation {
        selector: ObservationTarget::Existing(item.id.clone()),
        expected_revision: Some(item.revision),
        observation: LocalObservation {
            observed: missing,
            presence: PresenceState::Unknown,
            proof: ObservationProof {
                source: ObservationSource::LocalEvent,
                observed_at: m::Timestamp { unix_millis: 10 },
                scan_or_generation: None,
                complete_scope: false,
            },
        },
    })
    .unwrap();
    let other = inv
        .record_remote(remote_request(
            remote_file(
                "independent",
                "independent",
                m::MetadataValue::Known("file".into()),
            ),
            ObservationTarget::NewItem,
            None,
        ))
        .unwrap()
        .item;
    let record = analyzer(&tree, inv.clone())
        .analyze(fresh_request(other))
        .await
        .unwrap()
        .analysis;
    assert_eq!(record.disposition, AnalysisDisposition::PendingEvidence);
    let plan = record.plan.unwrap();
    assert!(!plan.steps[0].blocked_by.contains(&PlanBlock::Collision));
    assert!(plan.steps[0].blocked_by.contains(&PlanBlock::Evidence));
    assert!(!plan
        .manual
        .iter()
        .any(|m| m.reason == ManualReason::DestinationOccupied));
}

#[tokio::test]
async fn many_remote_homonyms_remain_one_manual_collision_without_association() {
    let (tree, inv, _) = setup();
    let mut first = None;
    for n in 0..80 {
        let id = format!("homonym-{n}");
        let item = inv
            .record_remote(remote_request(
                remote_file(&id, &id, m::MetadataValue::Known("same-name".into())),
                ObservationTarget::NewItem,
                None,
            ))
            .unwrap()
            .item;
        first.get_or_insert(item);
    }
    let record = analyzer(&tree, inv.clone())
        .analyze(fresh_request(first.unwrap()))
        .await
        .unwrap()
        .analysis;
    let plan = record.plan.unwrap();
    let issues: Vec<_> = plan
        .manual
        .iter()
        .filter(|issue| issue.reason == ManualReason::DestinationOccupied)
        .collect();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].resources.len(), 80);
    assert!(plan.steps[0].blocked_by.contains(&PlanBlock::Collision));
}
