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
        Orchestrator::observe_detector(
            DetectorCreateRequest {
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
            DeliveryState::Enabled,
        )
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
    session.start_detector(DetectorStartMode::Initial).unwrap();
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
async fn delivery_policy_and_pending_events_survive_reopen_without_crossing_accounts() {
    let (tree, connector) = Tree::new().await;
    let first = tree.open(connector.clone(), "first");
    let second = tree.open(connector.clone(), "second");
    first.start_detector(DetectorStartMode::Initial).unwrap();
    second.start_detector(DetectorStartMode::Initial).unwrap();
    wait(|| first.diagnostics().confirmed_events == Some(1));
    first.pause_delivery().unwrap();
    std::fs::write(tree.0.join("first/pending"), b"durable").unwrap();
    wait(|| first.diagnostics().pending_events == Some(1));
    assert_eq!(second.diagnostics().delivery, Some(DeliveryState::Enabled));
    let id = first.diagnostics().events.last().unwrap().id.clone();
    drop(first);
    let reopened = tree.open(connector, "first");
    assert_eq!(reopened.diagnostics().delivery, Some(DeliveryState::Paused));
    assert!(reopened.diagnostics().events.iter().any(|e| e.id == id));
    reopened.start_detector(DetectorStartMode::Restart).unwrap();
    wait(|| reopened.diagnostics().pending_events == Some(2));
    reopened.resume_delivery().unwrap();
    wait(|| reopened.diagnostics().pending_events == Some(0));
    reopened.stop_detector(DetectorStopMode::StopNow).unwrap();
    assert_eq!(
        second.local_status().unwrap().state,
        LocalDetectorState::Running
    );
}
