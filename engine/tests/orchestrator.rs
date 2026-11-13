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
