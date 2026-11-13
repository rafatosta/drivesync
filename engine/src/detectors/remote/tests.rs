//! Real SQLite and real scheduler checks with an isolated Provider port.
use super::*;
use crate::{
    detectors::remote::google::{Request, Response},
    models::*,
    providers::google::*,
};
use std::sync::atomic::{AtomicUsize, Ordering};
struct ApiFixture {
    calls: AtomicUsize,
    slow: bool,
}
impl Api for ApiFixture {
    fn call(&self, request: Request, cancel: &Cancellation) -> RemoteResult<Response> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.slow {
            loop {
                cancel.check()?;
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        Ok(match request {
            Request::Start => Response::Start("start".into()),
            Request::Get(id) => Response::File(Box::new(serde_json::from_value(serde_json::json!({"id":id,"name":"Root","mimeType":"application/vnd.google-apps.folder","trashed":false})).unwrap())),
            Request::List { .. } => Response::List(GoogleFileList::default()),
            Request::Changes(_) => Response::Changes(GoogleChangeList { new_start_page_token: Some("end".into()), ..Default::default() }),
        })
    }
}
fn base() -> std::path::PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "drivesync-remote-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir(&path).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path
}
fn scope(account: &str, root: &str) -> RemoteDetectorScope {
    RemoteDetectorScope {
        account_id: AccountId(account.into()),
        sync_id: SyncId(format!("sync-{account}")),
        remote_root_id: root.into(),
    }
}
fn settings() -> RemoteDetectorSettings {
    RemoteDetectorSettings {
        polling_interval: Duration::from_millis(20),
        page_budget: 3,
    }
}
fn wait_until(test: impl Fn() -> bool) {
    let until = Instant::now() + Duration::from_secs(3);
    while !test() {
        assert!(Instant::now() < until, "worker deadline exceeded");
        std::thread::sleep(Duration::from_millis(5));
    }
}
#[test]
fn stopped_constructor_and_status_never_request_network() {
    let base = base();
    let api = Arc::new(ApiFixture {
        calls: AtomicUsize::new(0),
        slow: false,
    });
    let detector =
        RemoteDetector::open(scope("a", "root-a"), settings(), base.clone(), api.clone()).unwrap();
    assert_eq!(
        detector.status(RemoteStatusRequest {}).unwrap().state,
        RemoteState::Stopped
    );
    assert!(detector.diagnostics().status.last_poll_started_at.is_none());
    assert_eq!(api.calls.load(Ordering::SeqCst), 0);
    drop(detector);
    std::fs::remove_dir_all(base).unwrap();
}
#[test]
fn scheduler_polls_while_delivery_is_paused_and_stop_cancels_wait() {
    let base = base();
    let api = Arc::new(ApiFixture {
        calls: AtomicUsize::new(0),
        slow: false,
    });
    let detector =
        RemoteDetector::open(scope("a", "root-a"), settings(), base.clone(), api.clone()).unwrap();
    detector
        .start(RemoteStartRequest {
            mode: RemotePollMode::Initial,
        })
        .unwrap();
    assert_eq!(
        detector
            .start(RemoteStartRequest {
                mode: RemotePollMode::Initial
            })
            .unwrap_err()
            .kind,
        RemoteError::InvalidState
    );
    wait_until(|| api.calls.load(Ordering::SeqCst) >= 6);
    assert_eq!(detector.diagnostics().queue.delivery, DeliveryState::Paused);
    assert_eq!(detector.diagnostics().queue.detected, 1);
    assert!(detector.diagnostics().status.next_poll_at.is_some());
    assert_eq!(
        detector
            .take_events(TakeEventsRequest {
                max_events: 10,
                lease_duration: Duration::from_secs(30)
            })
            .unwrap_err()
            .kind,
        QueueError::DeliveryPaused
    );
    detector.stop(RemoteStopRequest {}).unwrap();
    let stopped_calls = api.calls.load(Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(40));
    assert_eq!(api.calls.load(Ordering::SeqCst), stopped_calls);
    assert_eq!(detector.diagnostics().status.next_poll_at, None);
    detector.stop(RemoteStopRequest {}).unwrap();
    drop(detector);
    std::fs::remove_dir_all(base).unwrap();
}
#[test]
fn in_flight_request_is_cancelled_and_drop_joins_it() {
    let base = base();
    let api = Arc::new(ApiFixture {
        calls: AtomicUsize::new(0),
        slow: true,
    });
    let detector =
        RemoteDetector::open(scope("a", "root-a"), settings(), base.clone(), api.clone()).unwrap();
    detector
        .start(RemoteStartRequest {
            mode: RemotePollMode::Initial,
        })
        .unwrap();
    wait_until(|| api.calls.load(Ordering::SeqCst) > 0);
    let started = Instant::now();
    drop(detector);
    assert!(started.elapsed() < Duration::from_secs(1));
    std::fs::remove_dir_all(base).unwrap();
}
#[test]
fn real_storage_reopens_cursor_and_complete_payload_without_duplicate_initial_events() {
    let base = base();
    let api = Arc::new(ApiFixture {
        calls: AtomicUsize::new(0),
        slow: false,
    });
    let detector =
        RemoteDetector::open(scope("a", "root-a"), settings(), base.clone(), api.clone()).unwrap();
    detector
        .poll_once(RemotePollRequest {
            mode: RemotePollMode::Initial,
            page_budget: None,
        })
        .unwrap();
    let original = detector.shared.store.recent().unwrap()[0].clone();
    assert!(matches!(original.data, Some(FileData::Remote(_))));
    drop(detector);
    let reopened =
        RemoteDetector::open(scope("a", "root-a"), settings(), base.clone(), api.clone()).unwrap();
    assert_eq!(reopened.shared.store.recent().unwrap()[0], original);
    reopened
        .poll_once(RemotePollRequest {
            mode: RemotePollMode::Incremental,
            page_budget: None,
        })
        .unwrap();
    assert_eq!(reopened.diagnostics().queue.detected, 1);
    reopened.resume().unwrap();
    let batch = reopened
        .take_events(TakeEventsRequest {
            max_events: 10,
            lease_duration: Duration::from_secs(30),
        })
        .unwrap();
    assert_eq!(batch.files, vec![original]);
    reopened
        .release_events(batch.batch_id.as_ref().unwrap())
        .unwrap();
    let batch = reopened
        .take_events(TakeEventsRequest {
            max_events: 10,
            lease_duration: Duration::from_secs(30),
        })
        .unwrap();
    reopened
        .acknowledge(
            batch.batch_id.as_ref().unwrap(),
            EventDisposition::Processed,
        )
        .unwrap();
    assert_eq!(reopened.diagnostics().queue.confirmed, 1);
    assert_eq!(api.calls.load(Ordering::SeqCst), 5);
    drop(reopened);
    std::fs::remove_dir_all(base).unwrap();
}
#[test]
fn accounts_and_remote_roots_do_not_share_progress_or_queues() {
    let base = base();
    let api = Arc::new(ApiFixture {
        calls: AtomicUsize::new(0),
        slow: false,
    });
    let first =
        RemoteDetector::open(scope("a", "root"), settings(), base.clone(), api.clone()).unwrap();
    let second =
        RemoteDetector::open(scope("b", "root"), settings(), base.clone(), api.clone()).unwrap();
    let replaced = RemoteDetector::open(
        scope("a", "another-root"),
        settings(),
        base.clone(),
        api.clone(),
    )
    .unwrap();
    first
        .poll_once(RemotePollRequest {
            mode: RemotePollMode::Initial,
            page_budget: None,
        })
        .unwrap();
    assert_eq!(first.diagnostics().queue.detected, 1);
    assert_eq!(second.diagnostics().queue.detected, 0);
    assert_eq!(replaced.diagnostics().queue.detected, 0);
    drop((first, second, replaced));
    std::fs::remove_dir_all(base).unwrap();
}
#[test]
fn invalid_scope_or_budget_is_rejected_before_side_effects() {
    let base = base();
    let api = Arc::new(ApiFixture {
        calls: AtomicUsize::new(0),
        slow: false,
    });
    assert!(
        RemoteDetector::open(scope("", "root"), settings(), base.clone(), api.clone()).is_err()
    );
    let detector =
        RemoteDetector::open(scope("a", "root"), settings(), base.clone(), api.clone()).unwrap();
    assert_eq!(
        detector
            .poll_once(RemotePollRequest {
                mode: RemotePollMode::Initial,
                page_budget: Some(0)
            })
            .unwrap_err()
            .kind,
        RemoteError::InvalidScope
    );
    assert_eq!(api.calls.load(Ordering::SeqCst), 0);
    drop(detector);
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn corrupted_progress_is_rejected_without_network_or_silent_recreation() {
    let base = base();
    let api = Arc::new(ApiFixture {
        calls: AtomicUsize::new(0),
        slow: false,
    });
    let detector =
        RemoteDetector::open(scope("a", "root"), settings(), base.clone(), api.clone()).unwrap();
    detector
        .shared
        .store
        .save_remote_state(None, b"invalid checkpoint")
        .unwrap();
    drop(detector);
    assert!(
        RemoteDetector::open(scope("a", "root"), settings(), base.clone(), api.clone()).is_err()
    );
    assert_eq!(api.calls.load(Ordering::SeqCst), 0);
    std::fs::remove_dir_all(base).unwrap();
}
#[test]
fn a_remote_queue_refuses_another_accounts_payload_even_with_the_same_sync_id() {
    let base = base();
    let api = Arc::new(ApiFixture {
        calls: AtomicUsize::new(0),
        slow: false,
    });
    let detector =
        RemoteDetector::open(scope("a", "root"), settings(), base.clone(), api.clone()).unwrap();
    detector
        .poll_once(RemotePollRequest {
            mode: RemotePollMode::Initial,
            page_budget: None,
        })
        .unwrap();
    let mut file = detector.shared.store.recent().unwrap()[0].clone();
    let Some(FileData::Remote(remote)) = &mut file.data else {
        panic!()
    };
    remote.reference.account_id = AccountId("another-account".into());
    assert_eq!(
        detector.shared.store.enqueue(&file).unwrap_err().kind,
        QueueError::IdentityMismatch
    );
    drop(detector);
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn stop_also_cancels_an_explicit_poll_round() {
    let base = base();
    let api = Arc::new(ApiFixture {
        calls: AtomicUsize::new(0),
        slow: true,
    });
    let detector = Arc::new(
        RemoteDetector::open(scope("a", "root"), settings(), base.clone(), api.clone()).unwrap(),
    );
    let polling = detector.clone();
    let worker = std::thread::spawn(move || {
        polling.poll_once(RemotePollRequest {
            mode: RemotePollMode::Initial,
            page_budget: None,
        })
    });
    wait_until(|| api.calls.load(Ordering::SeqCst) > 0);
    let started = Instant::now();
    detector.stop(RemoteStopRequest {}).unwrap();
    assert_eq!(
        worker.join().unwrap().unwrap_err().kind,
        RemoteError::Cancelled
    );
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(detector.diagnostics().status.state, RemoteState::Stopped);
    drop(detector);
    std::fs::remove_dir_all(base).unwrap();
}
#[test]
fn provider_retry_after_is_owned_by_the_scheduler_and_wait_is_interruptible() {
    struct Limited;
    impl Api for Limited {
        fn call(&self, _: Request, _: &Cancellation) -> RemoteResult<Response> {
            Err(RemoteFailure {
                kind: RemoteError::RemoteUnavailable,
                retryable: true,
                retry_after: Some(Duration::from_secs(600)),
            })
        }
    }
    let base = base();
    let detector = RemoteDetector::open(
        scope("a", "root"),
        settings(),
        base.clone(),
        Arc::new(Limited),
    )
    .unwrap();
    detector
        .start(RemoteStartRequest {
            mode: RemotePollMode::Initial,
        })
        .unwrap();
    wait_until(|| {
        detector.diagnostics().status.next_poll_at.is_some()
            && detector.diagnostics().status.consecutive_failures > 0
    });
    let status = detector.diagnostics().status;
    assert_eq!(status.state, RemoteState::Degraded);
    assert_eq!(status.last_error, Some(RemoteError::RemoteUnavailable));
    assert!(status.next_poll_at.unwrap().unix_millis - now().unix_millis > 590000);
    let start = Instant::now();
    detector.stop(RemoteStopRequest {}).unwrap();
    assert!(start.elapsed() < Duration::from_secs(1));
    drop(detector);
    std::fs::remove_dir_all(base).unwrap();
}
