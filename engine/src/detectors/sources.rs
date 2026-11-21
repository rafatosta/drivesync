//! Engine-owned source tasks. Only normalized observations enter the Context facade.
use super::{
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
    synchronization::{SynchronizationIssue, SynchronizationIssueKind},
};
use std::{
    collections::BTreeMap,
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
fn issue() -> SynchronizationIssue {
    SynchronizationIssue {
        kind: SynchronizationIssueKind::Deferred,
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
    tokio::select! {biased;_=cancelled(cancel)=>{},_=handle.source_status(source,scanning,if problem{Some(issue())}else{None})=>{}}
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
        let file = match detectors.clone().contextualize_async(observation).await {
            Ok(file) => file,
            Err(_) => return false,
        };
        tokio::select! {biased;_=cancelled(cancel)=>return false,result=handle.submit(file)=>if result.is_err(){return false;}}
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
    while !cancel.is_cancelled() && !handle.is_finished() {
        if observer.is_none() {
            let path = config.roots.local_root.clone();
            let root_id = config.sync_root_id.clone();
            if let Ok(Ok(scanner)) =
                tokio::task::spawn_blocking(move || Scanner::open(path, root_id)).await
            {
                observer = Some(Arc::new(Mutex::new(LocalObserver::new(scanner))));
            }
        }
        let Some(current) = observer.clone() else {
            health(&handle, Source::Local, false, true, &cancel).await;
            sleep(Duration::from_secs(2), &cancel).await;
            continue;
        };
        if initial {
            health(&handle, Source::Local, true, true, &cancel).await;
        }
        let token = cancel.clone();
        let round = tokio::task::spawn_blocking(move || {
            let mut observer = current.lock().unwrap();
            match observer.poll(&token) {
                Ok(round) => Ok(round),
                Err(error) => Err((error, observer.unavailable())),
            }
        })
        .await;
        match round {
            Ok(Ok((observations, coverage))) => {
                health(
                    &handle,
                    Source::Local,
                    false,
                    coverage != ObservationCoverage::Complete,
                    &cancel,
                )
                .await;
                if !deliver(&detectors, &handle, observations, &cancel).await {
                    break;
                }
                initial = false;
            }
            Ok(Err((ScanError::Cancelled, _))) => break,
            Ok(Err((_, observations))) => {
                health(&handle, Source::Local, false, true, &cancel).await;
                if !deliver(&detectors, &handle, observations, &cancel).await {
                    break;
                }
                health(&handle, Source::Local, false, true, &cancel).await;
            }
            Err(_) => {
                health(&handle, Source::Local, false, true, &cancel).await;
                break;
            }
        }
        sleep(Duration::from_secs(2), &cancel).await;
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
    health(&handle, Source::Remote, false, true, &cancel).await;
    let mut mode = RemoteObserveMode::Initial;
    let mut known = BTreeMap::<RemoteId, super::observation::RemoteObservation>::new();
    let mut invalidated = false;
    while !cancel.is_cancelled() && !handle.is_finished() {
        let round = async {
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
                        scope: RemoteScope {
                            account_id: config.roots.account_id.clone(),
                            provider_id: config.provider_id.clone(),
                            sync_root_id: config.sync_root_id.clone(),
                            remote_root_id: config.roots.remote_root_id.clone(),
                        },
                        mode,
                        page_budget: Some(16),
                    },
                    &session,
                    &cancel,
                )
                .await
        };
        // Session acquisition can commit refreshed credentials: retain the ownership lease
        // and await its real completion before allowing a disconnect to remove the account.
        let result = round.await;
        let delay = match result {
            Ok(result) => {
                let complete = result.coverage == ObservationCoverage::Complete && !result.has_more;
                let observations = result.observations;
                for observation in &observations {
                    if let Some(id) = &observation.remote_id {
                        if observation.state == FileState::Present {
                            known.insert(id.clone(), observation.clone());
                        } else {
                            known.remove(id);
                        }
                    }
                }
                health(&handle, Source::Remote, false, !complete, &cancel).await;
                if !deliver(
                    &detectors,
                    &handle,
                    observations.into_iter().map(Observation::Remote).collect(),
                    &cancel,
                )
                .await
                {
                    break;
                }
                mode = RemoteObserveMode::Incremental;
                if complete {
                    invalidated = false;
                    Duration::from_secs(15)
                } else {
                    Duration::from_millis(50)
                }
            }
            Err(RemoteObserverError::Cancelled) => break,
            Err(_) => {
                health(&handle, Source::Remote, false, true, &cancel).await;
                if !invalidated {
                    let missing = known
                        .values()
                        .cloned()
                        .map(|mut data| {
                            data.state = FileState::Unavailable;
                            data.coverage = ObservationCoverage::Unavailable;
                            data.observed_at = super::observation::scanner::now();
                            data.checksum = Evidence::Unavailable;
                            data.version = Evidence::Unavailable;
                            data.capabilities = Evidence::Unavailable;
                            Observation::Remote(data)
                        })
                        .collect();
                    if !deliver(&detectors, &handle, missing, &cancel).await {
                        break;
                    }
                    invalidated = true;
                }
                health(&handle, Source::Remote, false, true, &cancel).await;
                mode = RemoteObserveMode::Recovery;
                Duration::from_secs(15)
            }
        };
        sleep(delay, &cancel).await;
    }
    if !cancel.is_cancelled() && !handle.is_finished() {
        failed(&handle, Source::Remote, &cancel).await;
    }
}
