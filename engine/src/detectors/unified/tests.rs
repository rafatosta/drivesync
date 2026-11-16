//! Real shared SQLite with independent source ports; no Orchestrator is constructed.
use super::*;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "unified-detector-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::create_dir(path.join("local")).unwrap();
        Self(path)
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
struct FakeSource {
    origin: DetectorOrigin,
    state: Mutex<DetectorState>,
    starts: AtomicUsize,
    stops: AtomicUsize,
    scans: AtomicUsize,
    fail: bool,
}
impl FakeSource {
    fn new(origin: DetectorOrigin, fail: bool) -> Arc<Self> {
        Arc::new(Self {
            origin,
            state: Mutex::new(DetectorState::Stopped),
            starts: AtomicUsize::new(0),
            stops: AtomicUsize::new(0),
            scans: AtomicUsize::new(0),
            fail,
        })
    }
}
impl Source for FakeSource {
    fn start(&self, _: DetectorStartMode) -> DetectorResult<()> {
        self.starts.fetch_add(1, Ordering::SeqCst);
        *self.state.lock().unwrap() = if self.fail {
            DetectorState::Failed
        } else {
            DetectorState::Running
        };
        if self.fail {
            return Err(DetectorFailure {
                kind: DetectorError::SourceUnavailable,
                retryable: true,
                source: Some(if self.origin == DetectorOrigin::Local {
                    DetectorSourceError::Local(LocalDetectorError::RootUnavailable)
                } else {
                    DetectorSourceError::Remote(RemoteError::RemoteUnavailable)
                }),
                queue: None,
            });
        }
        Ok(())
    }
    fn stop(&self, _: DetectorStopMode) -> DetectorResult<()> {
        self.stops.fetch_add(1, Ordering::SeqCst);
        *self.state.lock().unwrap() = DetectorState::Stopped;
        Ok(())
    }
    fn status(&self) -> DetectorSourceStatus {
        DetectorSourceStatus {
            state: *self.state.lock().unwrap(),
            last_error: None,
            local: None,
            remote: None,
        }
    }
    fn request_scan(&self, _: &DetectorScanRequest) -> DetectorResult<()> {
        self.scans.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
fn scope(tree: &Tree) -> DetectorScope {
    DetectorScope {
        account_id: AccountId("account".into()),
        sync_id: SyncId("sync".into()),
        local_root: tree.0.join("local"),
        remote_root_id: "remote-root".into(),
    }
}
fn store(tree: &Tree) -> Arc<Store> {
    let scope = scope(tree);
    Arc::new(
        Store::open_unified(
            &LocalDetectorScope {
                account_id: scope.account_id,
                sync_id: scope.sync_id,
                local_root: scope.local_root,
            },
            "google",
            &scope.remote_root_id,
        )
        .unwrap(),
    )
}
fn detector(
    tree: &Tree,
    local_fail: bool,
    remote_fail: bool,
) -> (Detector, Arc<FakeSource>, Arc<FakeSource>) {
    let local = FakeSource::new(DetectorOrigin::Local, local_fail);
    let remote = FakeSource::new(DetectorOrigin::Remote, remote_fail);
    (
        Detector::compose(
            scope(tree),
            store(tree),
            local.clone(),
            remote.clone(),
            None,
        )
        .unwrap(),
        local,
        remote,
    )
}
fn remote_file(id: &str) -> File {
    let reference = RemoteFileRef {
        account_id: AccountId("account".into()),
        file_id: RemoteFileId("resource".into()),
    };
    let data: RemoteFile = serde_json::from_value(serde_json::json!({
        "reference":{"account_id":"account","file_id":"resource"},"provider":"google","name":{"Known":"Document"},"kind":{"Known":"File"},
        "mime_type":"NotCollected","parent_ids":"NotCollected","trashed":"NotCollected","version":"NotCollected","created_time":"NotCollected","modified_time":"NotCollected","size":"NotCollected","md5_checksum":"NotCollected","sha256_checksum":"NotCollected","provider_data":"NotCollected"
    })).unwrap();
    File {
        id: FileId(id.into()),
        sync_id: SyncId("sync".into()),
        event: FileEvent {
            local: None,
            remote: None,
            kind: FileEventKind::Created,
            uncertainty: None,
            previous: None,
            current: Some(FileLocation::Remote {
                reference: reference.clone(),
                parent_id: None,
                name: Some("Document".into()),
            }),
        },
        observed_at: Timestamp { unix_millis: 20 },
        coverage: ObservationCoverage::Partial,
        scope: FileScope::Remote(reference),
        data: Some(FileData::Remote(data)),
    }
}
fn local_file(tree: &Tree, id: &str) -> File {
    std::fs::create_dir_all(tree.0.join("local")).unwrap();
    std::fs::write(tree.0.join("local/file"), b"contents").unwrap();
    let collector = LocalMetadataCollector::new(LocalDetectorScope {
        account_id: AccountId("account".into()),
        sync_id: SyncId("sync".into()),
        local_root: tree.0.join("local"),
    })
    .unwrap();
    let path = LocalRelativePath::new(b"file".to_vec()).unwrap();
    let data = collector
        .collect(LocalMetadataRequest {
            relative_path: path.clone(),
            cancellation: LocalCancellation::new(),
        })
        .unwrap()
        .file
        .unwrap();
    File {
        id: FileId(id.into()),
        sync_id: SyncId("sync".into()),
        event: FileEvent {
            local: None,
            remote: None,
            kind: FileEventKind::Created,
            uncertainty: None,
            previous: None,
            current: Some(FileLocation::Local(path.clone())),
        },
        observed_at: Timestamp { unix_millis: 10 },
        coverage: ObservationCoverage::Partial,
        scope: FileScope::Local(path),
        data: Some(FileData::Local(data)),
    }
}
fn start(detector: &Detector, delivery: DeliveryState) {
    detector
        .start(DetectorStartRequest {
            mode: DetectorStartMode::Initial,
            delivery,
        })
        .unwrap();
}
fn take(detector: &Detector) -> FileBatch {
    detector
        .take_events(TakeEventsRequest {
            max_events: 64,
            lease_duration: Duration::from_secs(30),
        })
        .unwrap()
        .batch
}
#[test]
fn starts_both_even_on_failure_and_status_preserves_the_healthy_source() {
    let tree = Tree::new();
    let (detector, local, remote) = detector(&tree, true, false);
    start(&detector, DeliveryState::Paused);
    let status = detector.status(DetectorStatusRequest {}).unwrap().status;
    assert_eq!(status.state, DetectorState::Degraded);
    assert_eq!(status.local.state, DetectorState::Failed);
    assert_eq!(status.remote.state, DetectorState::Running);
    assert_eq!(
        status.local.last_error,
        Some(DetectorSourceError::Local(
            LocalDetectorError::RootUnavailable
        ))
    );
    assert_eq!(local.starts.load(Ordering::SeqCst), 1);
    assert_eq!(remote.starts.load(Ordering::SeqCst), 1);
    detector
        .stop(DetectorStopRequest {
            mode: DetectorStopMode::StopNow,
        })
        .unwrap();
    assert_eq!(local.stops.load(Ordering::SeqCst), 1);
    assert_eq!(remote.stops.load(Ordering::SeqCst), 1);
}
#[test]
fn mixed_batch_is_fifo_complete_and_acknowledged_atomically() {
    let tree = Tree::new();
    let (detector, _, _) = detector(&tree, false, false);
    start(&detector, DeliveryState::Enabled);
    let local = local_file(&tree, "local");
    let remote = remote_file("remote");
    detector
        .store
        .enqueue_from(&local, DetectorOrigin::Local)
        .unwrap();
    detector
        .store
        .enqueue_from(&remote, DetectorOrigin::Remote)
        .unwrap();
    let batch = take(&detector);
    assert_eq!(batch.files, vec![local, remote]);
    assert_eq!(batch.first_observed_at, Some(Timestamp { unix_millis: 10 }));
    assert_eq!(batch.last_observed_at, Some(Timestamp { unix_millis: 20 }));
    let id = batch.batch_id.unwrap();
    assert_eq!(
        detector
            .acknowledge(AcknowledgeEventsRequest {
                batch_id: id.clone(),
                disposition: EventDisposition::Processed
            })
            .unwrap()
            .confirmed_count,
        2
    );
    assert_eq!(
        detector
            .acknowledge(AcknowledgeEventsRequest {
                batch_id: id,
                disposition: EventDisposition::Processed
            })
            .unwrap()
            .confirmed_count,
        2
    );
    assert_eq!(
        detector
            .status(DetectorStatusRequest {})
            .unwrap()
            .status
            .queue
            .confirmed,
        2
    );
}
#[test]
fn pause_keeps_both_sources_active_and_old_leases_can_be_released_or_acknowledged() {
    let tree = Tree::new();
    let (detector, local, remote) = detector(&tree, false, false);
    start(&detector, DeliveryState::Enabled);
    detector
        .store
        .enqueue_from(&remote_file("first"), DetectorOrigin::Remote)
        .unwrap();
    let batch = take(&detector);
    detector.pause(DetectorPauseRequest {}).unwrap();
    detector.pause(DetectorPauseRequest {}).unwrap();
    detector
        .store
        .enqueue_from(&remote_file("second"), DetectorOrigin::Remote)
        .unwrap();
    assert_eq!(
        detector
            .take_events(TakeEventsRequest {
                max_events: 1,
                lease_duration: Duration::from_secs(1)
            })
            .unwrap_err()
            .kind,
        DetectorError::DeliveryPaused
    );
    assert_eq!(local.status().state, DetectorState::Running);
    assert_eq!(remote.status().state, DetectorState::Running);
    detector
        .release_events(ReleaseEventsRequest {
            batch_id: batch.batch_id.unwrap(),
        })
        .unwrap();
    detector.resume(DetectorResumeRequest {}).unwrap();
    let batch = take(&detector);
    assert_eq!(batch.files.len(), 2);
    detector.pause(DetectorPauseRequest {}).unwrap();
    detector
        .acknowledge(AcknowledgeEventsRequest {
            batch_id: batch.batch_id.unwrap(),
            disposition: EventDisposition::Processed,
        })
        .unwrap();
}
#[test]
fn stop_pauses_delivery_stops_both_and_restart_preserves_unconfirmed_occurrences() {
    let tree = Tree::new();
    let (detector, local, remote) = detector(&tree, false, false);
    start(&detector, DeliveryState::Enabled);
    let file = remote_file("durable");
    detector
        .store
        .enqueue_from(&file, DetectorOrigin::Remote)
        .unwrap();
    detector
        .stop(DetectorStopRequest {
            mode: DetectorStopMode::DrainInFlight,
        })
        .unwrap();
    assert_eq!(
        detector
            .status(DetectorStatusRequest {})
            .unwrap()
            .status
            .state,
        DetectorState::Stopped
    );
    assert_eq!(local.status().state, DetectorState::Stopped);
    assert_eq!(remote.status().state, DetectorState::Stopped);
    assert_eq!(
        detector.resume(DetectorResumeRequest {}).unwrap_err().kind,
        DetectorError::InvalidState
    );
    detector
        .start(DetectorStartRequest {
            mode: DetectorStartMode::Restart,
            delivery: DeliveryState::Enabled,
        })
        .unwrap();
    assert_eq!(take(&detector).files, vec![file]);
}
#[test]
fn reopen_redelivers_expired_mixed_lease_without_changing_ids_or_payloads() {
    let tree = Tree::new();
    let (first, _, _) = detector(&tree, false, false);
    start(&first, DeliveryState::Enabled);
    let files = vec![local_file(&tree, "local"), remote_file("remote")];
    first
        .store
        .enqueue_from(&files[0], DetectorOrigin::Local)
        .unwrap();
    first
        .store
        .enqueue_from(&files[1], DetectorOrigin::Remote)
        .unwrap();
    first
        .take_events(TakeEventsRequest {
            max_events: 10,
            lease_duration: Duration::from_millis(5),
        })
        .unwrap();
    drop(first);
    std::thread::sleep(Duration::from_millis(10));
    let (second, _, _) = detector(&tree, false, false);
    start(&second, DeliveryState::Enabled);
    assert_eq!(take(&second).files, files);
}
#[test]
fn source_counters_distinguish_bootstrap_and_real_changes() {
    let tree = Tree::new();
    let (detector, _, _) = detector(&tree, false, false);
    start(&detector, DeliveryState::Paused);
    let initial = File {
        id: FileId("initial".into()),
        sync_id: SyncId("sync".into()),
        event: FileEvent {
            local: None,
            remote: None,
            kind: FileEventKind::RecheckRequired,
            uncertainty: Some(ObservationUncertainty::CaptureStarted),
            previous: None,
            current: None,
        },
        observed_at: Timestamp { unix_millis: 0 },
        coverage: ObservationCoverage::Partial,
        scope: FileScope::WholeSync,
        data: None,
    };
    detector
        .store
        .enqueue_from(&initial, DetectorOrigin::Remote)
        .unwrap();
    detector
        .store
        .enqueue_from(&remote_file("change"), DetectorOrigin::Remote)
        .unwrap();
    let queue = detector
        .status(DetectorStatusRequest {})
        .unwrap()
        .status
        .queue;
    assert_eq!(
        (
            queue.detected,
            queue.initial_observations,
            queue.changes_detected
        ),
        (2, 1, 1)
    );
    assert_eq!(
        detector.source_queue_status(DetectorOrigin::Local).detected,
        0
    );
    assert_eq!(detector.recent_events(DetectorOrigin::Remote).len(), 2);
}
#[test]
fn scoped_reobservation_validates_account_and_dispatches_without_inventory_or_scanner() {
    let tree = Tree::new();
    let (detector, local, remote) = detector(&tree, false, false);
    start(&detector, DeliveryState::Paused);
    detector
        .request_scan(DetectorScanRequest {
            scope: DetectorScanScope::All,
            reason: DetectorScanReason::Manual,
            request_id: Some("request".into()),
        })
        .unwrap();
    assert_eq!(local.scans.load(Ordering::SeqCst), 1);
    assert_eq!(remote.scans.load(Ordering::SeqCst), 1);
    assert_eq!(
        detector
            .request_scan(DetectorScanRequest {
                scope: DetectorScanScope::Scopes(vec![FileScope::Remote(RemoteFileRef {
                    account_id: AccountId("other".into()),
                    file_id: RemoteFileId("file".into())
                })]),
                reason: DetectorScanReason::Manual,
                request_id: None
            })
            .unwrap_err()
            .kind,
        DetectorError::InvalidScope
    );
}
#[test]
fn notification_is_after_durable_acceptance_and_never_confirms_events() {
    struct Notify(AtomicUsize);
    impl EventAvailableSink for Notify {
        fn notify_events_available(&self, _: SyncId) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let tree = Tree::new();
    let sink = Arc::new(Notify(AtomicUsize::new(0)));
    let detector = Detector::compose(
        scope(&tree),
        store(&tree),
        FakeSource::new(DetectorOrigin::Local, false),
        FakeSource::new(DetectorOrigin::Remote, false),
        Some(sink.clone()),
    )
    .unwrap();
    let file = remote_file("event");
    detector
        .store
        .enqueue_from(&file, DetectorOrigin::Remote)
        .unwrap();
    detector
        .store
        .enqueue_from(&file, DetectorOrigin::Remote)
        .unwrap();
    assert_eq!(sink.0.load(Ordering::SeqCst), 1);
    assert_eq!(
        detector.source_queue_status(DetectorOrigin::Remote).pending,
        1
    );
}
#[test]
fn concurrent_origins_share_one_global_enqueue_order_and_one_lease() {
    let tree = Tree::new();
    let (detector, _, _) = detector(&tree, false, false);
    start(&detector, DeliveryState::Enabled);
    let store1 = detector.store.clone();
    let store2 = detector.store.clone();
    let local = local_file(&tree, "local");
    let remote = remote_file("remote");
    let local_thread =
        std::thread::spawn(move || store1.enqueue_from(&local, DetectorOrigin::Local).unwrap());
    let remote_thread = std::thread::spawn(move || {
        store2
            .enqueue_from(&remote, DetectorOrigin::Remote)
            .unwrap()
    });
    local_thread.join().unwrap();
    remote_thread.join().unwrap();
    let batch = take(&detector);
    assert_eq!(batch.files.len(), 2);
    assert!(take(&detector).batch_id.is_none());
}

#[test]
fn confirmed_pause_wins_over_all_later_claims_and_does_not_stop_emission() {
    use std::sync::atomic::AtomicBool;
    let tree = Tree::new();
    let (detector, _, _) = detector(&tree, false, false);
    let detector = Arc::new(detector);
    start(&detector, DeliveryState::Enabled);
    detector
        .store
        .enqueue_from(&remote_file("queued"), DetectorOrigin::Remote)
        .unwrap();
    let paused = Arc::new(AtomicBool::new(false));
    let reading = detector.clone();
    let flag = paused.clone();
    let reader = std::thread::spawn(move || {
        for _ in 0..100 {
            let after = flag.load(Ordering::SeqCst);
            let result = reading.take_events(TakeEventsRequest {
                max_events: 1,
                lease_duration: Duration::from_secs(1),
            });
            if after {
                assert_eq!(result.unwrap_err().kind, DetectorError::DeliveryPaused);
            } else if let Ok(batch) = result {
                if let Some(id) = batch.batch.batch_id {
                    reading
                        .release_events(ReleaseEventsRequest { batch_id: id })
                        .unwrap();
                }
            }
        }
    });
    detector.pause(DetectorPauseRequest {}).unwrap();
    paused.store(true, Ordering::SeqCst);
    reader.join().unwrap();
    detector
        .store
        .enqueue_from(&remote_file("after-pause"), DetectorOrigin::Remote)
        .unwrap();
    assert_eq!(
        detector
            .status(DetectorStatusRequest {})
            .unwrap()
            .status
            .queue
            .pending,
        2
    );
}
#[test]
fn failed_notification_cannot_undo_enqueue_and_stop_is_idempotent() {
    struct Panic;
    impl EventAvailableSink for Panic {
        fn notify_events_available(&self, _: SyncId) {
            panic!("synthetic notification failure");
        }
    }
    let tree = Tree::new();
    let detector = Detector::compose(
        scope(&tree),
        store(&tree),
        FakeSource::new(DetectorOrigin::Local, false),
        FakeSource::new(DetectorOrigin::Remote, false),
        Some(Arc::new(Panic)),
    )
    .unwrap();
    assert!(detector
        .store
        .enqueue_from(&remote_file("event"), DetectorOrigin::Remote)
        .unwrap());
    assert_eq!(
        detector
            .status(DetectorStatusRequest {})
            .unwrap()
            .status
            .queue
            .pending,
        1
    );
    detector
        .stop(DetectorStopRequest {
            mode: DetectorStopMode::StopNow,
        })
        .unwrap();
    detector
        .stop(DetectorStopRequest {
            mode: DetectorStopMode::DrainInFlight,
        })
        .unwrap();
}

#[test]
fn status_does_not_wait_for_a_source_starting_on_another_thread() {
    use std::sync::atomic::AtomicBool;
    struct Slow {
        source: Arc<FakeSource>,
        entered: Arc<AtomicBool>,
        release: Arc<AtomicBool>,
    }
    impl Source for Slow {
        fn start(&self, mode: DetectorStartMode) -> DetectorResult<()> {
            *self.source.state.lock().unwrap() = DetectorState::Starting;
            self.entered.store(true, Ordering::SeqCst);
            while !self.release.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(2));
            }
            self.source.start(mode)
        }
        fn stop(&self, mode: DetectorStopMode) -> DetectorResult<()> {
            self.source.stop(mode)
        }
        fn status(&self) -> DetectorSourceStatus {
            self.source.status()
        }
        fn request_scan(&self, request: &DetectorScanRequest) -> DetectorResult<()> {
            self.source.request_scan(request)
        }
    }
    let tree = Tree::new();
    let entered = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let detector = Arc::new(
        Detector::compose(
            scope(&tree),
            store(&tree),
            Arc::new(Slow {
                source: FakeSource::new(DetectorOrigin::Local, false),
                entered: entered.clone(),
                release: release.clone(),
            }),
            FakeSource::new(DetectorOrigin::Remote, false),
            None,
        )
        .unwrap(),
    );
    let starting = detector.clone();
    let worker = std::thread::spawn(move || {
        starting.start(DetectorStartRequest {
            mode: DetectorStartMode::Initial,
            delivery: DeliveryState::Paused,
        })
    });
    while !entered.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(2));
    }
    let reading = detector.clone();
    let (send, receive) = std::sync::mpsc::channel();
    let status_worker =
        std::thread::spawn(move || send.send(reading.status(DetectorStatusRequest {})).unwrap());
    let snapshot = receive.recv_timeout(Duration::from_millis(250));
    release.store(true, Ordering::SeqCst);
    worker.join().unwrap().unwrap();
    status_worker.join().unwrap();
    assert_eq!(
        snapshot.unwrap().unwrap().status.state,
        DetectorState::Starting
    );
}
