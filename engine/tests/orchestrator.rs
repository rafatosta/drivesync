//! The minimal orchestrator consumes only the unified detector facade.
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
#[tokio::test]
async fn paused_capture_survives_reopen_without_acknowledgement_or_crossing_accounts() {
    let (tree, connector) = Tree::new().await;
    let first = tree.open(connector.clone(), "first");
    let second = tree.open(connector.clone(), "second");
    first.prepare().await.unwrap();
    second.prepare().await.unwrap();
    wait(|| first.diagnostics().pending_events.is_some_and(|v| v >= 1));
    let before = first.diagnostics().pending_events.unwrap();
    std::fs::write(tree.0.join("first/pending"), b"durable").unwrap();
    wait(|| {
        first
            .diagnostics()
            .pending_events
            .is_some_and(|v| v > before)
    });
    assert_eq!(first.diagnostics().confirmed_events, Some(0));
    assert_eq!(first.diagnostics().delivery, Some(DeliveryState::Paused));
    assert_eq!(second.diagnostics().delivery, Some(DeliveryState::Paused));
    assert!(first
        .detector()
        .take_events(TakeEventsRequest {
            max_events: 10,
            lease_duration: Duration::from_secs(10)
        })
        .is_err());
    let id = first.diagnostics().events.last().unwrap().id.clone();
    first.stop_detector(DetectorStopMode::StopNow).unwrap();
    drop(first);
    let reopened = tree.open(connector, "first");
    let report = reopened.prepare().await.unwrap();
    assert!(report.ready_for_analysis());
    assert!(reopened.diagnostics().events.iter().any(|e| e.id == id));
    assert_eq!(reopened.diagnostics().confirmed_events, Some(0));
    let inventory = drivesync_engine::inventory::InventoryManager::open(
        drivesync_engine::inventory::OpenInventoryRequest {
            local_root: tree.0.join("first"),
            expected: drivesync_engine::inventory::SyncIdentity {
                sync_id: SyncId("sync-first".into()),
                account_id: AccountId("first".into()),
                remote_root_id: "root".into(),
            },
            create_if_missing: false,
        },
    )
    .unwrap()
    .inventory;
    let items = inventory
        .list_items(drivesync_engine::inventory::ListItemsRequest {
            filter: Default::default(),
            pagination: Default::default(),
        })
        .unwrap()
        .page
        .entries;
    assert!(items.iter().any(|item| item
        .local
        .as_ref()
        .is_some_and(|l| l.relative_path.as_bytes() == b"pending")));
    assert!(
        second
            .diagnostics()
            .events
            .iter()
            .all(|e| e.scope
                != FileScope::Local(LocalRelativePath::new(b"pending".to_vec()).unwrap()))
    );
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
    assert_eq!(session.diagnostics().confirmed_events, Some(0));
    assert!(session.prepare().await.unwrap().ready_for_analysis());
    assert_eq!(
        session.preparation_status().state,
        PreparationState::ReadyForAnalyzer
    );
    assert_eq!(session.diagnostics().delivery, Some(DeliveryState::Paused));
}
#[tokio::test]
async fn incompatible_inventory_is_preserved_and_never_consumes_capture_events() {
    use drivesync_engine::orchestrator::{PreparationFailure, PreparationState};
    let (tree, connector) = Tree::new().await;
    let session = tree.open(connector, "first");
    session.prepare().await.unwrap();
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
    assert_eq!(session.diagnostics().confirmed_events, Some(0));
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
    assert_eq!(owner.diagnostics().confirmed_events, Some(0));
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
    assert_eq!(session.diagnostics().confirmed_events, Some(0));
    assert_eq!(session.diagnostics().delivery, Some(DeliveryState::Paused));
}
