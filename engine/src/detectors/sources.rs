//! Engine-owned source tasks. Only normalized observations enter the Context facade.
use super::{
    activity::{
        ActivityGuard, ActivityPublisher, ActivitySession, Details, Phase, Progress, ScanUpdate,
        Source, State, Unit, Wait,
    },
    observation::models::{CaptureEntries, ScopeCapture},
    observation::{
        local::LocalObserver,
        remote::{
            google::{cancelled, GoogleObserverProvider, GoogleObserverSession},
            interfaces::*,
            models::*,
        },
        scanner::{ScanError, Scanner},
        Observation, ObservationCoverage,
    },
    Detectors,
};
use crate::{
    accounts::{Accounts, AcquireSessionRequest},
    models::*,
    orchestrator::{OrchestratorConfig, OrchestratorHandle},
    providers::google::GoogleDriveClient,
    synchronization::{
        SynchronizationIssue, SynchronizationIssueKind, SynchronizationScanState,
        SynchronizationWait,
    },
};
use std::{
    future::{poll_fn, Future},
    sync::{Arc, Mutex},
    task::Poll,
    time::Duration,
};
use tokio::{sync::RwLock, task::JoinHandle};
/// Stopping cancels collection and queue admission before waiting for active Operations.
pub(crate) struct SourceTasks {
    cancel: CancellationToken,
    tasks: Mutex<Vec<JoinHandle<()>>>,
}
impl Drop for SourceTasks {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
impl SourceTasks {
    pub(crate) async fn stop(&self) {
        self.cancel.cancel();
        let tasks = self.tasks.lock().unwrap().drain(..).collect::<Vec<_>>();
        for task in tasks {
            let _ = task.await;
        }
    }
}
/// Starts separate read-only local and remote loops on the Engine executor.
pub(crate) fn start(
    config: OrchestratorConfig,
    detectors: Arc<Detectors>,
    orchestrator: Arc<OrchestratorHandle>,
    accounts: Accounts,
    client: GoogleDriveClient,
    ownership: Arc<RwLock<()>>,
) -> SourceTasks {
    let cancel = CancellationToken::new();
    let local_session = orchestrator.detector_session(Source::Local);
    let remote_session = orchestrator.detector_session(Source::Remote);
    let worker = Arc::new(SourceWorker {
        config,
        detectors,
        handle: orchestrator,
        cancel: cancel.clone(),
        local: local_session.publisher(),
        remote: remote_session.publisher(),
    });
    let local_worker = tokio::spawn(local(worker.clone()));
    let remote_worker = tokio::spawn(remote(worker.clone(), accounts, client, ownership));
    let local = supervise(local_worker, worker.clone(), Source::Local, local_session);
    let remote = supervise(remote_worker, worker, Source::Remote, remote_session);
    SourceTasks {
        cancel,
        tasks: Mutex::new(vec![local, remote]),
    }
}
struct SourceWorker {
    config: OrchestratorConfig,
    detectors: Arc<Detectors>,
    handle: Arc<OrchestratorHandle>,
    cancel: CancellationToken,
    local: ActivityPublisher,
    remote: ActivityPublisher,
}

impl SourceWorker {
    fn publisher(&self, source: Source) -> &ActivityPublisher {
        match source {
            Source::Local => &self.local,
            Source::Remote => &self.remote,
        }
    }

    fn begin(&self, source: Source, phase: Phase, state: State) -> ActivityGuard {
        self.publisher(source)
            .begin(source.origin(), None, phase, state, Details::default())
    }

    fn scan_state(&self, source: Source, state: SynchronizationScanState, items: Option<u64>) {
        self.publisher(source).scan(ScanUpdate::State(state, items));
    }

    async fn health(&self, source: Source, scanning: bool, problem: bool) {
        tokio::select! {
            biased;
            _ = cancelled(&self.cancel) => {},
            _ = async {
                self.scan_state(source, if scanning {
                    SynchronizationScanState::Reading
                } else if problem {
                    SynchronizationScanState::RetryScheduled
                } else {
                    SynchronizationScanState::Complete
                }, None);
                self.handle.source_status(source, scanning, problem.then(|| issue(source))).await
            } => {},
        }
    }

