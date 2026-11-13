//! Minimal composition tested through the external library API and real files.
use drivesync_engine::{
    detectors::*,
    models::{File, FileScope, LocalRelativePath, SyncId},
    AccountId, Engine, EngineConfig, ListAccountsRequest, Orchestrator, Timestamp,
};
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};
struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "drivesync-orchestrator-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("fixture: {e}"),
            }
        }
    }
    async fn connector(&self) -> Engine {
        Engine::open(EngineConfig {
            data_directory: self.0.join("accounts"),
            application_id: "org.example.OrchestratorTests".into(),
            google_oauth_file: None,
        })
        .await
        .unwrap()
    }
    fn request(&self, root: PathBuf) -> (LocalDetectorCreateRequest, mpsc::Receiver<File>) {
        let (tx, rx) = mpsc::channel();
        (
            LocalDetectorCreateRequest {
                scope: LocalDetectorScope {
                    account_id: AccountId("supplied-account".into()),
                    sync_id: SyncId("supplied-sync".into()),
                    local_root: root,
                },
                notifications: Arc::new(Sink(tx)),
                settings: LocalDetectorSettings::default(),
            },
            rx,
        )
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
struct Sink(mpsc::Sender<File>);
impl LocalNotificationSink for Sink {
    fn notify(&self, file: File) -> LocalDetectorResult<()> {
        self.0.send(file).unwrap();
        Ok(())
    }
}
fn start(orchestrator: &Orchestrator) -> LocalDetectorResult<LocalStartResult> {
    orchestrator.start(LocalStartRequest {
        mode: LocalStartMode::Initial,
        requested_at: Timestamp { unix_millis: 0 },
    })
}
fn stop(orchestrator: &Orchestrator) {
    assert_eq!(
        orchestrator
            .stop(LocalStopRequest {
                reason: LocalStopReason::UserRequest,
                requested_at: Timestamp { unix_millis: 0 }
            })
            .unwrap()
            .in_flight,
        0
    );
}
#[tokio::test]
async fn composition_keeps_connector_available_without_starting_detection() {
    let tree = Tree::new();
    let connector = tree.connector().await;
    let (request, rx) = tree.request(tree.0.join("does-not-exist"));
    let orchestrator = Orchestrator::new(connector, request).unwrap();
    let status = orchestrator.status(LocalStatusRequest {}).unwrap();
    assert_eq!(status.state, LocalDetectorState::Stopped);
    assert_eq!(status.activity, LocalActivity::Inactive);
    assert!(matches!(rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
    let connection = orchestrator.connector().clone();
    drop(orchestrator);
    assert!(connection
        .list_accounts(ListAccountsRequest { provider: None })
        .await
        .unwrap()
        .accounts
        .is_empty());
}
#[tokio::test]
async fn starts_real_detector_and_stops_it_without_replacing_connector() {
    let tree = Tree::new();
    let root = tree.0.join("local");
    fs::create_dir(&root).unwrap();
    let (request, rx) = tree.request(root.clone());
    let orchestrator = Orchestrator::new(tree.connector().await, request).unwrap();
    assert!(start(&orchestrator).unwrap().scheduled_scan);
    assert!(!start(&orchestrator).unwrap().scheduled_scan);
    let first = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(first.scope, FileScope::WholeSync);
    fs::write(root.join("file"), b"observed").unwrap();
    let expected = FileScope::Local(LocalRelativePath::new(b"file".to_vec()).unwrap());
    loop {
        let event = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        if event.scope == expected {
            assert_eq!(event.validate(), Ok(()));
            break;
        }
    }
    stop(&orchestrator);
    stop(&orchestrator);
    assert_eq!(
        orchestrator.status(LocalStatusRequest {}).unwrap().state,
        LocalDetectorState::Stopped
    );
    assert!(orchestrator
        .connector()
        .list_accounts(ListAccountsRequest { provider: None })
        .await
        .unwrap()
        .accounts
        .is_empty());
}
#[tokio::test]
async fn detector_failure_remains_typed_and_does_not_disable_connector() {
    let tree = Tree::new();
    let (request, _rx) = tree.request(tree.0.join("missing"));
    let orchestrator = Orchestrator::new(tree.connector().await, request).unwrap();
    assert_eq!(
        start(&orchestrator).unwrap_err().kind,
        LocalDetectorError::RootUnavailable
    );
    assert_eq!(
        orchestrator.status(LocalStatusRequest {}).unwrap().state,
        LocalDetectorState::Failed
    );
    assert!(orchestrator
        .connector()
        .list_accounts(ListAccountsRequest { provider: None })
        .await
        .unwrap()
        .accounts
        .is_empty());
    stop(&orchestrator);
}

#[tokio::test]
async fn observation_sessions_keep_history_and_stop_isolated_by_account() {
    let tree = Tree::new();
    let connector = tree.connector().await;
    let mut sessions = Vec::new();
    for name in ["first", "second"] {
        let root = tree.0.join(name);
        fs::create_dir(&root).unwrap();
        let session = Orchestrator::observe(
            connector.clone(),
            LocalDetectorScope {
                account_id: AccountId(name.into()),
                sync_id: SyncId(format!("sync-{name}")),
                local_root: root,
            },
            LocalDetectorSettings::default(),
        )
        .unwrap();
        start(&session).unwrap();
        sessions.push(session);
    }
    let wait = |session: &Orchestrator, count: u64| {
        let until = std::time::Instant::now() + Duration::from_secs(5);
        while session.diagnostics().events_detected < count {
            assert!(std::time::Instant::now() < until);
            std::thread::yield_now();
        }
    };
    for session in &sessions {
        wait(session, 1);
    }
    stop(&sessions[0]);
    let first = sessions[0].diagnostics();
    fs::write(tree.0.join("second/only-second"), b"observed").unwrap();
    let until = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let snapshot = sessions[1].diagnostics();
        assert_eq!(snapshot.account_id, AccountId("second".into()));
        assert_eq!(snapshot.pending_events, None);
        if snapshot.events.iter().any(|event| {
            event.scope
                == FileScope::Local(LocalRelativePath::new(b"only-second".to_vec()).unwrap())
        }) {
            break;
        }
        assert!(std::time::Instant::now() < until);
        std::thread::yield_now();
    }
    assert_eq!(
        sessions[0].diagnostics().events_detected,
        first.events_detected
    );
    assert_eq!(first.account_id, AccountId("first".into()));
    stop(&sessions[1]);
}

#[tokio::test]
async fn orchestrator_observes_normalized_save_and_explicit_startup_uncertainty() {
    use drivesync_engine::models::{FileEventKind, ObservationUncertainty};
    let tree = Tree::new();
    let root = tree.0.join("local");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("document"), b"before").unwrap();
    let (request, _) = tree.request(root.clone());
    let orchestrator =
        Orchestrator::observe(tree.connector().await, request.scope, request.settings).unwrap();
    start(&orchestrator).unwrap();
    let until = std::time::Instant::now() + Duration::from_secs(5);
    while orchestrator.diagnostics().events_detected == 0 {
        assert!(std::time::Instant::now() < until);
        std::thread::yield_now();
    }
    assert_eq!(
        orchestrator.diagnostics().events[0].event.uncertainty,
        Some(ObservationUncertainty::CaptureStarted)
    );
    fs::write(root.join("not-a-special-filename"), b"after").unwrap();
    fs::rename(root.join("not-a-special-filename"), root.join("document")).unwrap();
    let until = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let snapshot = orchestrator.diagnostics();
        if snapshot.events_detected > 1 {
            assert_eq!(snapshot.events_detected, 2);
            let saved = &snapshot.events[1];
            assert_eq!(
                saved.scope,
                FileScope::Local(LocalRelativePath::new(b"document".to_vec()).unwrap())
            );
            assert_eq!(saved.event.kind, FileEventKind::Modified);
            assert_eq!(saved.event.uncertainty, None);
            break;
        }
        assert!(std::time::Instant::now() < until);
        std::thread::yield_now();
    }
    stop(&orchestrator);
}

