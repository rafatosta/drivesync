//! One interruptible scheduler per remote scope; delivery never controls polling.
use super::{
    google::{Api, Backend, Source},
    *,
};
use crate::{
    detectors::{storage::Store, *},
    models::{File, Timestamp},
};
use std::{
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
struct Shared {
    source: Source,
    store: Arc<Store>,
    settings: RemoteDetectorSettings,
    status: Mutex<RemoteStatusResult>,
    events: Mutex<Vec<RemoteEventSummary>>,
    poll: Mutex<()>,
}
struct Worker {
    signal: Arc<(Mutex<bool>, Condvar)>,
    cancel: Cancellation,
    join: JoinHandle<()>,
}
/// Remote-only producer with a durable queue and explicit confirmable delivery.
/// No diagnostic consumer is implicit. All I/O methods must run outside the UI thread.
/// Drop cancels and joins the scheduler without clearing its checkpoint or queue.
pub struct RemoteDetector {
    shared: Arc<Shared>,
    worker: Mutex<Option<Worker>>,
    lifecycle: Mutex<()>,
    manual_cancel: Mutex<Option<Cancellation>>,
}
struct DurableBackend(Arc<Store>);
impl Backend for DurableBackend {
    fn load(&self) -> RemoteResult<Option<Vec<u8>>> {
        self.0
            .remote_state()
            .map_err(|_| RemoteFailure::new(RemoteError::CheckpointUnavailable))
    }
    fn save(&self, previous: Option<&[u8]>, value: &[u8]) -> RemoteResult<()> {
        self.0
            .save_remote_state(previous, value)
            .map_err(|error| RemoteFailure {
                kind: RemoteError::CheckpointUnavailable,
                retryable: error.retryable,
                retry_after: None,
            })
    }
    fn emit(&self, file: &File) -> RemoteResult<bool> {
        self.0.enqueue(file).map_err(|error| RemoteFailure {
            kind: RemoteError::SinkUnavailable,
            retryable: error.retryable,
            retry_after: None,
        })
    }
}
impl RemoteDetector {
    pub fn new(request: RemoteDetectorCreateRequest) -> RemoteResult<Self> {
        let api = request
            .connector
            .remote_detector_api(request.scope.account_id.clone());
        Self::open(request.scope, request.settings, request.data_directory, api)
    }
    pub(super) fn open(
        scope: RemoteDetectorScope,
        settings: RemoteDetectorSettings,
        directory: std::path::PathBuf,
        api: Arc<dyn Api>,
    ) -> RemoteResult<Self> {
        if scope.account_id.0.trim().is_empty()
            || scope.sync_id.0.trim().is_empty()
            || scope.remote_root_id.trim().is_empty()
            || scope.remote_root_id.len() > 4096
            || scope.remote_root_id.chars().any(char::is_control)
            || settings.polling_interval.is_zero()
            || settings.polling_interval > Duration::from_secs(86400)
            || settings.page_budget == 0
            || settings.page_budget > 1000
        {
            return Err(RemoteFailure::new(RemoteError::InvalidScope));
        }
        let store = Arc::new(
            Store::open_remote(
                &directory,
                &scope.account_id.0,
                &scope.sync_id.0,
                &scope.remote_root_id,
            )
            .map_err(|_| RemoteFailure::new(RemoteError::CheckpointUnavailable))?,
        );
        let source = Source {
            scope,
            api,
            backend: Arc::new(DurableBackend(store.clone())),
        };
        source.validate_progress()?;
        let events = store
            .recent()
            .map_err(|_| RemoteFailure::new(RemoteError::SinkUnavailable))?;
        Ok(Self {
            shared: Arc::new(Shared {
                source,
                store,
                status: Mutex::new(RemoteStatusResult {
                    state: RemoteState::Stopped,
                    activity: RemoteActivity::Inactive,
                    coverage: RemoteCoverage::Unavailable,
                    polling_interval: settings.polling_interval,
                    observed_in_current_round: 0,
                    emitted_in_current_round: 0,
                    last_observed_at: None,
                    last_poll_started_at: None,
                    next_poll_at: None,
                    last_poll_duration: None,
                    cursor: None,
                    consecutive_failures: 0,
                    recovery_attempts: 0,
                    last_error: None,
                }),
                settings,
                events: Mutex::new(events.into_iter().map(super::google::summarize).collect()),
                poll: Mutex::new(()),
            }),
            worker: Mutex::new(None),
            lifecycle: Mutex::new(()),
            manual_cancel: Mutex::new(None),
        })
    }
    /// Schedules the first round immediately and returns before any provider request.
    pub fn start(&self, request: RemoteStartRequest) -> RemoteResult<RemoteStartResult> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| RemoteFailure::new(RemoteError::Internal))?;
        let mut worker = self
            .worker
            .lock()
            .map_err(|_| RemoteFailure::new(RemoteError::Internal))?;
        if worker.is_some()
            || self
                .manual_cancel
                .lock()
                .map_err(|_| RemoteFailure::new(RemoteError::Internal))?
                .is_some()
        {
            return Err(RemoteFailure::new(RemoteError::InvalidState));
        }
        let signal = Arc::new((Mutex::new(false), Condvar::new()));
        let cancel = Cancellation::default();
        let shared = self.shared.clone();
        let running = signal.clone();
        let cancellation = cancel.clone();
        {
            let mut status = shared.status.lock().unwrap_or_else(|e| e.into_inner());
            status.state = RemoteState::Starting;
            status.next_poll_at = Some(now());
        }
        let join = thread::Builder::new()
            .name("remote-detector".into())
            .spawn(move || {
                let mut mode = request.mode;
                loop {
                    if cancellation.check().is_err() {
                        break;
                    }
                    let result = shared.poll(mode, shared.settings.page_budget, &cancellation);
                    if cancellation.check().is_err() {
                        break;
                    }
                    let delay = match &result {
                        Ok(result) => {
                            mode = RemotePollMode::Incremental;
                            if result.has_more {
                                Duration::from_millis(100)
                            } else {
                                shared.settings.polling_interval
                            }
                        }
                        Err(failure) => {
                            if failure.kind == RemoteError::CursorExpired {
                                mode = RemotePollMode::Recovery;
                            }
                            let failures = shared
                                .status
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .consecutive_failures;
                            let backoff = shared
                                .settings
                                .polling_interval
                                .saturating_mul(1u32 << failures.saturating_sub(1).min(5))
                                .min(Duration::from_secs(300));
                            // Authorization and inaccessible scopes remain visible and retry conservatively.
                            backoff
                                .max(failure.retry_after.unwrap_or_default())
                                .min(Duration::from_secs(86400))
                        }
                    };
                    {
                        let mut status = shared.status.lock().unwrap_or_else(|e| e.into_inner());
                        status.next_poll_at = Some(Timestamp {
                            unix_millis: now()
                                .unix_millis
                                .saturating_add(delay.as_millis().min(i64::MAX as u128) as i64),
                        });
                    }
                    // A monotonic deadline owns scheduling; UI clocks never start rounds.
                    let deadline = Instant::now() + delay;
                    let mut stop = running.0.lock().unwrap_or_else(|e| e.into_inner());
                    while !*stop && Instant::now() < deadline {
                        let remaining = deadline.saturating_duration_since(Instant::now());
                        stop = running
                            .1
                            .wait_timeout(stop, remaining)
                            .unwrap_or_else(|e| e.into_inner())
                            .0;
                    }
                    if *stop {
                        break;
                    }
                }
            })
            .map_err(|_| {
                let mut status = self.shared.status.lock().unwrap_or_else(|e| e.into_inner());
                status.state = RemoteState::Failed;
                status.activity = RemoteActivity::Unavailable;
                status.last_error = Some(RemoteError::Internal);
                RemoteFailure::new(RemoteError::Internal)
            })?;
        *worker = Some(Worker {
            signal,
            cancel,
            join,
        });
        Ok(RemoteStartResult {
            state: RemoteState::Starting,
            scheduled: true,
        })
    }
    /// Executes bounded work synchronously. Concurrent rounds are rejected, never overlapped.
    pub fn poll_once(&self, request: RemotePollRequest) -> RemoteResult<RemotePollResult> {
        let budget = request
            .page_budget
            .unwrap_or(self.shared.settings.page_budget);
        if budget == 0 || budget > 1000 {
            return Err(RemoteFailure::new(RemoteError::InvalidScope));
        }
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| RemoteFailure::new(RemoteError::Internal))?;
        if self
            .worker
            .lock()
            .map_err(|_| RemoteFailure::new(RemoteError::Internal))?
            .is_some()
        {
            return Err(RemoteFailure::new(RemoteError::InvalidState));
        }
        let cancel = Cancellation::default();
        {
            let mut manual = self
                .manual_cancel
                .lock()
                .map_err(|_| RemoteFailure::new(RemoteError::Internal))?;
            if manual.is_some() {
                return Err(RemoteFailure::new(RemoteError::InvalidState));
            }
            *manual = Some(cancel.clone());
        }
        drop(_lifecycle);
        let result = self.shared.poll(request.mode, budget, &cancel);
        *self.manual_cancel.lock().unwrap_or_else(|e| e.into_inner()) = None;
        result
    }
    /// Cancels in-flight provider I/O, joins the worker, and preserves all durable progress.
    pub fn stop(&self, _: RemoteStopRequest) -> RemoteResult<RemoteStopResult> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| RemoteFailure::new(RemoteError::Internal))?;
        if let Some(cancel) = self
            .manual_cancel
            .lock()
            .map_err(|_| RemoteFailure::new(RemoteError::Internal))?
            .as_ref()
        {
            cancel.cancel();
        }
        let worker = self
            .worker
            .lock()
            .map_err(|_| RemoteFailure::new(RemoteError::Internal))?
            .take();
        if let Some(worker) = worker {
            self.shared
                .status
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .state = RemoteState::Stopping;
            worker.cancel.cancel();
            *worker.signal.0.lock().unwrap_or_else(|e| e.into_inner()) = true;
            worker.signal.1.notify_all();
            worker
                .join
                .join()
                .map_err(|_| RemoteFailure::new(RemoteError::Internal))?;
        }
        let _round = self
            .shared
            .poll
            .lock()
            .map_err(|_| RemoteFailure::new(RemoteError::Internal))?;
        let mut status = self.shared.status.lock().unwrap_or_else(|e| e.into_inner());
        status.state = RemoteState::Stopped;
        status.activity = RemoteActivity::Inactive;
        status.next_poll_at = None;
        Ok(RemoteStopResult {
            state: RemoteState::Stopped,
        })
    }
    /// Cached, credential-free status; performs no I/O and never schedules polling.
    pub fn status(&self, _: RemoteStatusRequest) -> RemoteResult<RemoteStatusResult> {
        self.shared
            .status
            .lock()
            .map(|status| status.clone())
            .map_err(|_| RemoteFailure::new(RemoteError::Internal))
    }
    pub fn diagnostics(&self) -> RemoteDiagnostics {
        RemoteDiagnostics {
            status: self
                .shared
                .status
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            queue: self.shared.store.snapshot(),
            events: self
                .shared
                .events
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
        }
    }
    /// Updates cached counts and diagnostic history after another process changes delivery.
    pub fn refresh_queue(&self) -> QueueResult<()> {
        self.shared.store.refresh()?;
        *self.shared.events.lock().unwrap_or_else(|e| e.into_inner()) = self
            .shared
            .store
            .recent()?
            .into_iter()
            .map(super::google::summarize)
            .collect();
        Ok(())
    }
    pub(crate) fn queue_is_new(&self) -> bool {
        self.shared.store.created
    }
    pub fn pause(&self) -> QueueResult<()> {
        self.shared.store.set_delivery(DeliveryState::Paused)
    }
    pub fn resume(&self) -> QueueResult<()> {
        self.shared.store.set_delivery(DeliveryState::Enabled)
    }
    pub fn take_events(&self, request: TakeEventsRequest) -> QueueResult<FileBatch> {
        self.shared.store.claim(request)
    }
    pub fn acknowledge(&self, batch: &BatchId, disposition: EventDisposition) -> QueueResult<u32> {
        self.shared.store.acknowledge(batch, disposition)
    }
    pub fn release_events(&self, batch: &BatchId) -> QueueResult<u32> {
        self.shared.store.release(batch)
    }
}
impl Shared {
    fn poll(
        &self,
        mode: RemotePollMode,
        budget: u32,
        cancel: &Cancellation,
    ) -> RemoteResult<RemotePollResult> {
        let _round = self
            .poll
            .try_lock()
            .map_err(|_| RemoteFailure::new(RemoteError::InvalidState))?;
        let started = Instant::now();
        {
            let mut status = self.status.lock().unwrap_or_else(|e| e.into_inner());
            status.state = RemoteState::Running;
            status.activity = if mode == RemotePollMode::Recovery {
                RemoteActivity::Recovering
            } else {
                RemoteActivity::Polling
            };
            status.next_poll_at = None;
            status.last_poll_started_at = Some(now());
            status.observed_in_current_round = 0;
            status.emitted_in_current_round = 0;
            if mode == RemotePollMode::Recovery {
                status.recovery_attempts += 1;
            }
        }
        let result = self
            .source
            .poll_with_progress(mode, budget, cancel, &|observed, emitted| {
                let mut status = self.status.lock().unwrap_or_else(|e| e.into_inner());
                status.observed_in_current_round = observed;
                status.emitted_in_current_round = emitted;
            });
        if let Ok(events) = self.store.recent() {
            *self.events.lock().unwrap_or_else(|e| e.into_inner()) =
                events.into_iter().map(super::google::summarize).collect();
        }
        let mut status = self.status.lock().unwrap_or_else(|e| e.into_inner());
        status.last_poll_duration = Some(started.elapsed());
        match &result {
            Ok(result) => {
                status.state = RemoteState::Running;
                status.activity = RemoteActivity::Waiting;
                status.coverage = result.coverage;
                status.observed_in_current_round = result.observed;
                status.emitted_in_current_round = result.emitted;
                status.cursor = result.checkpoint_confirmed.clone();
                status.last_observed_at = Some(now());
                status.last_error = None;
                status.consecutive_failures = 0;
            }
            Err(error) => {
                status.state = RemoteState::Degraded;
                status.activity = RemoteActivity::Unavailable;
                status.coverage = RemoteCoverage::Partial;
                status.last_error = Some(error.kind);
                status.consecutive_failures = status.consecutive_failures.saturating_add(1);
            }
        }
        result
    }
}
impl Drop for RemoteDetector {
    fn drop(&mut self) {
        let _ = self.stop(RemoteStopRequest {});
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
