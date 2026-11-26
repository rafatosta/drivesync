//! Engine-owned source tasks. Only normalized observations enter the Context facade.
use super::{
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
    orchestrator::{OrchestratorConfig, OrchestratorHandle, Source},
    providers::google::GoogleDriveClient,
    synchronization::{
        SynchronizationIssue, SynchronizationIssueKind, SynchronizationScanState,
        SynchronizationWait,
    },
};
use std::{
    sync::{Arc, Mutex},
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
    let local_worker = tokio::spawn(local(
        config.clone(),
        detectors.clone(),
        orchestrator.clone(),
        cancel.clone(),
    ));
    let remote_worker = tokio::spawn(remote(
        config,
        detectors,
        orchestrator.clone(),
        accounts,
        client,
        ownership,
        cancel.clone(),
    ));
    let local = supervise(
        local_worker,
        orchestrator.clone(),
        Source::Local,
        cancel.clone(),
    );
    let remote = supervise(remote_worker, orchestrator, Source::Remote, cancel.clone());
    SourceTasks {
        cancel,
        tasks: Mutex::new(vec![local, remote]),
    }
}
fn supervise(
    worker: JoinHandle<()>,
    handle: Arc<OrchestratorHandle>,
    source: Source,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        if worker.await.is_err() && !cancel.is_cancelled() {
            tokio::select! {biased;_=cancelled(&cancel)=>{},_=handle.source_status(source,false,Some(SynchronizationIssue {kind:SynchronizationIssueKind::WorkerFailed,retryable:false}))=>{}}
        }
    })
}
async fn failed(handle: &OrchestratorHandle, source: Source, cancel: &CancellationToken) {
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
async fn health(
    handle: &OrchestratorHandle,
    source: Source,
    scanning: bool,
    problem: bool,
    cancel: &CancellationToken,
) {
    tokio::select! {biased;_=cancelled(cancel)=>{},_=handle.source_status(source,scanning,if problem{Some(issue(source))}else{None})=>{}}
}
async fn deliver(
    detectors: &Arc<Detectors>,
    handle: &OrchestratorHandle,
    observations: Vec<Observation>,
    cancel: &CancellationToken,
) -> bool {
    for observation in observations {
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
                health(handle, Source::Local, false, true, cancel).await;
                health(handle, Source::Remote, false, true, cancel).await;
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
        for file in files {
            tokio::select! {biased;_=cancelled(cancel)=>return false,result=handle.submit(file)=>if result.is_err(){return false;}}
        }
    }
    true
}
async fn sleep(duration: Duration, cancel: &CancellationToken) {
    tokio::select! {_=cancelled(cancel)=>{},_=tokio::time::sleep(duration)=>{}}
}
async fn local(
    config: OrchestratorConfig,
    detectors: Arc<Detectors>,
    handle: Arc<OrchestratorHandle>,
    cancel: CancellationToken,
) {
    let mut observer: Option<Arc<Mutex<LocalObserver>>> = None;
    let mut initial = true;
    let mut generation = 0;
    while !cancel.is_cancelled() && !handle.is_finished() {
        let epoch = handle.collection_epoch();
        if observer.is_none() {
            let path = config.roots.local_root.clone();
            let root_id = config.sync_root_id.clone();
            if let Ok(Ok(mut scanner)) =
                tokio::task::spawn_blocking(move || Scanner::open(path, root_id)).await
            {
                let reporting = handle.clone();
                scanner.set_progress(super::observation::ProgressReporter::new(move |sample| {
                    let scan = sample.value;
                    reporting.scan_progress(Source::Local, scan.path, scan.items, scan.pages);
                }));
                observer = Some(Arc::new(Mutex::new(LocalObserver::new(scanner))));
            }
        }
        let Some(current) = observer.clone() else {
            health(&handle, Source::Local, false, true, &cancel).await;
            generation += 1;
            if !deliver(
                &detectors,
                &handle,
                vec![capture(
                    &config,
                    generation,
                    epoch,
                    ObservationCoverage::Unavailable,
                    CaptureEntries::Local(Vec::new()),
                )],
                &cancel,
            )
            .await
            {
                break;
            }
            sleep(Duration::from_secs(2), &cancel).await;
            continue;
        };
        if initial {
            health(&handle, Source::Local, true, true, &cancel).await;
        }
        handle.scan_progress(Source::Local, Some(Path::new()), 0, 0);
        let token = cancel.clone();
        let round = tokio::task::spawn_blocking(move || {
            let mut observer = current.lock().unwrap();
            match observer.poll(&token) {
                Ok((_, coverage)) => Ok((observer.snapshot(), coverage)),
                Err(error) => Err((error, observer.unavailable())),
            }
        })
        .await;
        generation += 1;
        match round {
            Ok(Ok((observations, coverage))) => {
                handle.scan_state(
                    Source::Local,
                    if coverage == ObservationCoverage::Complete {
                        SynchronizationScanState::Complete
                    } else {
                        SynchronizationScanState::RetryScheduled
                    },
                    Some(observations.len() as u64),
                );
                health(
                    &handle,
                    Source::Local,
                    false,
                    coverage != ObservationCoverage::Complete,
                    &cancel,
                )
                .await;
                if !deliver(
                    &detectors,
                    &handle,
                    vec![capture(
                        &config,
                        generation,
                        epoch,
                        coverage,
                        CaptureEntries::Local(observations),
                    )],
                    &cancel,
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
                health(&handle, Source::Local, false, true, &cancel).await;
                if !deliver(
                    &detectors,
                    &handle,
                    vec![capture(
                        &config,
                        generation,
                        epoch,
                        ObservationCoverage::Unavailable,
                        CaptureEntries::Local(Vec::new()),
                    )],
                    &cancel,
                )
                .await
                {
                    break;
                }
                health(&handle, Source::Local, false, true, &cancel).await;
            }
            Err(_) => {
                health(&handle, Source::Local, false, true, &cancel).await;
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
        tokio::select! {
            _ = cancelled(&cancel) => {},
            _ = handle.local_refresh() => {},
            _ = async {
                match &event_fd {
                    Some(fd) => { let _ = fd.readable().await; }
                    None => std::future::pending::<()>().await,
                }
            } => {},
            _ = tokio::time::sleep(Duration::from_secs(2)) => {},
        }
    }
    if !cancel.is_cancelled() && !handle.is_finished() {
        failed(&handle, Source::Local, &cancel).await;
    }
}
async fn remote(
    config: OrchestratorConfig,
    detectors: Arc<Detectors>,
    handle: Arc<OrchestratorHandle>,
    accounts: Accounts,
    client: GoogleDriveClient,
    ownership: Arc<RwLock<()>>,
    cancel: CancellationToken,
) {
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
            health(&handle, Source::Remote, false, true, &cancel).await;
            return;
        }
    };
    let mut mode = RemoteObserveMode::Initial;
    let mut invalidated = false;
    let mut generation = 0;
    let mut collection_epoch = None;
    while !cancel.is_cancelled() && !handle.is_finished() {
        if matches!(
            mode,
            RemoteObserveMode::Initial | RemoteObserveMode::Recovery
        ) {
            health(&handle, Source::Remote, true, true, &cancel).await;
        }
        // A paginated traversal may have started before a concurrent operation. Keep
        // the original epoch until every page has been collected or the round fails.
        let epoch = *collection_epoch.get_or_insert_with(|| handle.collection_epoch());
        handle.scan_state(Source::Remote, SynchronizationScanState::Reading, None);
        let progress_handle = handle.clone();
        let progress = super::observation::ProgressReporter::new(move |sample| {
            let scan = sample.value;
            progress_handle.scan_progress(Source::Remote, scan.path, scan.items, scan.pages);
        });
        let round = async {
            let scoped = detectors.clone();
            let known_remote_ids = tokio::task::spawn_blocking(move || scoped.known_remote_ids())
                .await
                .map_err(|_| RemoteObserverError::Unavailable)?
                .map_err(|_| RemoteObserverError::Unavailable)?;
            let _lease = tokio::select! {biased;_=cancelled(&cancel)=>return Err(RemoteObserverError::Cancelled),lease=ownership.clone().read_owned()=>lease};
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
                    &cancel,
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
        generation += 1;
        let delay = match result {
            Ok(result) => {
                let complete = result.coverage == ObservationCoverage::Complete && !result.has_more;
                if complete {
                    handle.scan_state(
                        Source::Remote,
                        SynchronizationScanState::Complete,
                        Some(result.observations.len() as u64),
                    );
                }
                // A paginated round owns its partial data until it can publish a complete scope.
                health(&handle, Source::Remote, result.has_more, !complete, &cancel).await;
                mode = RemoteObserveMode::Incremental;
                if complete {
                    collection_epoch = None;
                    if !deliver(
                        &detectors,
                        &handle,
                        vec![capture(
                            &config,
                            generation,
                            epoch,
                            result.coverage,
                            CaptureEntries::Remote(result.observations),
                        )],
                        &cancel,
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
                health(&handle, Source::Remote, false, true, &cancel).await;
                if !invalidated {
                    if !deliver(
                        &detectors,
                        &handle,
                        vec![capture(
                            &config,
                            generation,
                            epoch,
                            ObservationCoverage::Unavailable,
                            CaptureEntries::Remote(Vec::new()),
                        )],
                        &cancel,
                    )
                    .await
                    {
                        break;
                    }
                    invalidated = true;
                }
                health(&handle, Source::Remote, false, true, &cancel).await;
                mode = RemoteObserveMode::Recovery;
                Duration::from_secs(15)
            }
        };
        tokio::select! {
            _ = cancelled(&cancel) => {},
            _ = handle.remote_refresh() => {},
            _ = tokio::time::sleep(delay) => {},
        }
    }
    if !cancel.is_cancelled() && !handle.is_finished() {
        failed(&handle, Source::Remote, &cancel).await;
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