#[tokio::test]
async fn persistent_diagnostic_delivery_pause_survives_restart_and_resumes_pending_events() {
    let tree = Tree::new();
    let connector = tree.connector().await;
    let root = tree.0.join("durable-root");
    fs::create_dir(&root).unwrap();
    let (request, _) = tree.request(root.clone());
    let scope = request.scope;
    let open = || {
        Orchestrator::observe_persistent(
            connector.clone(),
            scope.clone(),
            LocalDetectorSettings::default(),
            tree.0.join("queue"),
            DeliveryState::Enabled,
        )
        .unwrap()
    };
    let first = open();
    start(&first).unwrap();
    let wait_for =
        |orchestrator: &Orchestrator,
         condition: &dyn Fn(&drivesync_engine::orchestrator::DetectorDiagnostics) -> bool| {
            let until = std::time::Instant::now() + Duration::from_secs(5);
            loop {
                let snapshot = orchestrator.diagnostics();
                if condition(&snapshot) {
                    break;
                }
                assert!(
                    std::time::Instant::now() < until,
                    "queue status: {snapshot:?}"
                );
                std::thread::yield_now();
            }
        };
    wait_for(&first, &|s| s.confirmed_events == Some(1));
    first.pause_delivery().unwrap();
    fs::write(root.join("while-paused"), b"retained").unwrap();
    wait_for(&first, &|s| s.pending_events == Some(1));
    let saved_id = first.diagnostics().events.last().unwrap().id.clone();
    drop(first);
    let reopened = open();
    assert_eq!(reopened.diagnostics().delivery, Some(DeliveryState::Paused));
    assert_eq!(reopened.diagnostics().pending_events, Some(1));
    assert!(reopened
        .diagnostics()
        .events
        .iter()
        .any(|event| event.id == saved_id));
    start(&reopened).unwrap();
    wait_for(&reopened, &|s| s.pending_events == Some(2));
    reopened.resume_delivery().unwrap();
    wait_for(&reopened, &|s| {
        s.pending_events == Some(0) && s.confirmed_events == Some(3)
    });
    assert_eq!(reopened.diagnostics().events_detected, 3);
    stop(&reopened);
}
