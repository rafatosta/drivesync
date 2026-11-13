//! Public local lifecycle and event delivery using real Linux filesystem notifications.
use drivesync_engine::{
    detectors::*,
    models::{
        AccountId, File, FileData, FileEventKind, FileScope, LocalRelativePath, MetadataValue,
        SyncId, Timestamp,
    },
};
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};
struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "drivesync-watch-{}-{}",
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
    fn scope(&self) -> LocalDetectorScope {
        LocalDetectorScope {
            account_id: AccountId("watch-account".into()),
            sync_id: SyncId(self.0.file_name().unwrap().to_string_lossy().into_owned()),
            local_root: self.0.clone(),
        }
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
fn start(detector: &LocalDetector) -> LocalStartResult {
    detector
        .start(LocalStartRequest {
            mode: LocalStartMode::Initial,
            requested_at: Timestamp { unix_millis: 0 },
        })
        .unwrap()
}
fn stop(detector: &LocalDetector) -> LocalStopResult {
    detector
        .stop(LocalStopRequest {
            reason: LocalStopReason::UserRequest,
            requested_at: Timestamp { unix_millis: 0 },
        })
        .unwrap()
}
fn status(detector: &LocalDetector) -> LocalStatusResult {
    detector.status(LocalStatusRequest {}).unwrap()
}
fn producer(tree: &Tree) -> (LocalDetector, mpsc::Receiver<File>) {
    let (tx, rx) = mpsc::channel();
    let result = LocalDetector::new(LocalDetectorCreateRequest {
        scope: tree.scope(),
        notifications: Arc::new(Sink(tx)),
        settings: LocalDetectorSettings::default(),
    })
    .unwrap();
    assert_eq!(result.state, LocalDetectorState::Stopped);
    (result.detector, rx)
}
fn receive(rx: &mpsc::Receiver<File>, predicate: impl Fn(&File) -> bool) -> File {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        let file = rx
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .expect("expected detector event");
        assert_eq!(file.validate(), Ok(()));
        if predicate(&file) {
            return file;
        }
    }
}
fn path(file: &File, expected: &[u8]) -> bool {
    file.scope == FileScope::Local(LocalRelativePath::new(expected.to_vec()).unwrap())
}
fn size(file: &File, expected: u64) -> bool {
    matches!(&file.data,Some(FileData::Local(data))if data.storage.size_bytes==MetadataValue::Known(expected))
}
fn ready(rx: &mpsc::Receiver<File>) {
    receive(rx, |f| f.scope == FileScope::WholeSync);
}
fn wait_state(detector: &LocalDetector, expected: LocalDetectorState) {
    let until = Instant::now() + Duration::from_secs(5);
    while status(detector).state != expected {
        assert!(
            Instant::now() < until,
            "state did not become {expected:?}: {:?}",
            status(detector)
        );
        std::thread::yield_now();
    }
}

