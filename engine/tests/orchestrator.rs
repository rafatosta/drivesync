//! Scan and decision coordination uses only public module facades.
use drivesync_engine::{
    detectors::*, models::*, Engine, EngineConfig, ListAccountsRequest, Orchestrator,
};
use std::{
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};
struct Tree(PathBuf);
impl Tree {
    async fn new() -> (Self, Engine) {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let base = std::env::temp_dir().join(format!(
            "orchestrator-unified-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir(&base).unwrap();
        std::fs::create_dir(base.join("accounts")).unwrap();
        std::fs::set_permissions(
            base.join("accounts"),
            std::fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        let records: serde_json::Map<String, serde_json::Value> = ["first", "second"].into_iter().map(|id| (id.into(), serde_json::json!({"account":{"id":id,"provider":"google","provider_user_id":id,"email":format!("{id}@example.invalid"),"display_name":null,"state":"ReauthorizationRequired","created_at":{"unix_millis":0},"updated_at":{"unix_millis":0}},"key":format!("google:{id}:synthetic")}))).collect();
        let path = base.join("accounts/accounts.json");
        std::fs::write(&path, serde_json::to_vec(&serde_json::json!({"version":1,"application":"org.example.OrchestratorTests","intent":null,"records":records})).unwrap()).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let connector = Engine::open(EngineConfig {
            data_directory: base.join("accounts"),
            application_id: "org.example.OrchestratorTests".into(),
            google_oauth_file: None,
        })
        .await
        .unwrap();
        for id in ["first", "second"] {
            std::fs::create_dir(base.join(id)).unwrap();
        }
        (Self(base), connector)
    }
    fn open(&self, connector: Engine, id: &str) -> Orchestrator {
        Orchestrator::new(drivesync_engine::orchestrator::OrchestratorCreateRequest {
            detector: DetectorCreateRequest {
                connector,
                scope: DetectorScope {
                    account_id: AccountId(id.into()),
                    sync_id: SyncId(format!("sync-{id}")),
                    local_root: self.0.join(id),
                    remote_root_id: "root".into(),
                },
                local_settings: LocalDetectorSettings::default(),
                remote_settings: RemoteDetectorSettings::default(),
                event_available_sink: None,
            },
        })
        .unwrap()
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn wait(test: impl Fn() -> bool) {
    let end = Instant::now() + Duration::from_secs(5);
    while !test() {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[tokio::test]
async fn connector_survives_stopped_capture_and_facade_starts_both_sources() {
    let (tree, connector) = Tree::new().await;
    let session = tree.open(connector, "first");
    assert_eq!(
        session.detector_status().unwrap().state,
        DetectorState::Stopped
    );
    assert!(session.diagnostics().events.is_empty());
    let report = session.prepare().await.unwrap();
    assert!(report.ready_for_analysis());
    wait(|| {
        session.remote_diagnostics().unwrap().status.last_error
            == Some(RemoteError::AuthorizationRequired)
    });
    assert_eq!(
        session.local_status().unwrap().state,
        LocalDetectorState::Running
    );
    session.stop_detector(DetectorStopMode::StopNow).unwrap();
    let connector = session.connector().clone();
    drop(session);
    assert_eq!(
        connector
            .list_accounts(ListAccountsRequest { provider: None })
            .await
            .unwrap()
            .accounts
            .len(),
        2
    );
}
fn settle_flow(session: &Orchestrator) {
    use drivesync_engine::orchestrator::PreparationState;
    wait(|| {
        let status = session.preparation_status();
        assert!(
            !matches!(
                status.state,
                PreparationState::Failed | PreparationState::Partial
            ),
            "flow failed: {:?}",
            status.error
        );
        status.state == PreparationState::ReadyForAnalyzer
            && session.diagnostics().pending_events == Some(0)
            && session.diagnostics().leased_events == Some(0)
            && session.remote_diagnostics().is_some_and(|remote| {
                remote.status.last_error == Some(RemoteError::AuthorizationRequired)
            })
    });
}
#[tokio::test]
async fn live_capture_delivers_new_files_and_folders_to_analyzer_without_rescan_or_crossing_accounts(
) {
    use drivesync_engine::inventory::*;
    let (tree, connector) = Tree::new().await;
    let first = tree.open(connector.clone(), "first");
    let second = tree.open(connector.clone(), "second");
    first.prepare().await.unwrap();
    second.prepare().await.unwrap();
    settle_flow(&first);
    settle_flow(&second);
    let inv = inventory(&tree, "first");
    let before = first.diagnostics().confirmed_events.unwrap();
    std::fs::create_dir(tree.0.join("first/new-folder")).unwrap();
    std::fs::write(tree.0.join("first/new-folder/new-file.txt"), b"durable").unwrap();
    wait(|| {
        items(&inv).iter().any(|item| {
            item.local
                .as_ref()
                .is_some_and(|file| file.relative_path.as_bytes() == b"new-folder/new-file.txt")
        })
    });
    settle_flow(&first);
    let current = inv
        .list_analyses(ListAnalysesRequest {
            filter: AnalysisFilter {
                current_only: true,
                ..Default::default()
            },
            pagination: Pagination::default(),
        })
        .unwrap()
        .page
        .entries;
    assert_eq!(current.len(), 2);
    assert!(current.iter().all(|r| !r.stale));
    assert!(current
        .iter()
        .all(|r| r.decisions.len() == 1
            && r.decisions[0].intent.direction == OperationDirection::Upload));
    assert!(first.diagnostics().confirmed_events.unwrap() > before);
    assert!(first.preparation_status().events_processed > 0);
    assert_eq!(first.diagnostics().delivery, Some(DeliveryState::Enabled));
    assert!(items(&inventory(&tree, "second")).is_empty());
    let ids = items(&inv)
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    first.stop_detector(DetectorStopMode::StopNow).unwrap();
    drop(first);
    let reopened = tree.open(connector, "first");
    reopened.prepare().await.unwrap();
    settle_flow(&reopened);
    assert!(ids
        .iter()
        .all(|id| items(&inv).iter().any(|item| &item.id == id)));
    assert_eq!(
        std::fs::read(tree.0.join("first/new-folder/new-file.txt")).unwrap(),
        b"durable"
    );
    reopened.stop_detector(DetectorStopMode::StopNow).unwrap();
    second.stop_detector(DetectorStopMode::StopNow).unwrap();
}
#[tokio::test]
async fn partial_preparation_keeps_capture_and_delivery_paused_until_a_successful_rescan() {
    use drivesync_engine::orchestrator::PreparationState;
    let (tree, connector) = Tree::new().await;
    let root = tree.0.join("first");
    std::fs::create_dir(root.join("locked")).unwrap();
    std::fs::write(root.join("locked/child"), b"data").unwrap();
    let session = tree.open(connector, "first");
    session.prepare().await.unwrap();
    settle_flow(&session);
    std::fs::set_permissions(root.join("locked"), std::fs::Permissions::from_mode(0o000)).unwrap();
    let result = session.prepare().await;
    std::fs::set_permissions(root.join("locked"), std::fs::Permissions::from_mode(0o700)).unwrap();
    let report = result.unwrap();
    assert!(!report.ready_for_analysis());
    assert_eq!(
        session.preparation_status().state,
        PreparationState::Partial
    );
    assert_eq!(session.diagnostics().delivery, Some(DeliveryState::Paused));
    assert!(session.diagnostics().confirmed_events.is_some());
    assert!(session.prepare().await.unwrap().ready_for_analysis());
    assert_eq!(
        session.preparation_status().state,
        PreparationState::ReadyForAnalyzer
    );
    assert_eq!(session.diagnostics().delivery, Some(DeliveryState::Enabled));
}
#[tokio::test]
async fn incompatible_inventory_is_preserved_and_never_consumes_capture_events() {
    use drivesync_engine::orchestrator::{PreparationFailure, PreparationState};
    let (tree, connector) = Tree::new().await;
    let session = tree.open(connector, "first");
    session.prepare().await.unwrap();
    settle_flow(&session);
    session.stop_detector(DetectorStopMode::StopNow).unwrap();
    let path = tree.0.join("first/.drivesync/inventory.sqlite3");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.pragma_update(None, "user_version", 900).unwrap();
    drop(db);
    let before = std::fs::read(&path).unwrap();
    assert!(matches!(
        session.prepare().await,
        Err(PreparationFailure::Inventory(_))
    ));
    assert_eq!(session.preparation_status().state, PreparationState::Failed);
    assert_eq!(std::fs::read(path).unwrap(), before);
    assert!(session.diagnostics().confirmed_events.is_some());
}

#[tokio::test]
async fn stopping_during_preparation_cannot_publish_ready_after_stop() {
    use drivesync_engine::orchestrator::PreparationState;
    let (tree, connector) = Tree::new().await;
    let owner = std::sync::Arc::new(tree.open(connector, "first"));
    let running = owner.clone();
    let task = tokio::spawn(async move { running.prepare().await });
    for _ in 0..100 {
        if owner.preparation_status().state != PreparationState::Idle {
            break;
        }
        tokio::task::yield_now().await;
    }
    owner.stop_detector(DetectorStopMode::StopNow).unwrap();
    let _ = task.await;
    assert_eq!(owner.preparation_status().state, PreparationState::Stopped);
    assert_eq!(owner.diagnostics().delivery, Some(DeliveryState::Paused));
    assert_eq!(owner.diagnostics().leased_events, Some(0));
}
#[tokio::test]
async fn another_preparation_is_rejected_while_inventory_setup_is_busy() {
    let (tree, connector) = Tree::new().await;
    let owner = std::sync::Arc::new(tree.open(connector, "first"));
    owner.prepare().await.unwrap();
    let db = rusqlite::Connection::open(tree.0.join("first/.drivesync/inventory.sqlite3")).unwrap();
    db.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let running = owner.clone();
    let task = tokio::spawn(async move { running.prepare().await });
    while owner.preparation_status().state
        != drivesync_engine::orchestrator::PreparationState::Starting
    {
        tokio::task::yield_now().await;
    }
    assert!(matches!(
        owner.prepare().await,
        Err(drivesync_engine::orchestrator::PreparationFailure::AlreadyRunning)
    ));
    db.execute_batch("ROLLBACK").unwrap();
    assert!(task.await.unwrap().unwrap().ready_for_analysis());
}

#[tokio::test]
async fn preparation_can_retry_after_local_capture_failed_to_start() {
    use drivesync_engine::orchestrator::PreparationState;
    let (tree, connector) = Tree::new().await;
    let session = tree.open(connector, "first");
    let root = tree.0.join("first");
    let moved = tree.0.join("temporarily-moved");
    std::fs::rename(&root, &moved).unwrap();
    let failed = session.prepare().await;
    std::fs::rename(&moved, &root).unwrap();
    assert!(failed.is_err());
    assert_eq!(session.preparation_status().state, PreparationState::Failed);
    assert!(session.prepare().await.unwrap().ready_for_analysis());
    assert_eq!(
        session.preparation_status().state,
        PreparationState::ReadyForAnalyzer
    );
    assert!(session.diagnostics().confirmed_events.is_some());
    assert_eq!(session.diagnostics().delivery, Some(DeliveryState::Enabled));
}

fn inventory(tree: &Tree, id: &str) -> drivesync_engine::inventory::InventoryManager {
    use drivesync_engine::inventory::*;
    InventoryManager::open(OpenInventoryRequest {
        local_root: tree.0.join(id),
        expected: SyncIdentity {
            sync_id: SyncId(format!("sync-{id}")),
            account_id: AccountId(id.into()),
            remote_root_id: "root".into(),
        },
        create_if_missing: false,
    })
    .unwrap()
    .inventory
}
fn analyses(
    inventory: &drivesync_engine::inventory::InventoryManager,
) -> Vec<drivesync_engine::inventory::RecordedAnalysis> {
    use drivesync_engine::inventory::*;
    inventory
        .list_analyses(ListAnalysesRequest {
            filter: AnalysisFilter::default(),
            pagination: Pagination::default(),
        })
        .unwrap()
        .page
        .entries
}
fn items(
    inventory: &drivesync_engine::inventory::InventoryManager,
) -> Vec<drivesync_engine::inventory::SyncItem> {
    use drivesync_engine::inventory::*;
    inventory
        .list_items(ListItemsRequest {
            filter: ItemFilter::default(),
            pagination: Pagination::default(),
        })
        .unwrap()
        .page
        .entries
}
fn remote_observation(
    inventory: &drivesync_engine::inventory::InventoryManager,
    item: &drivesync_engine::inventory::SyncItem,
    event: &str,
    name: &str,
) -> drivesync_engine::inventory::RecordRemoteObservation {
    use drivesync_engine::inventory::*;
    let account = inventory
        .get_sync_info(GetSyncInfoRequest {})
        .unwrap()
        .info
        .account_id;
    let reference = RemoteFileRef {
        account_id: account,
        file_id: RemoteFileId("remote-file".into()),
    };
    let remote = RemoteFile {
        reference: reference.clone(),
        provider: ProviderId("google".into()),
        name: MetadataValue::Known(name.into()),
        kind: MetadataValue::Known(RemoteEntryKind::File),
        mime_type: MetadataValue::NotCollected,
        parent_ids: MetadataValue::Known(vec![RemoteFileId("root".into())]),
        trashed: MetadataValue::Known(false),
        version: MetadataValue::Known("1".into()),
        created_time: MetadataValue::NotCollected,
        modified_time: MetadataValue::NotCollected,
        size: MetadataValue::NotCollected,
        md5_checksum: MetadataValue::NotCollected,
        sha256_checksum: MetadataValue::NotCollected,
        provider_data: MetadataValue::NotCollected,
    };
    let observed = File {
        id: FileId(event.into()),
        sync_id: SyncId("sync-first".into()),
        observed_at: Timestamp { unix_millis: 10 },
        coverage: ObservationCoverage::Partial,
        scope: FileScope::Remote(reference.clone()),
        data: Some(FileData::Remote(remote)),
        event: FileEvent {
            kind: FileEventKind::Observed,
            local: None,
            remote: Some(drivesync_engine::models::RemoteObservation {
                source: RemoteObservationSource::Changes,
                removed: Some(false),
                time: None,
                change_type: None,
                drive_id: None,
            }),
            uncertainty: None,
            previous: None,
            current: Some(FileLocation::Remote {
                reference,
                parent_id: Some(RemoteFileId("root".into())),
                name: Some(name.into()),
            }),
        },
    };
    RecordRemoteObservation {
        selector: ObservationTarget::Existing(item.id.clone()),
        expected_revision: Some(item.revision),
        observation: drivesync_engine::inventory::RemoteObservation {
            observed,
            presence: PresenceState::Present,
            proof: ObservationProof {
                source: ObservationSource::RemoteChange,
                observed_at: Timestamp { unix_millis: 10 },
                scan_or_generation: None,
                complete_scope: false,
            },
        },
    }
}
#[tokio::test]
async fn complete_scan_records_decisions_before_live_delivery_without_executing_operations() {
    use drivesync_engine::inventory::*;
    let (tree, connector) = Tree::new().await;
    std::fs::create_dir(tree.0.join("first/folder")).unwrap();
    std::fs::write(tree.0.join("first/folder/file"), b"unchanged").unwrap();
    let session = tree.open(connector, "first");
    let report = session.prepare().await.unwrap();
    assert!(report.ready_for_analysis());
    settle_flow(&session);
    let inventory = inventory(&tree, "first");
    let entries = items(&inventory);
    let records = analyses(&inventory);
    assert_eq!(records.len(), entries.len());
    assert_eq!(records.len(), 2);
    for record in records {
        assert!(!record.stale);
        assert_eq!(record.disposition, AnalysisDisposition::PendingEvidence);
        assert!(record
            .requirements
            .contains(&AnalysisRequirement::ObserveRemote));
        assert_eq!(record.evidence.local_content, EvidenceRelation::Unknown);
        assert_eq!(record.evidence.current_content, EvidenceRelation::Unknown);
        assert_eq!(record.decisions.len(), 1);
        assert_eq!(
            record.decisions[0].intent,
            AnalysisIntent {
                direction: OperationDirection::Upload,
                kind: OperationKind::Create,
            }
        );
    }
    assert!(entries.iter().all(|item| item.baseline.is_none()));
    assert_eq!(
        std::fs::read(tree.0.join("first/folder/file")).unwrap(),
        b"unchanged"
    );
    assert_eq!(session.diagnostics().delivery, Some(DeliveryState::Enabled));
    assert!(session.diagnostics().confirmed_events.is_some());
    session.stop_detector(DetectorStopMode::StopNow).unwrap();
}
#[tokio::test]
async fn persisted_remote_rename_produces_only_a_blocked_decision_and_preserves_baseline_and_file()
{
    use drivesync_engine::inventory::*;
    let (tree, connector) = Tree::new().await;
    std::fs::write(tree.0.join("first/file"), b"local-content").unwrap();
    let session = tree.open(connector, "first");
    session.prepare().await.unwrap();
    settle_flow(&session);
    let inventory = inventory(&tree, "first");
    let item = items(&inventory).remove(0);
    let item = inventory
        .record_remote(remote_observation(
            &inventory,
            &item,
            "remote-initial",
            "file",
        ))
        .unwrap()
        .item;
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
                    confirmed_at: Timestamp { unix_millis: 20 },
                },
                status: SyncStatus::Synced,
                verified_at: Timestamp { unix_millis: 20 },
            },
            completed_operation: None,
            expected_operation_attempt: None,
            activity: None,
        })
        .unwrap()
        .item;
    let changed = inventory
        .record_remote(remote_observation(
            &inventory,
            &item,
            "remote-rename",
            "remote-renamed",
        ))
        .unwrap()
        .item;
    assert_eq!(changed.status, SyncStatus::Synced);
    session.prepare().await.unwrap();
    let record = inventory
        .get_analysis(GetAnalysisRequest {
            selector: AnalysisSelector::LatestForItem(item.id.clone()),
        })
        .unwrap()
        .analysis
        .unwrap();
    assert!(!record.stale);
    assert_eq!(record.decisions.len(), 1);
    assert_eq!(
        record.decisions[0].intent,
        AnalysisIntent {
            direction: OperationDirection::Download,
            kind: OperationKind::Rename
        }
    );
    assert!(record
        .requirements
        .contains(&AnalysisRequirement::VerifyContent));
    let op = inventory
        .list_operations(ListOperationsRequest {
            filter: OperationFilter::default(),
            pagination: Pagination::default(),
        })
        .unwrap()
        .page
        .entries
        .remove(0);
    assert_eq!(op.state, OperationState::Blocked);
    assert_eq!(op.attempt_count, 0);
    assert!(op.started_at.is_none());
    assert_eq!(
        std::fs::read(tree.0.join("first/file")).unwrap(),
        b"local-content"
    );
    assert!(!tree.0.join("first/remote-renamed").exists());
    assert_eq!(items(&inventory)[0].baseline, changed.baseline);
    session.stop_detector(DetectorStopMode::StopNow).unwrap();
    drop(session);
    drop(inventory);
    let reopened = self::inventory(&tree, "first");
    assert_eq!(analyses(&reopened).last().unwrap(), &record);
}
#[tokio::test]
async fn known_remote_absence_creates_an_upload_intent_without_uploading() {
    use drivesync_engine::inventory::*;
    let (tree, connector) = Tree::new().await;
    std::fs::write(tree.0.join("first/file"), b"stay-local").unwrap();
    let session = tree.open(connector, "first");
    session.prepare().await.unwrap();
    settle_flow(&session);
    let inventory = inventory(&tree, "first");
    let item = items(&inventory).remove(0);
    let item = inventory
        .record_remote(remote_observation(
            &inventory,
            &item,
            "remote-initial",
            "file",
        ))
        .unwrap()
        .item;
    let mut absent = remote_observation(&inventory, &item, "remote-absent", "file");
    absent.observation.observed.data = None;
    absent.observation.observed.event.current = None;
    absent.observation.presence = PresenceState::AbsentConfirmed;
    absent.observation.proof = ObservationProof {
        source: ObservationSource::RemoteListing,
        observed_at: Timestamp { unix_millis: 11 },
        scan_or_generation: Some("observed-listing".into()),
        complete_scope: true,
    };
    inventory.record_remote(absent).unwrap();
    session.prepare().await.unwrap();
    let record = inventory
        .get_analysis(GetAnalysisRequest {
            selector: AnalysisSelector::LatestForItem(item.id),
        })
        .unwrap()
        .analysis
        .unwrap();
    assert_eq!(
        record.decisions[0].intent,
        AnalysisIntent {
            direction: OperationDirection::Upload,
            kind: OperationKind::Create
        }
    );
    assert_eq!(record.disposition, AnalysisDisposition::PendingEvidence);
    let ops = inventory
        .list_operations(ListOperationsRequest {
            filter: OperationFilter::default(),
            pagination: Pagination::default(),
        })
        .unwrap()
        .page
        .entries;
    assert!(ops
        .iter()
        .all(|op| op.state == OperationState::Blocked && op.attempt_count == 0));
    assert_eq!(
        std::fs::read(tree.0.join("first/file")).unwrap(),
        b"stay-local"
    );
    assert!(session.diagnostics().confirmed_events.is_some());
    session.stop_detector(DetectorStopMode::StopNow).unwrap();
}
#[tokio::test]
async fn partial_scan_does_not_publish_new_analyses_until_coverage_is_restored() {
    use drivesync_engine::orchestrator::PreparationState;
    let (tree, connector) = Tree::new().await;
    std::fs::create_dir(tree.0.join("first/locked")).unwrap();
    std::fs::write(tree.0.join("first/locked/file"), b"data").unwrap();
    let session = tree.open(connector, "first");
    session.prepare().await.unwrap();
    settle_flow(&session);
    let inventory = inventory(&tree, "first");
    let before = analyses(&inventory).len();
    std::fs::set_permissions(
        tree.0.join("first/locked"),
        std::fs::Permissions::from_mode(0o000),
    )
    .unwrap();
    let result = session.prepare().await;
    std::fs::set_permissions(
        tree.0.join("first/locked"),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    assert!(!result.unwrap().ready_for_analysis());
    assert_eq!(
        session.preparation_status().state,
        PreparationState::Partial
    );
    assert_eq!(analyses(&inventory).len(), before);
    session.prepare().await.unwrap();
    assert!(analyses(&inventory).len() > before);
    session.stop_detector(DetectorStopMode::StopNow).unwrap();
}
#[tokio::test]
async fn analysis_persistence_failure_is_visible_and_preparation_can_retry() {
    use drivesync_engine::orchestrator::{PreparationFailure, PreparationState};
    let (tree, connector) = Tree::new().await;
    std::fs::write(tree.0.join("first/file"), b"data").unwrap();
    let session = tree.open(connector, "first");
    session.prepare().await.unwrap();
    settle_flow(&session);
    let inventory = inventory(&tree, "first");
    let before = analyses(&inventory).len();
    let db = rusqlite::Connection::open(tree.0.join("first/.drivesync/inventory.sqlite3")).unwrap();
    db.execute_batch("CREATE TRIGGER reject_analysis BEFORE INSERT ON analyses BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert!(matches!(
        session.prepare().await,
        Err(PreparationFailure::Analyzer(_))
    ));
    assert_eq!(session.preparation_status().state, PreparationState::Failed);
    assert!(session
        .preparation_status()
        .report
        .unwrap()
        .ready_for_analysis());
    assert_eq!(analyses(&inventory).len(), before);
    assert!(session.diagnostics().confirmed_events.is_some());
    db.execute_batch("DROP TRIGGER reject_analysis;").unwrap();
    assert!(session.prepare().await.unwrap().ready_for_analysis());
    assert!(analyses(&inventory).len() > before);
    session.stop_detector(DetectorStopMode::StopNow).unwrap();
}
#[tokio::test]
async fn decision_passes_are_isolated_between_accounts_and_survive_reopening() {
    let (tree, connector) = Tree::new().await;
    std::fs::write(tree.0.join("first/first-file"), b"first").unwrap();
    std::fs::write(tree.0.join("second/second-file"), b"second").unwrap();
    let first = tree.open(connector.clone(), "first");
    let second = tree.open(connector.clone(), "second");
    let (a, b) = tokio::join!(first.prepare(), second.prepare());
    assert!(a.unwrap().ready_for_analysis());
    assert!(b.unwrap().ready_for_analysis());
    let first_inv = inventory(&tree, "first");
    let second_inv = inventory(&tree, "second");
    let first_record = analyses(&first_inv).remove(0);
    let second_record = analyses(&second_inv).remove(0);
    assert_ne!(first_record.item_id, second_record.item_id);
    assert!(items(&first_inv).iter().all(|i| i
        .local
        .as_ref()
        .is_some_and(|l| l.relative_path.as_bytes() == b"first-file")));
    assert!(items(&second_inv).iter().all(|i| i
        .local
        .as_ref()
        .is_some_and(|l| l.relative_path.as_bytes() == b"second-file")));
    first.stop_detector(DetectorStopMode::StopNow).unwrap();
    second.stop_detector(DetectorStopMode::StopNow).unwrap();
    drop(first);
    drop(second);
    drop(first_inv);
    drop(second_inv);
    let reopened = tree.open(connector, "first");
    reopened.prepare().await.unwrap();
    assert!(analyses(&inventory(&tree, "first"))
        .iter()
        .any(|a| a.id == first_record.id));
    assert_eq!(analyses(&inventory(&tree, "second")), vec![second_record]);
    reopened.stop_detector(DetectorStopMode::StopNow).unwrap();
}

#[test]
fn scan_and_decision_preparation_supports_callers_without_a_tokio_runtime() {
    use std::{
        future::Future,
        sync::Arc,
        task::{Context, Poll, Wake, Waker},
    };
    struct WakeThread(std::thread::Thread);
    impl Wake for WakeThread {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    fn block_on<F: Future>(future: F) -> F::Output {
        let waker = Waker::from(Arc::new(WakeThread(std::thread::current())));
        let mut context = Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::park_timeout(Duration::from_secs(2)),
            }
        }
    }
    let (tree, connector) = block_on(Tree::new());
    std::fs::write(tree.0.join("first/file"), b"unchanged").unwrap();
    let session = tree.open(connector, "first");
    assert!(block_on(session.prepare()).unwrap().ready_for_analysis());
    assert_eq!(analyses(&inventory(&tree, "first")).len(), 1);
    assert_eq!(
        std::fs::read(tree.0.join("first/file")).unwrap(),
        b"unchanged"
    );
    session.stop_detector(DetectorStopMode::StopNow).unwrap();
}

#[tokio::test]
async fn configured_root_alias_mismatch_does_not_invent_different_parent_evidence() {
    use drivesync_engine::inventory::*;
    let (tree, connector) = Tree::new().await;
    std::fs::write(tree.0.join("first/file"), b"data").unwrap();
    let session = tree.open(connector, "first");
    session.prepare().await.unwrap();
    settle_flow(&session);
    let inventory = inventory(&tree, "first");
    let item = items(&inventory).remove(0);
    let mut observed = remote_observation(&inventory, &item, "canonical-root", "file");
    if let Some(FileData::Remote(remote)) = &mut observed.observation.observed.data {
        remote.parent_ids = MetadataValue::Known(vec![RemoteFileId("canonical-root-id".into())]);
    }
    if let Some(FileLocation::Remote { parent_id, .. }) =
        &mut observed.observation.observed.event.current
    {
        *parent_id = Some(RemoteFileId("canonical-root-id".into()));
    }
    inventory.record_remote(observed).unwrap();
    session.prepare().await.unwrap();
    let record = inventory
        .get_analysis(GetAnalysisRequest {
            selector: AnalysisSelector::LatestForItem(item.id),
        })
        .unwrap()
        .analysis
        .unwrap();
    assert_eq!(record.evidence.current_parents, EvidenceRelation::Unknown);
    assert!(record
        .decisions
        .iter()
        .all(|d| d.intent.kind != OperationKind::Move));
    session.stop_detector(DetectorStopMode::StopNow).unwrap();
}

#[tokio::test]
async fn live_ingestion_failures_keep_events_for_recovery_and_exact_file_receipts_are_not_duplicated(
) {
    use drivesync_engine::{inventory::*, orchestrator::PreparationState};
    for table in ["receipts", "analyses"] {
        let (tree, connector) = Tree::new().await;
        let root = tree.0.join("first");
        std::fs::write(root.join("file"), b"before").unwrap();
        let session = tree.open(connector, "first");
        session.prepare().await.unwrap();
        settle_flow(&session);
        let inv = inventory(&tree, "first");
        let before = session.diagnostics().confirmed_events.unwrap();
        let db = rusqlite::Connection::open(root.join(".drivesync/inventory.sqlite3")).unwrap();
        db.execute_batch(&format!("CREATE TRIGGER reject_live BEFORE INSERT ON {table} BEGIN SELECT RAISE(ABORT,'injected'); END;")).unwrap();
        std::fs::write(root.join("file"), b"after").unwrap();
        wait(|| {
            session.preparation_status().state == PreparationState::Failed
                && session.diagnostics().delivery == Some(DeliveryState::Paused)
        });
        assert_eq!(session.diagnostics().confirmed_events, Some(before));
        assert_eq!(session.diagnostics().leased_events, Some(0));
        assert!(session.diagnostics().pending_events.unwrap() > 0);
        let event = session
            .diagnostics()
            .events
            .iter()
            .rev()
            .find(|e| {
                e.scope == FileScope::Local(LocalRelativePath::new(b"file".to_vec()).unwrap())
            })
            .unwrap()
            .id
            .clone();
        db.execute_batch("DROP TRIGGER reject_live;").unwrap();
        session.prepare().await.unwrap();
        settle_flow(&session);
        assert!(session.diagnostics().confirmed_events.unwrap() > before);
        assert_eq!(session.diagnostics().pending_events, Some(0));
        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM receipts WHERE event=?1",
                [&event.0],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "the same File.id must be ingested exactly once");
        let current = inv
            .list_analyses(ListAnalysesRequest {
                filter: AnalysisFilter {
                    current_only: true,
                    ..Default::default()
                },
                pagination: Pagination::default(),
            })
            .unwrap()
            .page
            .entries;
        assert_eq!(current.len(), 1);
        assert_eq!(std::fs::read(root.join("file")).unwrap(), b"after");
        session.stop_detector(DetectorStopMode::StopNow).unwrap();
    }
}
#[tokio::test]
async fn live_rename_delete_and_stop_restart_follow_scanner_observations_without_executing_operations(
) {
    use drivesync_engine::inventory::*;
    let (tree, connector) = Tree::new().await;
    let root = tree.0.join("first");
    std::fs::write(root.join("original"), b"user bytes").unwrap();
    let session = tree.open(connector, "first");
    session.prepare().await.unwrap();
    settle_flow(&session);
    let inv = inventory(&tree, "first");
    std::fs::rename(root.join("original"), root.join("renamed")).unwrap();
    wait(|| {
        items(&inv).iter().any(|i| {
            i.local_presence == PresenceState::Present
                && i.local
                    .as_ref()
                    .is_some_and(|f| f.relative_path.as_bytes() == b"renamed")
        })
    });
    settle_flow(&session);
    assert!(!root.join("original").exists());
    assert_eq!(std::fs::read(root.join("renamed")).unwrap(), b"user bytes");
    std::fs::remove_file(root.join("renamed")).unwrap();
    wait(|| {
        items(&inv)
            .iter()
            .filter(|i| {
                i.local
                    .as_ref()
                    .is_some_and(|f| f.relative_path.as_bytes() == b"renamed")
            })
            .all(|i| i.local_presence == PresenceState::AbsentConfirmed)
    });
    settle_flow(&session);
    assert!(items(&inv).iter().all(|i| i.baseline.is_none()));
    assert!(inv
        .list_operations(ListOperationsRequest {
            filter: OperationFilter::default(),
            pagination: Pagination::default()
        })
        .unwrap()
        .page
        .entries
        .iter()
        .all(|op| op.state == OperationState::Blocked && op.attempt_count == 0));
    session.stop_detector(DetectorStopMode::StopNow).unwrap();
    let detected = session.diagnostics().events_detected;
    std::fs::write(root.join("created-while-stopped"), b"new").unwrap();
    assert_eq!(session.diagnostics().events_detected, detected);
    session.prepare().await.unwrap();
    settle_flow(&session);
    assert!(items(&inv).iter().any(|i| i
        .local
        .as_ref()
        .is_some_and(|f| f.relative_path.as_bytes() == b"created-while-stopped")));
    session.stop_detector(DetectorStopMode::StopNow).unwrap();
}

#[tokio::test]
async fn live_local_and_remote_renames_identify_manual_intervention() {
    use drivesync_engine::inventory::*;
    let (tree, connector) = Tree::new().await;
    std::fs::write(tree.0.join("first/file"), b"preserve both versions").unwrap();
    let session = tree.open(connector, "first");
    session.prepare().await.unwrap();
    settle_flow(&session);
    let inv = inventory(&tree, "first");
    let item = items(&inv).remove(0);
    let item = inv
        .record_remote(remote_observation(&inv, &item, "remote-initial", "file"))
        .unwrap()
        .item;
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
    // This fixture represents a previously verified synchronization, not an effect of analysis.
    let item = inv
        .confirm_sync(ConfirmSyncRequest {
            item_id: item.id.clone(),
            expected_item_revision: item.revision,
            confirmed: ConfirmedState {
                baseline: SyncBaseline {
                    local,
                    remote,
                    confirmed_at: Timestamp { unix_millis: 20 },
                },
                status: SyncStatus::Synced,
                verified_at: Timestamp { unix_millis: 20 },
            },
            completed_operation: None,
            expected_operation_attempt: None,
            activity: None,
        })
        .unwrap()
        .item;
    let item = inv
        .record_remote(remote_observation(
            &inv,
            &item,
            "remote-renamed",
            "remote-name",
        ))
        .unwrap()
        .item;
    let baseline = item.baseline.clone();
    std::fs::rename(tree.0.join("first/file"), tree.0.join("first/local-name")).unwrap();
    wait(|| {
        inv.get_analysis(GetAnalysisRequest {
            selector: AnalysisSelector::LatestForItem(item.id.clone()),
        })
        .unwrap()
        .analysis
        .is_some_and(|a| !a.stale && a.disposition == AnalysisDisposition::PendingUserDecision)
    });
    settle_flow(&session);
    let record = inv
        .get_analysis(GetAnalysisRequest {
            selector: AnalysisSelector::LatestForItem(item.id.clone()),
        })
        .unwrap()
        .analysis
        .unwrap();
    assert!(record
        .findings
        .iter()
        .any(|f| f.dimension == AnalysisDimension::Name
            && f.status == AnalysisFindingStatus::Divergent
            && f.reason == AnalysisReason::ConcurrentChanges));
    assert!(record.decisions.is_empty());
    assert_eq!(
        std::fs::read(tree.0.join("first/local-name")).unwrap(),
        b"preserve both versions"
    );
    assert!(!tree.0.join("first/remote-name").exists());
    assert_eq!(
        inv.get_item(GetItemRequest {
            selector: ItemSelector::Id(item.id)
        })
        .unwrap()
        .item
        .unwrap()
        .baseline,
        baseline
    );
    assert!(inv
        .list_operations(ListOperationsRequest {
            filter: Default::default(),
            pagination: Default::default()
        })
        .unwrap()
        .page
        .entries
        .iter()
        .all(|o| o.state == OperationState::Blocked && o.attempt_count == 0));
    session.stop_detector(DetectorStopMode::StopNow).unwrap();
}

#[tokio::test]
async fn observed_content_hashes_drive_real_upload_decisions_without_changing_files() {
    use drivesync_engine::inventory::*;
    use sha2::{Digest, Sha256};
    let (tree, connector) = Tree::new().await;
    let path = tree.0.join("first/file");
    std::fs::write(&path, b"original").unwrap();
    let session = tree.open(connector, "first");
    session.prepare().await.unwrap();
    settle_flow(&session);
    let inv = inventory(&tree, "first");
    let item = items(&inv).remove(0);
    let hash = format!("{:x}", Sha256::digest(b"original"));
    let mut observation = remote_observation(&inv, &item, "remote-content", "file");
    if let Some(FileData::Remote(r)) = &mut observation.observation.observed.data {
        r.sha256_checksum = MetadataValue::Known(hash.clone());
    }
    let item = inv.record_remote(observation).unwrap().item;
    let local = BaselineSide {
        presence: PresenceState::Present,
        name: None,
        local_name_bytes: Some(b"file".to_vec()),
        parent_identity: None,
        version: None,
        checksum: Some(format!("sha256:{hash}")),
        size: Some(8),
        modified_at: None,
    };
    let mut remote = local.clone();
    remote.local_name_bytes = None;
    remote.name = Some("file".into());
    let item = inv
        .confirm_sync(ConfirmSyncRequest {
            item_id: item.id.clone(),
            expected_item_revision: item.revision,
            confirmed: ConfirmedState {
                baseline: SyncBaseline {
                    local,
                    remote,
                    confirmed_at: Timestamp { unix_millis: 20 },
                },
                status: SyncStatus::Synced,
                verified_at: Timestamp { unix_millis: 20 },
            },
            completed_operation: None,
            expected_operation_attempt: None,
            activity: None,
        })
        .unwrap()
        .item;
    std::fs::write(&path, b"edited locally").unwrap();
    wait(|| {
        inv.get_analysis(GetAnalysisRequest {
            selector: AnalysisSelector::LatestForItem(item.id.clone()),
        })
        .unwrap()
        .analysis
        .is_some_and(|a| {
            !a.stale
                && a.decisions.iter().any(|d| {
                    d.intent.kind == OperationKind::UpdateContent
                        && d.intent.direction == OperationDirection::Upload
                })
        })
    });
    settle_flow(&session);
    let record = inv
        .get_analysis(GetAnalysisRequest {
            selector: AnalysisSelector::LatestForItem(item.id.clone()),
        })
        .unwrap()
        .analysis
        .unwrap();
    assert_eq!(record.evidence.local_content, EvidenceRelation::Different);
    assert_eq!(record.evidence.remote_content, EvidenceRelation::Equal);
    assert_eq!(std::fs::read(path).unwrap(), b"edited locally");
    let baseline = inv
        .get_item(GetItemRequest {
            selector: ItemSelector::Id(item.id),
        })
        .unwrap()
        .item
        .unwrap()
        .baseline;
    assert_eq!(baseline, item.baseline);
    session.stop_detector(DetectorStopMode::StopNow).unwrap();
}
