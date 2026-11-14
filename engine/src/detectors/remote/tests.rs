//! Real SQLite and real scheduler checks with an isolated Provider port.
use super::*;
use crate::{
    detectors::remote::google::{Api, GoogleRemoteDetector, Request, Response},
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
fn open_google(
    scope: RemoteDetectorScope,
    settings: RemoteDetectorSettings,
    directory: std::path::PathBuf,
    api: Arc<dyn Api>,
) -> RemoteResult<RemoteDetector> {
    let source_scope = scope.clone();
    RemoteDetector::open(
        scope,
        settings,
        directory,
        ProviderId("google".into()),
        move |backend| {
            Ok(Box::new(GoogleRemoteDetector {
                scope: source_scope,
                api,
                backend,
            }))
        },
    )
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
        open_google(scope("a", "root-a"), settings(), base.clone(), api.clone()).unwrap();
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
        open_google(scope("a", "root-a"), settings(), base.clone(), api.clone()).unwrap();
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
    // API call entry does not imply that the round has published its next schedule.
    wait_until(|| detector.diagnostics().status.next_poll_at.is_some());
    assert_eq!(
        detector
            .shared
            .store
            .claim(TakeEventsRequest {
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
        open_google(scope("a", "root-a"), settings(), base.clone(), api.clone()).unwrap();
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
        open_google(scope("a", "root-a"), settings(), base.clone(), api.clone()).unwrap();
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
        open_google(scope("a", "root-a"), settings(), base.clone(), api.clone()).unwrap();
    assert_eq!(reopened.shared.store.recent().unwrap()[0], original);
    reopened
        .poll_once(RemotePollRequest {
            mode: RemotePollMode::Incremental,
            page_budget: None,
        })
        .unwrap();
    assert_eq!(reopened.diagnostics().queue.detected, 1);
    reopened
        .shared
        .store
        .set_delivery(DeliveryState::Enabled)
        .unwrap();
    let batch = reopened
        .shared
        .store
        .claim(TakeEventsRequest {
            max_events: 10,
            lease_duration: Duration::from_secs(30),
        })
        .unwrap();
    assert_eq!(batch.files, vec![original]);
    reopened
        .shared
        .store
        .release(batch.batch_id.as_ref().unwrap())
        .unwrap();
    let batch = reopened
        .shared
        .store
        .claim(TakeEventsRequest {
            max_events: 10,
            lease_duration: Duration::from_secs(30),
        })
        .unwrap();
    reopened
        .shared
        .store
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
    let first = open_google(scope("a", "root"), settings(), base.clone(), api.clone()).unwrap();
    let second = open_google(scope("b", "root"), settings(), base.clone(), api.clone()).unwrap();
    let replaced = open_google(
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
    assert!(open_google(scope("", "root"), settings(), base.clone(), api.clone()).is_err());
    let detector = open_google(scope("a", "root"), settings(), base.clone(), api.clone()).unwrap();
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
    let detector = open_google(scope("a", "root"), settings(), base.clone(), api.clone()).unwrap();
    detector
        .shared
        .store
        .save_remote_page(None, b"invalid checkpoint", &RemoteScanUpdate::default())
        .unwrap();
    drop(detector);
    assert!(open_google(scope("a", "root"), settings(), base.clone(), api.clone()).is_err());
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
    let detector = open_google(scope("a", "root"), settings(), base.clone(), api.clone()).unwrap();
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
    let detector =
        Arc::new(open_google(scope("a", "root"), settings(), base.clone(), api.clone()).unwrap());
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
    let detector = open_google(
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

#[test]
fn scheduler_and_queue_accept_a_provider_independent_observer() {
    struct Independent {
        backend: Arc<dyn RemoteBackend>,
        calls: Arc<AtomicUsize>,
    }
    impl RemoteObserver for Independent {
        fn validate_progress(&self) -> RemoteResult<()> {
            self.backend.load().map(|_| ())
        }
        fn poll_once(
            &self,
            request: RemotePollRequest,
            cancel: &Cancellation,
            progress: &dyn Fn(u64, u64),
        ) -> RemoteResult<RemotePollResult> {
            cancel.check()?;
            assert!(request.page_budget.is_some_and(|budget| budget > 0));
            self.calls.fetch_add(1, Ordering::SeqCst);
            let emitted = u64::from(self.backend.emit(&File {
                id: crate::models::FileId("independent-occurrence".into()),
                sync_id: SyncId("sync-a".into()),
                event: FileEvent {
                    local: None,
                    remote: None,
                    kind: FileEventKind::RecheckRequired,
                    uncertainty: Some(ObservationUncertainty::CaptureStarted),
                    previous: None,
                    current: None,
                },
                observed_at: Timestamp { unix_millis: 1 },
                coverage: ObservationCoverage::Partial,
                scope: FileScope::WholeSync,
                data: None,
            })?);
            progress(1, emitted);
            Ok(RemotePollResult {
                coverage: RemoteCoverage::Complete,
                observed: 1,
                emitted,
                has_more: false,
                checkpoint_confirmed: Some("independent-checkpoint".into()),
            })
        }
    }
    let base = base();
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let detector = RemoteDetector::open(
        scope("a", "root"),
        settings(),
        base.clone(),
        ProviderId("independent".into()),
        move |backend| {
            Ok(Box::new(Independent {
                backend,
                calls: observed,
            }))
        },
    )
    .unwrap();
    detector
        .start(RemoteStartRequest {
            mode: RemotePollMode::Incremental,
        })
        .unwrap();
    wait_until(|| calls.load(Ordering::SeqCst) >= 2);
    detector.stop(RemoteStopRequest {}).unwrap();
    assert_eq!(detector.diagnostics().queue.detected, 1);
    assert_eq!(
        detector.diagnostics().status.cursor.as_deref(),
        Some("independent-checkpoint")
    );
    let google = open_google(
        scope("a", "root"),
        settings(),
        base.clone(),
        Arc::new(ApiFixture {
            calls: AtomicUsize::new(0),
            slow: false,
        }),
    )
    .unwrap();
    assert_eq!(
        google.diagnostics().queue.detected,
        0,
        "provider identity isolates otherwise identical scopes"
    );
    drop((detector, google));
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn google_provider_identity_rejects_foreign_payloads() {
    let base = base();
    let api = Arc::new(ApiFixture {
        calls: AtomicUsize::new(0),
        slow: false,
    });
    let detector = open_google(scope("a", "root"), settings(), base.clone(), api.clone()).unwrap();
    detector
        .poll_once(RemotePollRequest {
            mode: RemotePollMode::Initial,
            page_budget: None,
        })
        .unwrap();
    let mut foreign = detector.shared.store.recent().unwrap()[0].clone();
    let Some(FileData::Remote(remote)) = &mut foreign.data else {
        panic!()
    };
    remote.provider = ProviderId("another-provider".into());
    assert_eq!(
        detector.shared.store.enqueue(&foreign).unwrap_err().kind,
        QueueError::IdentityMismatch
    );
    drop(detector);
    let reopened = open_google(scope("a", "root"), settings(), base.clone(), api).unwrap();
    assert_eq!(reopened.diagnostics().queue.detected, 1);
    drop(reopened);
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn drain_finishes_the_current_round_without_cancelling_or_starting_another() {
    use std::sync::atomic::AtomicBool;
    struct Blocking {
        entered: Arc<AtomicBool>,
        release: Arc<AtomicBool>,
        calls: Arc<AtomicUsize>,
    }
    impl Api for Blocking {
        fn call(&self, _: Request, cancel: &Cancellation) -> RemoteResult<Response> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.entered.store(true, Ordering::SeqCst);
            while !self.release.load(Ordering::SeqCst) {
                cancel.check()?;
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(RemoteFailure::new(RemoteError::RemoteUnavailable))
        }
    }
    let base = base();
    let entered = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let calls = Arc::new(AtomicUsize::new(0));
    let detector = Arc::new(
        open_google(
            scope("a", "root"),
            settings(),
            base.clone(),
            Arc::new(Blocking {
                entered: entered.clone(),
                release: release.clone(),
                calls: calls.clone(),
            }),
        )
        .unwrap(),
    );
    detector
        .start(RemoteStartRequest {
            mode: RemotePollMode::Initial,
        })
        .unwrap();
    wait_until(|| entered.load(Ordering::SeqCst));
    let draining = detector.clone();
    let (done, received) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let result = draining.stop_mode(true);
        done.send(result).unwrap();
    });
    assert!(received.recv_timeout(Duration::from_millis(30)).is_err());
    release.store(true, Ordering::SeqCst);
    received
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap();
    worker.join().unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    drop(detector);
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn sqlite_restart_preserves_observations_after_failures_before_and_after_cursor_commit() {
    struct FailCommit {
        backend: Arc<dyn RemoteBackend>,
        saves: AtomicUsize,
        after_commit: bool,
    }
    impl RemoteBackend for FailCommit {
        fn load(&self) -> RemoteResult<Option<Vec<u8>>> {
            self.backend.load()
        }
        fn save(
            &self,
            previous: Option<&[u8]>,
            value: &[u8],
            traversal: &RemoteScanUpdate,
        ) -> RemoteResult<()> {
            if self.saves.fetch_add(1, Ordering::SeqCst) == 1 {
                if self.after_commit {
                    self.backend.save(previous, value, traversal)?;
                }
                return Err(RemoteFailure::new(RemoteError::CheckpointUnavailable));
            }
            self.backend.save(previous, value, traversal)
        }
        fn next_scan(&self) -> RemoteResult<Option<RemoteScan>> {
            self.backend.next_scan()
        }
        fn emit(&self, file: &File) -> RemoteResult<bool> {
            self.backend.emit(file)
        }
    }
    for after_commit in [false, true] {
        let base = base();
        let api = Arc::new(ApiFixture {
            calls: AtomicUsize::new(0),
            slow: false,
        });
        let source_scope = scope("a", "root");
        let source_api = api.clone();
        let detector = RemoteDetector::open(
            source_scope.clone(),
            settings(),
            base.clone(),
            ProviderId("google".into()),
            move |backend| {
                Ok(Box::new(GoogleRemoteDetector {
                    scope: source_scope,
                    api: source_api,
                    backend: Arc::new(FailCommit {
                        backend,
                        saves: AtomicUsize::new(0),
                        after_commit,
                    }),
                }))
            },
        )
        .unwrap();
        assert_eq!(
            detector
                .poll_once(RemotePollRequest {
                    mode: RemotePollMode::Initial,
                    page_budget: None,
                })
                .unwrap_err()
                .kind,
            RemoteError::CheckpointUnavailable
        );
        let original = detector.shared.store.recent().unwrap()[0].clone();
        assert_eq!(original.event.kind, FileEventKind::Observed);
        assert_eq!(
            original.event.remote.as_ref().unwrap().source,
            RemoteObservationSource::Capture
        );
        assert_eq!(
            detector.shared.store.next_remote_scan().unwrap().is_some(),
            after_commit
        );
        drop(detector);
        let reopened =
            open_google(scope("a", "root"), settings(), base.clone(), api.clone()).unwrap();
        reopened
            .poll_once(RemotePollRequest {
                mode: RemotePollMode::Incremental,
                page_budget: None,
            })
            .unwrap();
        assert_eq!(reopened.shared.store.recent().unwrap(), vec![original]);
        assert_eq!(reopened.diagnostics().queue.detected, 1);
        assert_eq!(
            api.calls.load(Ordering::SeqCst),
            4,
            "replay must not query the root again"
        );
        drop(reopened);
        std::fs::remove_dir_all(base).unwrap();
    }
}

#[test]
fn sqlite_preserves_removal_signal_and_resource_identity_through_delivery_and_reopen() {
    struct RemovalApi;
    impl Api for RemovalApi {
        fn call(&self, request: Request, cancel: &Cancellation) -> RemoteResult<Response> {
            if matches!(request, Request::Changes(_)) {
                return Ok(Response::Changes(serde_json::from_value(serde_json::json!({
                    "changes": [{"fileId":"removed-file", "removed":true, "time":"2026-11-14T12:00:00Z", "changeType":"file"}],
                    "newStartPageToken":"end"
                })).unwrap()));
            }
            ApiFixture {
                calls: AtomicUsize::new(0),
                slow: false,
            }
            .call(request, cancel)
        }
    }
    let base = base();
    let detector = open_google(
        scope("a", "root"),
        settings(),
        base.clone(),
        Arc::new(RemovalApi),
    )
    .unwrap();
    detector
        .poll_once(RemotePollRequest {
            mode: RemotePollMode::Initial,
            page_budget: None,
        })
        .unwrap();
    let original = detector.shared.store.recent().unwrap();
    let removed = original.iter().find(|file| file.data.is_none()).unwrap();
    assert_eq!(removed.event.kind, FileEventKind::Observed);
    assert_eq!(
        removed.scope,
        FileScope::Remote(RemoteFileRef {
            account_id: AccountId("a".into()),
            file_id: RemoteFileId("removed-file".into()),
        })
    );
    assert_eq!(removed.event.remote.as_ref().unwrap().removed, Some(true));
    assert_eq!(
        removed.event.remote.as_ref().unwrap().time.as_deref(),
        Some("2026-11-14T12:00:00Z")
    );
    drop(detector);
    let reopened = open_google(
        scope("a", "root"),
        settings(),
        base.clone(),
        Arc::new(RemovalApi),
    )
    .unwrap();
    assert_eq!(reopened.shared.store.recent().unwrap(), original);
    reopened
        .shared
        .store
        .set_delivery(DeliveryState::Enabled)
        .unwrap();
    let batch = reopened
        .shared
        .store
        .claim(TakeEventsRequest {
            max_events: 10,
            lease_duration: Duration::from_secs(30),
        })
        .unwrap();
    assert_eq!(batch.files.len(), 2);
    for file in &batch.files {
        assert!(original.contains(file));
    }
    reopened
        .shared
        .store
        .acknowledge(
            batch.batch_id.as_ref().unwrap(),
            EventDisposition::Processed,
        )
        .unwrap();
    assert_eq!(
        reopened.shared.store.recent().unwrap(),
        original,
        "ack must retain history"
    );
    assert_eq!(reopened.diagnostics().queue.confirmed, 2);
    drop(reopened);
    std::fs::remove_dir_all(base).unwrap();
}