#[test]
fn starts_real_capture_and_delivers_create_edit_rename_remove_as_shared_events() {
    let tree = Tree::new();
    let (detector, rx) = producer(&tree);
    let before = status(&detector);
    assert_eq!(before.state, LocalDetectorState::Stopped);
    assert_eq!(before.activity, LocalActivity::Inactive);
    assert_eq!(before.last_observation_at, None);
    assert!(start(&detector).scheduled_scan);
    ready(&rx);
    assert_eq!(status(&detector).state, LocalDetectorState::Running);
    assert!(!start(&detector).scheduled_scan);
    fs::write(tree.0.join("file"), b"one").unwrap();
    let created = receive(&rx, |f| path(f, b"file") && size(f, 3));
    assert_eq!(created.sync_id, tree.scope().sync_id);
    assert_eq!(created.event.kind, FileEventKind::RecheckRequired);
    fs::write(tree.0.join("file"), b"four").unwrap();
    let edited = receive(&rx, |f| path(f, b"file") && size(f, 4));
    assert_ne!(created.id, edited.id);
    fs::rename(tree.0.join("file"), tree.0.join("renamed")).unwrap();
    receive(&rx, |f| path(f, b"renamed") && size(f, 4));
    fs::remove_file(tree.0.join("renamed")).unwrap();
    let removed = receive(&rx, |f| path(f, b"renamed") && f.data.is_none());
    assert_eq!(removed.event.kind, FileEventKind::RecheckRequired);
    assert_eq!(stop(&detector).in_flight, 0);
    assert_eq!(stop(&detector).state, LocalDetectorState::Stopped);
    while rx.try_recv().is_ok() {}
    fs::write(tree.0.join("after-stop"), b"x").unwrap();
    assert!(matches!(
        rx.recv_timeout(Duration::from_millis(100)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
}
#[test]
fn adds_watches_for_new_subdirectories_without_polling_file_contents() {
    let tree = Tree::new();
    let (detector, rx) = producer(&tree);
    start(&detector);
    ready(&rx);
    fs::create_dir(tree.0.join("new")).unwrap();
    receive(&rx, |f| path(f, b"new") && f.data.is_some());
    fs::create_dir(tree.0.join("new/deep")).unwrap();
    receive(&rx, |f| path(f, b"new/deep") && f.data.is_some());
    fs::write(tree.0.join("new/deep/file"), b"nested").unwrap();
    receive(&rx, |f| path(f, b"new/deep/file") && size(f, 6));
    stop(&detector);
}
#[test]
fn reserved_changes_and_external_symlink_targets_are_not_observed() {
    use std::os::unix::fs::symlink;
    let tree = Tree::new();
    let outside = Tree::new();
    fs::create_dir(tree.0.join(".drivesync")).unwrap();
    symlink(&outside.0, tree.0.join("outside")).unwrap();
    let (detector, rx) = producer(&tree);
    start(&detector);
    ready(&rx);
    while rx.try_recv().is_ok() {}
    fs::write(tree.0.join(".drivesync/private"), b"internal").unwrap();
    fs::write(outside.0.join("file"), b"external").unwrap();
    assert!(matches!(
        rx.recv_timeout(Duration::from_millis(150)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    stop(&detector);
}
#[test]
fn start_failure_is_observable_and_failed_requires_stop_before_retry() {
    let tree = Tree::new();
    let (mut mut_scope, sink) = (tree.scope(), mpsc::channel::<File>().0);
    mut_scope.local_root = tree.0.join("missing");
    let detector = LocalDetector::new(LocalDetectorCreateRequest {
        scope: mut_scope,
        notifications: Arc::new(Sink(sink)),
        settings: LocalDetectorSettings::default(),
    })
    .unwrap()
    .detector;
    assert_eq!(status(&detector).state, LocalDetectorState::Stopped);
    let req = || LocalStartRequest {
        mode: LocalStartMode::Initial,
        requested_at: Timestamp { unix_millis: 0 },
    };
    assert_eq!(
        detector.start(req()).unwrap_err().kind,
        LocalDetectorError::RootUnavailable
    );
    assert_eq!(status(&detector).state, LocalDetectorState::Failed);
    assert_eq!(
        detector.start(req()).unwrap_err().kind,
        LocalDetectorError::InvalidState
    );
    stop(&detector);
}
#[test]
fn root_replacement_emits_unavailable_and_cannot_silently_bind_another_root() {
    let tree = Tree::new();
    fs::create_dir(tree.0.join("root")).unwrap();
    let (tx, rx) = mpsc::channel();
    let detector = LocalDetector::new(LocalDetectorCreateRequest {
        scope: LocalDetectorScope {
            local_root: tree.0.join("root"),
            ..tree.scope()
        },
        notifications: Arc::new(Sink(tx)),
        settings: LocalDetectorSettings::default(),
    })
    .unwrap()
    .detector;
    start(&detector);
    ready(&rx);
    fs::rename(tree.0.join("root"), tree.0.join("old")).unwrap();
    fs::create_dir(tree.0.join("root")).unwrap();
    receive(&rx, |f| {
        f.event.kind == FileEventKind::ScopeUnavailable && f.scope == FileScope::WholeSync
    });
    wait_state(&detector, LocalDetectorState::Failed);
    stop(&detector);
    assert_eq!(
        detector
            .start(LocalStartRequest {
                mode: LocalStartMode::Resume,
                requested_at: Timestamp { unix_millis: 0 }
            })
            .unwrap_err()
            .kind,
        LocalDetectorError::RootUnavailable
    );
}
#[test]
fn stopped_interval_is_rechecked_without_inventing_offline_history() {
    let tree = Tree::new();
    let (detector, rx) = producer(&tree);
    start(&detector);
    ready(&rx);
    stop(&detector);
    while rx.try_recv().is_ok() {}
    fs::write(tree.0.join("offline"), b"old").unwrap();
    start(&detector);
    ready(&rx);
    fs::write(tree.0.join("offline"), b"new content").unwrap();
    receive(&rx, |f| path(f, b"offline") && size(f, 11));
    stop(&detector);
}
struct RetrySink {
    attempts: mpsc::Sender<File>,
    accepted: mpsc::Sender<File>,
    allow: AtomicBool,
}
impl LocalNotificationSink for RetrySink {
    fn notify(&self, file: File) -> LocalDetectorResult<()> {
        if path(&file, b"file") && !self.allow.load(Ordering::Acquire) {
            self.attempts.send(file).unwrap();
            Err(LocalDetectorFailure {
                kind: LocalDetectorError::NotificationFailed,
                message: "controlled rejection".into(),
                retryable: true,
                progress: None,
            })
        } else {
            self.accepted.send(file).unwrap();
            Ok(())
        }
    }
}
#[test]
fn rejected_event_is_retried_unchanged_before_new_observations() {
    let tree = Tree::new();
    let (at, ar) = mpsc::channel();
    let (tx, rx) = mpsc::channel();
    let sink = Arc::new(RetrySink {
        attempts: at,
        accepted: tx,
        allow: AtomicBool::new(false),
    });
    let detector = LocalDetector::new(LocalDetectorCreateRequest {
        scope: tree.scope(),
        notifications: sink.clone(),
        settings: LocalDetectorSettings::default(),
    })
    .unwrap()
    .detector;
    start(&detector);
    ready(&rx);
    fs::write(tree.0.join("file"), b"original").unwrap();
    let rejected = ar.recv_timeout(Duration::from_secs(5)).unwrap();
    wait_state(&detector, LocalDetectorState::Degraded);
    sink.allow.store(true, Ordering::Release);
    fs::write(tree.0.join("wake"), b"trigger").unwrap();
    let accepted = receive(&rx, |f| f.id == rejected.id);
    assert_eq!(accepted, rejected);
    stop(&detector);
}
struct ReleaseOnDrop(Arc<(Mutex<bool>, std::sync::Condvar)>);
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        *self.0 .0.lock().unwrap() = true;
        self.0 .1.notify_all();
    }
}
struct BlockingSink {
    started: mpsc::Sender<()>,
    release: Arc<(Mutex<bool>, std::sync::Condvar)>,
    events: mpsc::Sender<File>,
}
impl LocalNotificationSink for BlockingSink {
    fn notify(&self, file: File) -> LocalDetectorResult<()> {
        if path(&file, b"blocked") {
            self.started.send(()).unwrap();
            let (lock, condition) = &*self.release;
            let mut go = lock.lock().unwrap();
            while !*go {
                go = condition.wait(go).unwrap();
            }
        }
        self.events.send(file).unwrap();
        Ok(())
    }
}
#[test]
fn status_does_not_wait_for_sink_and_stop_waits_for_callback_completion() {
    let tree = Tree::new();
    let (tx, rx) = mpsc::channel();
    let (st, started) = mpsc::channel();
    let release = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
    let detector = Arc::new(
        LocalDetector::new(LocalDetectorCreateRequest {
            scope: tree.scope(),
            notifications: Arc::new(BlockingSink {
                started: st,
                release: release.clone(),
                events: tx,
            }),
            settings: LocalDetectorSettings::default(),
        })
        .unwrap()
        .detector,
    );
    let _release_guard = ReleaseOnDrop(release.clone());
    start(&detector);
    ready(&rx);
    fs::write(tree.0.join("blocked"), b"x").unwrap();
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    let snapshot = status(&detector);
    assert_eq!(snapshot.activity, LocalActivity::Scanning);
    assert!(snapshot.observed_in_current_round > 0);
    let other = detector.clone();
    let (done_tx, done) = mpsc::channel();
    let stopper = std::thread::spawn(move || {
        let result = stop(&other);
        done_tx.send(result).unwrap();
    });
    wait_state(&detector, LocalDetectorState::Stopping);
    assert!(matches!(done.try_recv(), Err(mpsc::TryRecvError::Empty)));
    *release.0.lock().unwrap() = true;
    release.1.notify_all();
    assert_eq!(
        done.recv_timeout(Duration::from_secs(5)).unwrap().in_flight,
        0
    );
    stopper.join().unwrap();
    assert_eq!(status(&detector).activity, LocalActivity::Inactive);
}
#[test]
fn concurrent_start_is_idempotent_and_drop_stops_capture() {
    let tree = Tree::new();
    let (detector, rx) = producer(&tree);
    let detector = Arc::new(detector);
    std::thread::scope(|threads| {
        let first = threads.spawn(|| start(&detector));
        let second = threads.spawn(|| start(&detector));
        assert_eq!(
            u8::from(first.join().unwrap().scheduled_scan)
                + u8::from(second.join().unwrap().scheduled_scan),
            1
        );
    });
    ready(&rx);
    drop(detector);
    while rx.try_recv().is_ok() {}
    fs::write(tree.0.join("dropped"), b"x").unwrap();
    assert!(matches!(
        rx.recv_timeout(Duration::from_millis(100)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
}

#[test]
fn native_events_preserve_non_utf8_names() {
    use std::os::unix::ffi::OsStrExt;
    let tree = Tree::new();
    let (detector, rx) = producer(&tree);
    start(&detector);
    ready(&rx);
    let name = b"raw-\xff";
    fs::write(tree.0.join(std::ffi::OsStr::from_bytes(name)), b"bytes").unwrap();
    let event = receive(&rx, |f| path(f, name) && size(f, 5));
    assert_eq!(event.event.kind, FileEventKind::RecheckRequired);
    stop(&detector);
}

#[test]
fn ancestor_replacement_is_detected_without_file_activity() {
    let tree = Tree::new();
    let parent = tree.0.join("parent");
    fs::create_dir_all(parent.join("root")).unwrap();
    let (tx, rx) = mpsc::channel();
    let mut scope = tree.scope();
    scope.local_root = parent.join("root");
    let detector = LocalDetector::new(LocalDetectorCreateRequest {
        scope,
        notifications: Arc::new(Sink(tx)),
        settings: LocalDetectorSettings::default(),
    })
    .unwrap()
    .detector;
    start(&detector);
    ready(&rx);
    fs::rename(&parent, tree.0.join("old-parent")).unwrap();
    fs::create_dir_all(parent.join("root")).unwrap();
    receive(&rx, |f| f.event.kind == FileEventKind::ScopeUnavailable);
    wait_state(&detector, LocalDetectorState::Failed);
    stop(&detector);
}

#[test]
fn permanent_sink_failure_is_observable_and_stoppable() {
    struct Reject;
    impl LocalNotificationSink for Reject {
        fn notify(&self, _: File) -> LocalDetectorResult<()> {
            Err(LocalDetectorFailure {
                kind: LocalDetectorError::NotificationFailed,
                message: "rejected".into(),
                retryable: false,
                progress: None,
            })
        }
    }
    let tree = Tree::new();
    let detector = LocalDetector::new(LocalDetectorCreateRequest {
        scope: tree.scope(),
        notifications: Arc::new(Reject),
        settings: LocalDetectorSettings::default(),
    })
    .unwrap()
    .detector;
    start(&detector);
    wait_state(&detector, LocalDetectorState::Failed);
    assert_eq!(
        status(&detector).last_error,
        Some(LocalDetectorError::NotificationFailed)
    );
    assert_eq!(stop(&detector).in_flight, 0);
}
