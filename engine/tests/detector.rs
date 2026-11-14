//! Consumer-facing unified lifecycle with real local capture and isolated Accounts metadata.
use drivesync_engine::{detectors::*, models::*, Engine, EngineConfig};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};
fn prepare(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(path.join("local")).unwrap();
    std::fs::create_dir_all(path.join("accounts")).unwrap();
    std::fs::set_permissions(
        path.join("accounts"),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    let file = path.join("accounts/accounts.json");
    if !file.exists() {
        std::fs::write(&file,br#"{"version":1,"application":"org.example.UnifiedDetector","intent":null,"records":{"account":{"account":{"id":"account","provider":"google","provider_user_id":"synthetic","email":"fixture@example.invalid","display_name":null,"state":"ReauthorizationRequired","created_at":{"unix_millis":0},"updated_at":{"unix_millis":0}},"key":"google:account:synthetic"}}}"#).unwrap();
        std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
}
fn open(path: &Path) -> Detector {
    prepare(path);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let connector = runtime
        .block_on(Engine::open(EngineConfig {
            data_directory: path.join("accounts"),
            application_id: "org.example.UnifiedDetector".into(),
            google_oauth_file: None,
        }))
        .unwrap();
    Detector::new(DetectorCreateRequest {
        connector,
        scope: DetectorScope {
            account_id: AccountId("account".into()),
            sync_id: SyncId("sync".into()),
            local_root: path.join("local"),
            remote_root_id: "root".into(),
        },
        local_settings: LocalDetectorSettings::default(),
        remote_settings: RemoteDetectorSettings::default(),
        data_directory: path.join("queues"),
        event_available_sink: None,
    })
    .unwrap()
    .detector
}
struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "detector-public-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn wait(test: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !test() {
        assert!(Instant::now() < deadline, "observation timeout");
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn start(detector: &Detector) {
    detector
        .start(DetectorStartRequest {
            mode: DetectorStartMode::Initial,
            delivery: DeliveryState::Paused,
        })
        .unwrap();
    wait(|| {
        detector
            .source_queue_status(DetectorOrigin::Local)
            .initial_observations
            > 0
            && detector
                .source_queue_status(DetectorOrigin::Remote)
                .detected
                > 0
    });
}
#[test]
fn real_capture_remains_healthy_when_remote_requires_authorization_and_pause_keeps_capture_active()
{
    let tree = Tree::new();
    let detector = open(&tree.0);
    assert_eq!(
        detector
            .status(DetectorStatusRequest {})
            .unwrap()
            .status
            .state,
        DetectorState::Stopped
    );
    start(&detector);
    let status = detector.status(DetectorStatusRequest {}).unwrap().status;
    assert_eq!(status.state, DetectorState::Degraded);
    assert_eq!(status.local.state, DetectorState::Running);
    assert_eq!(
        status.remote.last_error,
        Some(DetectorSourceError::Remote(
            RemoteError::AuthorizationRequired
        ))
    );
    std::fs::write(tree.0.join("local/file"), b"data").unwrap();
    wait(|| {
        detector
            .recent_events(DetectorOrigin::Local)
            .iter()
            .any(|file| file.data.is_some())
    });
    assert_eq!(
        detector
            .take_events(TakeEventsRequest {
                max_events: 32,
                lease_duration: Duration::from_secs(1)
            })
            .unwrap_err()
            .kind,
        DetectorError::DeliveryPaused
    );
    detector.resume(DetectorResumeRequest {}).unwrap();
    let batch = detector
        .take_events(TakeEventsRequest {
            max_events: 32,
            lease_duration: Duration::from_secs(1),
        })
        .unwrap()
        .batch;
    assert!(batch
        .files
        .iter()
        .any(|file| matches!(file.data, Some(FileData::Local(_)))));
    assert!(batch
        .files
        .iter()
        .any(|file| file.event.kind == FileEventKind::ScopeUnavailable));
    detector.pause(DetectorPauseRequest {}).unwrap();
    detector
        .acknowledge(AcknowledgeEventsRequest {
            batch_id: batch.batch_id.unwrap(),
            disposition: EventDisposition::Processed,
        })
        .unwrap();
    let count = detector.source_queue_status(DetectorOrigin::Local).detected;
    detector
        .request_scan(DetectorScanRequest {
            scope: DetectorScanScope::Scopes(vec![FileScope::Local(
                LocalRelativePath::new(b"file".to_vec()).unwrap(),
            )]),
            reason: DetectorScanReason::Manual,
            request_id: Some("local-check".into()),
        })
        .unwrap();
    wait(|| detector.source_queue_status(DetectorOrigin::Local).detected > count);
    detector
        .stop(DetectorStopRequest {
            mode: DetectorStopMode::StopNow,
        })
        .unwrap();
    let count = detector
        .status(DetectorStatusRequest {})
        .unwrap()
        .status
        .queue
        .detected;
    std::fs::write(tree.0.join("local/after-stop"), b"data").unwrap();
    std::thread::sleep(Duration::from_millis(120));
    assert_eq!(
        detector
            .status(DetectorStatusRequest {})
            .unwrap()
            .status
            .queue
            .detected,
        count
    );
}
#[test]
fn interrupted_detector_child() {
    let Some(path) = std::env::var_os("DRIVESYNC_UNIFIED_CHILD") else {
        return;
    };
    let path = PathBuf::from(path);
    let detector = open(&path);
    start(&detector);
    std::fs::write(path.join("local/durable"), b"survives crash").unwrap();
    wait(|| {
        detector
            .recent_events(DetectorOrigin::Local)
            .iter()
            .any(|file| file.data.is_some())
    });
    let event = detector
        .recent_events(DetectorOrigin::Local)
        .into_iter()
        .find(|file| file.data.is_some())
        .unwrap();
    std::fs::write(
        path.join("committed.tmp"),
        serde_json::to_vec(&event).unwrap(),
    )
    .unwrap();
    std::fs::rename(path.join("committed.tmp"), path.join("committed.json")).unwrap();
    loop {
        std::thread::park();
    }
}
#[test]
fn abruptly_terminated_unified_detector_replays_the_exact_committed_file() {
    let tree = Tree::new();
    prepare(&tree.0);
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "interrupted_detector_child", "--nocapture"])
        .env("DRIVESYNC_UNIFIED_CHILD", &tree.0)
        .spawn()
        .unwrap();
    wait(|| tree.0.join("committed.json").exists());
    child.kill().unwrap();
    child.wait().unwrap();
    let original: File =
        serde_json::from_slice(&std::fs::read(tree.0.join("committed.json")).unwrap()).unwrap();
    let detector = open(&tree.0);
    detector
        .start(DetectorStartRequest {
            mode: DetectorStartMode::Restart,
            delivery: DeliveryState::Enabled,
        })
        .unwrap();
    let batch = detector
        .take_events(TakeEventsRequest {
            max_events: 32,
            lease_duration: Duration::from_secs(1),
        })
        .unwrap()
        .batch;
    assert!(batch.files.contains(&original));
}