    async fn failed(&self, source: Source) {
        self.scan_state(source, SynchronizationScanState::Failed, None);
        report_failure(&self.handle, source, &self.cancel).await;
    }
}

fn supervise(
    task: JoinHandle<()>,
    worker: Arc<SourceWorker>,
    source: Source,
    session: ActivitySession,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let unexpected = task.await.is_err() && !worker.cancel.is_cancelled();
        if unexpected {
            worker.scan_state(source, SynchronizationScanState::Failed, None);
        }
        drop(session);
        if unexpected {
            report_failure(&worker.handle, source, &worker.cancel).await;
        }
    })
}

async fn report_failure(handle: &OrchestratorHandle, source: Source, cancel: &CancellationToken) {
    tokio::select! {biased;_=cancelled(cancel)=>{},_=handle.source_status(source,false,Some(SynchronizationIssue {kind:SynchronizationIssueKind::WorkerFailed,retryable:false}))=>{}}
}
fn issue(source: Source) -> SynchronizationIssue {
    SynchronizationIssue {
        kind: SynchronizationIssueKind::DeferredFor(match source {
            Source::Local => SynchronizationWait::LocalObservation,
            Source::Remote => SynchronizationWait::RemoteObservation,
        }),
        retryable: true,
    }
}
async fn deliver(
    worker: &SourceWorker,
    observations: Vec<Observation>,
    round: &ActivityGuard,
) -> bool {
    let SourceWorker {
        detectors,
        handle,
        cancel,
        ..
    } = worker;
    let total = observations.len();
    let delivery = round.child(
        Phase::Deliver,
        State::Running,
        Details::count(Progress::new(Unit::Observations, 0, Some(total))),
    );
    for (received, observation) in observations.into_iter().enumerate() {
        delivery.report(|| {
            Details::count(Progress::new(Unit::Observations, received + 1, Some(total)))
        });
        if cancel.is_cancelled() || handle.is_finished() {
            return false;
        }
        let capture = match &observation {
            Observation::Capture(capture) => {
                let source = match &capture.entries {
                    CaptureEntries::Local(_) => Source::Local,
                    CaptureEntries::Remote(_) => Source::Remote,
                };
                let complete_epoch = (capture.coverage == ObservationCoverage::Complete)
                    .then_some(capture.collect_epoch);
                Some((source, complete_epoch))
            }
            _ => None,
        };
        let files = match detectors.clone().contextualize_batch(observation).await {
            Ok(file) => file,
            Err(_) => {
                delivery.finish(State::Failed);
                worker.health(Source::Local, false, true).await;
                worker.health(Source::Remote, false, true).await;
                return true;
            }
        };
        if let Some((source, complete_epoch)) = capture {
            tokio::select! {
                biased;
                _ = cancelled(cancel) => return false,
                result = handle.source_capture(source, complete_epoch) => {
                    if result.is_err() {
                        return false;
                    }
                }
            }
        }
        if !deliver_files(handle, files, cancel, &delivery).await {
            return false;
        }
    }
    delivery.transition(State::Running, || {
        Details::count(Progress::new(Unit::Observations, total, Some(total)))
    });
    delivery.finish(State::Complete);
    true
}

