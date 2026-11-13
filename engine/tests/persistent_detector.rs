//! Public durable queue acceptance and recovery, using real SQLite and native capture.
use drivesync_engine::{
    detectors::*,
    models::{AccountId, FileScope, LocalRelativePath, SyncId, Timestamp},
};
use std::{
    fs,
    os::unix::fs::DirBuilderExt,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "drivesync-persistent-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        fs::create_dir(path.join("root")).unwrap();
        Self(path)
    }
    fn request(&self) -> PersistentDetectorRequest {
        request(&self.0)
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn request(base: &std::path::Path) -> PersistentDetectorRequest {
    PersistentDetectorRequest {
        scope: LocalDetectorScope {
            account_id: AccountId("account".into()),
            sync_id: SyncId("sync".into()),
            local_root: base.join("root"),
        },
        settings: LocalDetectorSettings::default(),
        data_directory: base.join("queue"),
    }
}
fn start(detector: &PersistentDetector) {
    detector
        .start(LocalStartRequest {
            mode: LocalStartMode::Initial,
            requested_at: Timestamp { unix_millis: 0 },
        })
        .unwrap();
}
fn wait(detector: &PersistentDetector, pending: u64) {
    let until = Instant::now() + Duration::from_secs(5);
    while detector.queue_status().pending < pending {
        assert!(
            Instant::now() < until,
            "queue did not receive durable events"
        );
        std::thread::yield_now();
    }
}
fn claim(detector: &PersistentDetector) -> FileBatch {
    detector
        .take_events(TakeEventsRequest {
            max_events: 32,
            lease_duration: Duration::from_secs(30),
        })
        .unwrap()
}
#[test]
fn pause_preserves_capture_and_reopen_redelivers_full_unconfirmed_events() {
    let tree = Tree::new();
    let detector = PersistentDetector::open(tree.request()).unwrap();
    assert_eq!(detector.delivery(), DeliveryState::Paused);
    start(&detector);
    wait(&detector, 1);
    fs::write(tree.0.join("root/file"), b"durable contents").unwrap();
    wait(&detector, 2);
    assert_eq!(
        detector
            .take_events(TakeEventsRequest {
                max_events: 1,
                lease_duration: Duration::from_secs(1)
            })
            .unwrap_err()
            .kind,
        QueueError::DeliveryPaused
    );
    detector.resume().unwrap();
    let batch = claim(&detector);
    assert_eq!(batch.files.len(), 2);
    let original = batch.files;
    let lease = batch.batch_id.unwrap();
    detector.pause().unwrap();
    assert_eq!(detector.release_events(&lease).unwrap(), 2);
    drop(detector);
    let detector = PersistentDetector::open(tree.request()).unwrap();
    assert_eq!(detector.queue_status().pending, 2);
    detector.resume().unwrap();
    let batch = claim(&detector);
    assert_eq!(batch.files, original);
    for file in &batch.files {
        assert_eq!(file.validate(), Ok(()));
    }
    let lease = batch.batch_id.unwrap();
    assert_eq!(
        detector
            .acknowledge(&lease, EventDisposition::Processed)
            .unwrap(),
        2
    );
    assert_eq!(
        detector
            .acknowledge(&lease, EventDisposition::Processed)
            .unwrap(),
        2
    );
    drop(detector);
    let detector = PersistentDetector::open(tree.request()).unwrap();
    assert_eq!(detector.queue_status().confirmed, 2);
    assert_eq!(detector.queue_status().pending, 0);
}
#[test]
fn interrupted_writer_child() {
    let Some(base) = std::env::var_os("DRIVESYNC_QUEUE_CRASH_FIXTURE") else {
        return;
    };
    let base = PathBuf::from(base);
    assert!(base
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("drivesync-persistent-"));
    let detector = PersistentDetector::open(request(&base)).unwrap();
    start(&detector);
    wait(&detector, 1);
    fs::write(
        base.join("root/from-child"),
        b"committed before abrupt exit",
    )
    .unwrap();
    wait(&detector, 2);
    // Skip Rust destructors, proving recovery does not depend on a graceful queue close.
    std::process::exit(0);
}
#[test]
fn abruptly_terminated_process_keeps_committed_events() {
    let tree = Tree::new();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "interrupted_writer_child", "--nocapture"])
        .env("DRIVESYNC_QUEUE_CRASH_FIXTURE", &tree.0)
        .status()
        .unwrap();
    assert!(status.success());
    let detector = PersistentDetector::open(tree.request()).unwrap();
    assert_eq!(detector.queue_status().pending, 2);
    detector.resume().unwrap();
    let batch = claim(&detector);
    assert!(batch.files.iter().any(|file| file.scope
        == FileScope::Local(LocalRelativePath::new(b"from-child".to_vec()).unwrap())));
    assert_eq!(batch.files[0].scope, FileScope::WholeSync);
}

#[test]
fn transient_sqlite_busy_retries_the_same_occurrence_without_another_file_change() {
    let tree = Tree::new();
    let detector = PersistentDetector::open(tree.request()).unwrap();
    let db = fs::read_dir(tree.0.join("queue"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path()
        .join("events.sqlite3");
    let lock = rusqlite::Connection::open(db).unwrap();
    lock.execute_batch("BEGIN IMMEDIATE").unwrap();
    start(&detector);
    let until = Instant::now() + Duration::from_secs(5);
    while detector.status(LocalStatusRequest {}).unwrap().last_error
        != Some(LocalDetectorError::NotificationFailed)
    {
        assert!(Instant::now() < until);
        std::thread::yield_now();
    }
    assert_eq!(detector.queue_status().pending, 0);
    lock.execute_batch("ROLLBACK").unwrap();
    wait(&detector, 1);
    assert_eq!(detector.queue_status().detected, 1);
    detector.resume().unwrap();
    let batch = claim(&detector);
    assert_eq!(batch.files.len(), 1);
    assert_eq!(batch.files[0].scope, FileScope::WholeSync);
}