async fn deliver_files(
    handle: &OrchestratorHandle,
    files: Vec<super::context::File>,
    cancel: &CancellationToken,
    delivery: &ActivityGuard,
) -> bool {
    let total = files.len();
    let progress = |sent| Details::count(Progress::new(Unit::Files, sent, Some(total)));
    let batch = delivery.child(Phase::Deliver, State::Running, progress(0));
    for (sent, file) in files.into_iter().enumerate() {
        let mut admission = std::pin::pin!(handle.submit(file));
        let mut waited = false;
        let submit = poll_fn(|cx| {
            let result = admission.as_mut().poll(cx);
            if result == Poll::Pending && !waited {
                waited = true;
                batch.transition(State::Waiting(Wait::Queue), || progress(sent));
            }
            result
        });
        tokio::select! {
            biased;
            _ = cancelled(cancel) => return false,
            result = submit => if result.is_err() {
                batch.finish(State::Failed);
                return false;
            }
        }
        if waited {
            batch.transition(State::Running, || progress(sent + 1));
        } else {
            batch.report(|| progress(sent + 1));
        }
    }
    batch.transition(State::Running, || progress(total));
    batch.finish(State::Complete);
    true
}
async fn sleep(duration: Duration, cancel: &CancellationToken) {
    tokio::select! {_=cancelled(cancel)=>{},_=tokio::time::sleep(duration)=>{}}
}
async fn local(worker: Arc<SourceWorker>) {
    let SourceWorker {
        config,
        handle,
        cancel,
        local,
        ..
    } = worker.as_ref();
    let mut observer: Option<Arc<Mutex<LocalObserver>>> = None;
    let mut initial = true;
    let mut generation = 0;
    while !cancel.is_cancelled() && !handle.is_finished() {
        let activity = worker.begin(Source::Local, Phase::Prepare, State::Running);
        let epoch = handle.collection_epoch();
        if observer.is_none() {
            let path = config.roots.local_root.clone();
            let root_id = config.sync_root_id.clone();
            let initialization = activity.child(
                Phase::Initialize,
                State::Waiting(Wait::Worker),
                Details::default(),
            );
            let scanner = tokio::task::spawn_blocking(move || {
                initialization.transition(State::Running, Details::default);
                let result = Scanner::open(path, root_id);
                initialization.finish(if result.is_ok() {
                    State::Complete
                } else {
                    State::Failed
                });
                result
            })
            .await;
            if let Ok(Ok(scanner)) = scanner {
                observer = Some(Arc::new(Mutex::new(LocalObserver::new(scanner))));
            }
        }
        let Some(current) = observer.clone() else {
            worker.health(Source::Local, false, true).await;
            generation += 1;
            if !deliver(
                &worker,
                vec![capture(
                    config,
                    generation,
                    epoch,
                    ObservationCoverage::Unavailable,
                    CaptureEntries::Local(Vec::new()),
                )],
                &activity,
            )
            .await
            {
                break;
            }
            let wait = activity.child(Phase::Idle, State::Waiting(Wait::Retry), Details::default());
            sleep(Duration::from_secs(2), cancel).await;
            wait.finish(if cancel.is_cancelled() {
                State::Cancelled
            } else {
                State::Complete
            });
            activity.finish(State::Failed);
            continue;
        };
        if initial {
            worker.health(Source::Local, true, true).await;
        }
        local.scan(ScanUpdate::Progress(super::observation::ScanProgress {
            path: Some(Path::new()),
            items: 0,
            pages: 0,
        }));
        let token = cancel.clone();
        let observing = activity.child(
            Phase::Observe,
            State::Waiting(Wait::Worker),
            Details::default(),
        );
        let round = tokio::task::spawn_blocking(move || {
            let mut observer = current.lock().unwrap();
            observing.transition(State::Running, Details::default);
            match observer.poll(&token, Some(&observing)) {
                Ok((_, coverage)) => {
                    let snapshot = observer.snapshot();
                    observing.transition(State::Running, || {
                        Details::count(Progress::new(
                            Unit::Observations,
                            snapshot.len(),
                            Some(snapshot.len()),
                        ))
                    });
                    observing.finish(State::Complete);
                    Ok((snapshot, coverage))
                }
                Err(error) => {
                    let unavailable = observer.unavailable();
                    observing.finish(if matches!(error, ScanError::Cancelled) {
                        State::Cancelled
                    } else {
                        State::Failed
                    });
                    Err((error, unavailable))
                }
            }
        })
        .await;
        generation += 1;
        match round {
            Ok(Ok((observations, coverage))) => {
                worker.scan_state(
                    Source::Local,
                    if coverage == ObservationCoverage::Complete {
                        SynchronizationScanState::Complete
                    } else {
                        SynchronizationScanState::RetryScheduled
                    },
                    Some(observations.len() as u64),
                );
                worker
                    .health(
                        Source::Local,
                        false,
                        coverage != ObservationCoverage::Complete,
                    )
                    .await;
                if !deliver(
                    &worker,
                    vec![capture(
                        config,
                        generation,
                        epoch,
                        coverage,
                        CaptureEntries::Local(observations),
                    )],
                    &activity,
                )
                .await
                {
                    break;
                }
                initial = false;
            }
            Ok(Err((ScanError::Cancelled, _))) => break,
            Ok(Err((_, _))) => {
                observer = None;
                initial = true;
                worker.health(Source::Local, false, true).await;
                if !deliver(
                    &worker,
                    vec![capture(
                        config,
                        generation,
                        epoch,
                        ObservationCoverage::Unavailable,
                        CaptureEntries::Local(Vec::new()),
                    )],
                    &activity,
                )
                .await
                {
                    break;
                }
                worker.health(Source::Local, false, true).await;
            }
            Err(_) => {
                worker.health(Source::Local, false, true).await;
                break;
            }
        }
        let event_fd = observer
            .as_ref()
            .and_then(|observer| observer.lock().ok()?.event_fd())
            .and_then(|fd| {
                // AsyncFd owns this duplicate and its raw descriptor stays valid until drop.
                unsafe {
                    tokio::io::unix::AsyncFd::register_with_interest(
                        fd,
                        tokio::io::Interest::READABLE,
                    )
                    .ok()
                }
            });
        let waiting = activity.child(
            Phase::Idle,
            State::Waiting(Wait::Refresh),
            Details::default(),
        );
        tokio::select! {
            _ = cancelled(cancel) => {},
            _ = handle.local_refresh() => {},
            _ = async {
                match &event_fd {
                    Some(fd) => { let _ = fd.readable().await; }
                    None => std::future::pending::<()>().await,
                }
            } => {},
            _ = tokio::time::sleep(Duration::from_secs(2)) => {},
        }
        waiting.finish(if cancel.is_cancelled() {
            State::Cancelled
        } else {
            State::Complete
        });
        activity.finish(if cancel.is_cancelled() {
            State::Cancelled
        } else {
            State::Complete
        });
    }
    if !cancel.is_cancelled() && !handle.is_finished() {
        worker.failed(Source::Local).await;
    }
}
async fn remote(
    worker: Arc<SourceWorker>,
    accounts: Accounts,
    client: GoogleDriveClient,
    ownership: Arc<RwLock<()>>,
) {
    let SourceWorker {
        config,
        detectors,
        handle,
        cancel,
        remote,
        ..
    } = worker.as_ref();
    let initialization = worker.begin(Source::Remote, Phase::Initialize, State::Running);
    let mut factory = ProviderRemoteObserverFactory::new();
    factory.register(
        ProviderId("google".into()),
        Arc::new(GoogleObserverProvider { client }),
    );
    let observer = match factory.create(RemoteObserverConfig {
        account_id: config.roots.account_id.clone(),
        provider_id: config.provider_id.clone(),
        sync_root_id: config.sync_root_id.clone(),
        remote_root_id: config.roots.remote_root_id.clone(),
    }) {
        Ok(observer) => observer,
        Err(_) => {
            initialization.finish(State::Failed);
            worker.health(Source::Remote, false, true).await;
            return;
        }
    };
    initialization.finish(State::Complete);
    let mut mode = RemoteObserveMode::Initial;
    let mut invalidated = false;
    let mut generation = 0;
    let mut collection_epoch = None;
    while !cancel.is_cancelled() && !handle.is_finished() {
        let activity = worker.begin(Source::Remote, Phase::Prepare, State::Running);
        if matches!(
            mode,
            RemoteObserveMode::Initial | RemoteObserveMode::Recovery
        ) {
            worker.health(Source::Remote, true, true).await;
        }
        // A paginated traversal may have started before a concurrent operation. Keep
        // the original epoch until every page has been collected or the round fails.
        let epoch = *collection_epoch.get_or_insert_with(|| handle.collection_epoch());
        worker.scan_state(Source::Remote, SynchronizationScanState::Reading, None);
        let progress_publisher = remote.clone();
        let progress = super::observation::ProgressReporter::new(move |sample| {
            progress_publisher.scan(ScanUpdate::Progress(sample.value));
        });
        let observing = activity.child(
            Phase::Observe,
            State::Waiting(Wait::Worker),
            Details::default(),
        );
        let round = async {
            let scoped = detectors.clone();
            let known_remote_ids = tokio::task::spawn_blocking(move || scoped.known_remote_ids())
                .await
                .map_err(|_| RemoteObserverError::Unavailable)?
                .map_err(|_| RemoteObserverError::Unavailable)?;
            observing.transition(State::Waiting(Wait::Ownership), Details::default);
            let _lease = tokio::select! {biased;_=cancelled(cancel)=>return Err(RemoteObserverError::Cancelled),lease=ownership.clone().read_owned()=>lease};
            observing.transition(State::Waiting(Wait::Authorization), Details::default);
            let session = accounts
                .acquire_session(AcquireSessionRequest {
                    account_id: config.roots.account_id.clone(),
                })
                .await
                .map_err(|_| RemoteObserverError::AuthorizationRequired)?;
            let session = GoogleObserverSession {
                account_id: config.roots.account_id.clone(),
                provider_id: config.provider_id.clone(),
                session: session.session.google().clone(),
            };
            observing.transition(State::Running, Details::default);
            observer
                .observe(
                    RemoteObserveRequest {
                        progress: Some(progress),
                        scope: RemoteScope {
                            account_id: config.roots.account_id.clone(),
                            provider_id: config.provider_id.clone(),
                            sync_root_id: config.sync_root_id.clone(),
                            remote_root_id: config.roots.remote_root_id.clone(),
                        },
                        mode,
                        known_remote_ids,
                        page_budget: Some(16),
                    },
                    &session,
                    cancel,
                )
                .await
        };
        // Session acquisition can commit refreshed credentials: retain the ownership lease
        // and await its real completion before allowing a disconnect to remove the account.
        let result = {
            let _sample = crate::performance::Sample::begin(match mode {
                RemoteObserveMode::Initial | RemoteObserveMode::Recovery => {
                    "remote_full_mode_round"
                }
                RemoteObserveMode::Incremental => "remote_incremental_mode_round",
            });
            round.await
        };
        if let Ok(result) = &result {
            let count = result.observations.len();
            observing.transition(State::Running, || {
                Details::count(Progress::new(Unit::Observations, count, Some(count)))
            });
        }
        observing.finish(match result {
            Ok(_) => State::Complete,
            Err(RemoteObserverError::Cancelled) => State::Cancelled,
            Err(_) => State::Failed,
        });
        generation += 1;
        let delay = match result {
            Ok(result) => {
                let complete = result.coverage == ObservationCoverage::Complete && !result.has_more;
                if complete {
                    worker.scan_state(
                        Source::Remote,
                        SynchronizationScanState::Complete,
                        Some(result.observations.len() as u64),
                    );
                }
                // A paginated round owns its partial data until it can publish a complete scope.
                worker
                    .health(Source::Remote, result.has_more, !complete)
                    .await;
                mode = RemoteObserveMode::Incremental;
                if complete {
                    collection_epoch = None;
                    if !deliver(
                        &worker,
                        vec![capture(
                            config,
                            generation,
                            epoch,
                            result.coverage,
                            CaptureEntries::Remote(result.observations),
                        )],
                        &activity,
                    )
                    .await
                    {
                        break;
                    }
                    invalidated = false;
                    if handle.collection_epoch() > epoch {
                        Duration::ZERO
                    } else {
                        Duration::from_secs(15)
                    }
                } else {
                    Duration::from_millis(50)
                }
            }
            Err(RemoteObserverError::Cancelled) => break,
            Err(_) => {
                collection_epoch = None;
                worker.health(Source::Remote, false, true).await;
                if !invalidated {
                    if !deliver(
                        &worker,
                        vec![capture(
                            config,
                            generation,
                            epoch,
                            ObservationCoverage::Unavailable,
                            CaptureEntries::Remote(Vec::new()),
                        )],
                        &activity,
                    )
                    .await
                    {
                        break;
                    }
                    invalidated = true;
                }
                worker.health(Source::Remote, false, true).await;
                mode = RemoteObserveMode::Recovery;
                Duration::from_secs(15)
            }
        };
        let reason = if mode == RemoteObserveMode::Recovery {
            Wait::Retry
        } else {
            Wait::Refresh
        };
        let waiting = activity.child(Phase::Idle, State::Waiting(reason), Details::default());
        tokio::select! {
            _ = cancelled(cancel) => {},
            _ = handle.remote_refresh() => {},
            _ = tokio::time::sleep(delay) => {},
        }
        waiting.finish(if cancel.is_cancelled() {
            State::Cancelled
        } else {
            State::Complete
        });
        activity.finish(if cancel.is_cancelled() {
            State::Cancelled
        } else {
            State::Complete
        });
    }
    if !cancel.is_cancelled() && !handle.is_finished() {
        worker.failed(Source::Remote).await;
    }
}

fn capture(
    config: &OrchestratorConfig,
    generation: u64,
    collect_epoch: u64,
    coverage: ObservationCoverage,
    entries: CaptureEntries,
) -> Observation {
    Observation::Capture(Box::new(ScopeCapture {
        account_id: config.roots.account_id.clone(),
        provider_id: config.provider_id.clone(),
        sync_root_id: config.sync_root_id.clone(),
        local_root: config.roots.local_root.clone(),
        remote_root_id: config.roots.remote_root_id.clone(),
        generation,
        collect_epoch,
        coverage,
        observed_at: super::observation::scanner::now(),
        entries,
    }))
}
