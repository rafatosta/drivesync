//! Local event production: native hints -> opaque metadata -> shared File sink.
use super::{
    changes::Changes,
    collector::FilesystemMetadataCollector,
    models::*,
    watcher::{Hints, Limits, Wake, Watches},
};
use crate::models::{
    File, FileData, FileEvent, FileEventKind, FileId, FileLocation, FileScope, LocalRelativePath,
    ObservationCoverage, ObservationUncertainty, Timestamp,
};
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Shared {
    status: Mutex<LocalStatusResult>,
    generation: AtomicU64,
    active: Mutex<Option<(u64, LocalCancellation, Arc<Wake>)>>,
    requested: Mutex<Vec<Option<LocalRelativePath>>>,
}
impl Shared {
    fn change(&self, generation: u64, edit: impl FnOnce(&mut LocalStatusResult)) {
        let mut status = self.status.lock().unwrap_or_else(|e| e.into_inner());
        if self.generation.load(Ordering::Acquire) == generation
            && status.state != LocalDetectorState::Stopping
        {
            edit(&mut status);
        }
    }
}
/// Local-only producer with real native capture; no queue, remote polling or sync policy.
/// Accepted sink events are owned by the receiver. Pending unaccepted work is volatile.
pub struct LocalDetector {
    request: LocalDetectorCreateRequest,
    shared: Arc<Shared>,
    control: Mutex<Option<JoinHandle<()>>>,
    root: Arc<Mutex<Option<Arc<FilesystemMetadataCollector>>>>,
}
impl LocalDetector {
    /// Validates configuration in memory and returns Stopped without I/O or callbacks.
    #[allow(
        clippy::new_ret_no_self,
        reason = "The creation contract returns the producer and its initial state"
    )]
    pub fn new(
        request: LocalDetectorCreateRequest,
    ) -> LocalDetectorResult<LocalDetectorCreateResult> {
        let scope = &request.scope;
        use std::{os::unix::ffi::OsStrExt, path::Component};
        if scope.account_id.0.trim().is_empty()
            || scope.sync_id.0.trim().is_empty()
            || !scope.local_root.is_absolute()
            || scope.local_root.as_os_str().as_bytes().contains(&0)
            || scope
                .local_root
                .components()
                .any(|c| matches!(c, Component::ParentDir) || c.as_os_str() == ".drivesync")
            || request.settings.max_pending_paths == 0
            || Instant::now()
                .checked_add(Duration::from_millis(
                    request.settings.debounce_ms.unwrap_or(0),
                ))
                .is_none()
        {
            return Err(LocalDetectorFailure::new(LocalDetectorError::InvalidScope));
        }
        let status = LocalStatusResult {
            state: LocalDetectorState::Stopped,
            activity: LocalActivity::Inactive,
            observed_in_current_round: 0,
            emitted_in_current_round: 0,
            last_observation_at: None,
            last_coverage: None,
            last_error: None,
        };
        Ok(LocalDetectorCreateResult {
            state: LocalDetectorState::Stopped,
            detector: Self {
                request,
                shared: Arc::new(Shared {
                    status: Mutex::new(status),
                    generation: AtomicU64::new(0),
                    active: Mutex::new(None),
                    requested: Mutex::new(Vec::new()),
                }),
                control: Mutex::new(None),
                root: Arc::new(Mutex::new(None)),
            },
        })
    }
    pub(crate) fn request_observation(
        &self,
        paths: Vec<Option<LocalRelativePath>>,
    ) -> LocalDetectorResult<()> {
        if !matches!(
            self.status(LocalStatusRequest {})?.state,
            LocalDetectorState::Starting
                | LocalDetectorState::Running
                | LocalDetectorState::Degraded
        ) {
            return Err(LocalDetectorFailure::new(LocalDetectorError::InvalidState));
        }
        let active = self
            .shared
            .active
            .lock()
            .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::Internal))?;
        let Some((_, cancel, wake)) = active.as_ref() else {
            return Err(LocalDetectorFailure::new(LocalDetectorError::InvalidState));
        };
        cancel.check()?;
        let mut queued = self
            .shared
            .requested
            .lock()
            .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::Internal))?;
        if queued.len().saturating_add(paths.len())
            > self.request.settings.max_pending_paths as usize
        {
            return Err(LocalDetectorFailure::new(LocalDetectorError::ResourceLimit));
        }
        queued.extend(paths);
        wake.signal();
        Ok(())
    }
    /// Installs root capture and starts asynchronous coverage verification and event production.
    /// Repeated starts while operational do not create a second worker. Physical failures
    /// before acceptance are returned; later failures are visible through status.
    pub fn start(&self, _request: LocalStartRequest) -> LocalDetectorResult<LocalStartResult> {
        {
            let status = self.status(LocalStatusRequest {})?;
            match status.state {
                LocalDetectorState::Starting
                | LocalDetectorState::Running
                | LocalDetectorState::Degraded => {
                    return Ok(LocalStartResult {
                        state: status.state,
                        scheduled_scan: false,
                    })
                }
                LocalDetectorState::Stopping | LocalDetectorState::Failed => {
                    return Err(LocalDetectorFailure::new(LocalDetectorError::InvalidState))
                }
                _ => {}
            }
        }
        let mut control = self
            .control
            .lock()
            .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::Internal))?;
        let mut status = self
            .shared
            .status
            .lock()
            .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::Internal))?;
        if status.state != LocalDetectorState::Stopped {
            return if matches!(
                status.state,
                LocalDetectorState::Starting
                    | LocalDetectorState::Running
                    | LocalDetectorState::Degraded
            ) {
                Ok(LocalStartResult {
                    state: status.state,
                    scheduled_scan: false,
                })
            } else {
                Err(LocalDetectorFailure::new(LocalDetectorError::InvalidState))
            };
        }
        let wake = Arc::new(Wake::new()?);
        let cancel = LocalCancellation::new();
        let generation = self
            .shared
            .generation
            .load(Ordering::Acquire)
            .checked_add(1)
            .ok_or_else(|| LocalDetectorFailure::new(LocalDetectorError::ResourceLimit))?;
        self.shared.generation.store(generation, Ordering::Release);
        *self
            .shared
            .active
            .lock()
            .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::Internal))? =
            Some((generation, cancel.clone(), wake.clone()));
        status.state = LocalDetectorState::Starting;
        status.activity = LocalActivity::Recovering;
        drop(status);
        let shared = self.shared.clone();
        let root_cache = self.root.clone();
        let scope = self.request.scope.clone();
        let sink = self.request.notifications.clone();
        let settings = self.request.settings.clone();
        let (ready, received) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("drivesync-local-detector".into())
            .spawn(move || {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    run(
                        shared.clone(),
                        generation,
                        root_cache,
                        scope,
                        sink,
                        settings,
                        cancel,
                        wake,
                        ready,
                    )
                }));
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) if error.kind == LocalDetectorError::Cancelled => {}
                    Ok(Err(error)) => shared.change(generation, |s| {
                        s.state = LocalDetectorState::Failed;
                        s.activity = LocalActivity::Unavailable;
                        s.last_error = Some(error.kind);
                        s.observed_in_current_round = 0;
                        s.emitted_in_current_round = 0;
                    }),
                    Err(_) => shared.change(generation, |s| {
                        s.state = LocalDetectorState::Failed;
                        s.activity = LocalActivity::Unavailable;
                        s.last_error = Some(LocalDetectorError::Internal);
                        s.observed_in_current_round = 0;
                        s.emitted_in_current_round = 0;
                    }),
                }
            })
            .map_err(|_| {
                self.shared.change(generation, |s| {
                    s.state = LocalDetectorState::Failed;
                    s.activity = LocalActivity::Unavailable;
                    s.last_error = Some(LocalDetectorError::Internal);
                });
                LocalDetectorFailure::new(LocalDetectorError::Internal)
            })?;
        *control = Some(worker);
        match received.recv() {
            Ok(Ok(())) => Ok(LocalStartResult {
                state: LocalDetectorState::Starting,
                scheduled_scan: true,
            }),
            result => {
                let error = match result {
                    Ok(Err(error)) => error,
                    _ => LocalDetectorFailure::new(LocalDetectorError::Internal),
                };
                if let Some(worker) = control.take() {
                    let _ = worker.join();
                }
                Err(error)
            }
        }
    }
    /// Cancels capture/collection and waits for the current callback to finish.
    /// Success is Stopped with zero in-flight work; accepted events are never erased.
    pub fn stop(&self, _request: LocalStopRequest) -> LocalDetectorResult<LocalStopResult> {
        let stopped_generation;
        {
            let mut status = self
                .shared
                .status
                .lock()
                .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::Internal))?;
            if status.state == LocalDetectorState::Stopped {
                return Ok(LocalStopResult {
                    state: LocalDetectorState::Stopped,
                    in_flight: 0,
                });
            }
            stopped_generation = self.shared.generation.load(Ordering::Acquire);
            status.state = LocalDetectorState::Stopping;
        }
        if let Some((generation, cancel, wake)) = self
            .shared
            .active
            .lock()
            .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::Internal))?
            .as_ref()
        {
            if *generation == stopped_generation {
                cancel.cancel();
                wake.signal();
            }
        }
        let mut control = self
            .control
            .lock()
            .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::Internal))?;
        if self.shared.generation.load(Ordering::Acquire) != stopped_generation {
            return Ok(LocalStopResult {
                state: LocalDetectorState::Stopped,
                in_flight: 0,
            });
        }
        if control
            .as_ref()
            .is_some_and(|worker| worker.thread().id() == thread::current().id())
        {
            return Err(LocalDetectorFailure::new(LocalDetectorError::InvalidState));
        }
        let failed = control.take().is_some_and(|worker| worker.join().is_err());
        *self
            .shared
            .active
            .lock()
            .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::Internal))? = None;
        let mut status = self
            .shared
            .status
            .lock()
            .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::Internal))?;
        status.state = LocalDetectorState::Stopped;
        status.activity = LocalActivity::Inactive;
        status.observed_in_current_round = 0;
        status.emitted_in_current_round = 0;
        if failed {
            status.last_error = Some(LocalDetectorError::Internal);
            return Err(LocalDetectorFailure::new(LocalDetectorError::Internal));
        }
        Ok(LocalStopResult {
            state: LocalDetectorState::Stopped,
            in_flight: 0,
        })
    }
    /// Returns a consistent in-memory snapshot without touching disk, sink or watcher.
    pub fn status(&self, _request: LocalStatusRequest) -> LocalDetectorResult<LocalStatusResult> {
        self.shared
            .status
            .lock()
            .map(|s| s.clone())
            .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::Internal))
    }
}
impl Drop for LocalDetector {
    fn drop(&mut self) {
        let _ = self.stop(LocalStopRequest {
            reason: LocalStopReason::Shutdown,
            requested_at: Timestamp { unix_millis: 0 },
        });
    }
}

/// OS-random generation prefix plus checked sequence supplies occurrence identities.
struct Ids {
    prefix: String,
    sequence: u64,
}
impl Ids {
    fn new(cancel: &LocalCancellation) -> LocalDetectorResult<Self> {
        let mut bytes = [0u8; 16];
        let mut offset = 0;
        while offset < bytes.len() {
            cancel.check()?;
            // SAFETY: the remaining initialized slice is writable and length is exact.
            let count = unsafe {
                libc::getrandom(
                    bytes[offset..].as_mut_ptr().cast(),
                    bytes.len() - offset,
                    libc::GRND_NONBLOCK,
                )
            };
            if count < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(LocalDetectorFailure::new(LocalDetectorError::Internal));
            }
            if count == 0 {
                return Err(LocalDetectorFailure::new(LocalDetectorError::Internal));
            }
            offset += count as usize;
        }
        Ok(Self {
            prefix: bytes.iter().map(|b| format!("{b:02x}")).collect(),
            sequence: 0,
        })
    }
    fn next(&mut self) -> LocalDetectorResult<FileId> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| LocalDetectorFailure::new(LocalDetectorError::ResourceLimit))?;
        Ok(FileId(format!("{}-{:016x}", self.prefix, self.sequence)))
    }
}
fn now() -> LocalDetectorResult<Timestamp> {
    let millis = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_millis()),
        Err(error) => i64::try_from(error.duration().as_millis()).map(|v| -v),
    }
    .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::Internal))?;
    Ok(Timestamp {
        unix_millis: millis,
    })
}
fn envelope(
    ids: &mut Ids,
    sync_id: &crate::models::SyncId,
    path: Option<LocalRelativePath>,
    result: Option<LocalMetadataResult>,
    unavailable: bool,
) -> LocalDetectorResult<File> {
    let observed_at = now()?;
    let mut file = File {
        id: ids.next()?,
        sync_id: sync_id.clone(),
        observed_at,
        scope: path.clone().map_or(FileScope::WholeSync, FileScope::Local),
        event: FileEvent {
            remote: None,
            kind: if unavailable {
                FileEventKind::ScopeUnavailable
            } else {
                FileEventKind::RecheckRequired
            },
            uncertainty: (!unavailable).then_some(ObservationUncertainty::ObservationUnstable),
            previous: None,
            current: None,
        },
        coverage: if unavailable {
            ObservationCoverage::Unavailable
        } else {
            ObservationCoverage::Partial
        },
        data: None,
    };
    if let Some(result) = result {
        file.coverage = match result.coverage {
            LocalScanCoverage::Complete => ObservationCoverage::Complete,
            LocalScanCoverage::Partial => ObservationCoverage::Partial,
            LocalScanCoverage::Unavailable => ObservationCoverage::Unavailable,
        };
        if let Some(data) = result.file {
            file.event.current = path.map(FileLocation::Local);
            file.data = Some(FileData::Local(data));
        }
    }
    file.validate()
        .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::ModelInvalid))?;
    Ok(file)
}

#[allow(
    clippy::too_many_arguments,
    reason = "Worker inputs are the immutable generation composition, never public options"
)]
fn run(
    shared: Arc<Shared>,
    generation: u64,
    cache: Arc<Mutex<Option<Arc<FilesystemMetadataCollector>>>>,
    scope: LocalDetectorScope,
    sink: Arc<dyn LocalNotificationSink>,
    settings: LocalDetectorSettings,
    cancel: LocalCancellation,
    wake: Arc<Wake>,
    ready: mpsc::SyncSender<LocalDetectorResult<()>>,
) -> LocalDetectorResult<()> {
    let setup: LocalDetectorResult<_> = (|| {
        cancel.check()?;
        let mut cache = cache
            .lock()
            .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::Internal))?;
        let root = match cache.as_ref() {
            Some(root) => root.clone(),
            None => {
                let root = Arc::new(FilesystemMetadataCollector::new(scope.clone())?);
                *cache = Some(root.clone());
                root
            }
        };
        let watches = Watches::root(root.clone(), &cancel)?;
        let ids = Ids::new(&cancel)?;
        Ok((root, watches, ids))
    })();
    let (collector, mut watches, mut ids) = match setup {
        Ok(setup) => {
            let _ = ready.send(Ok(()));
            setup
        }
        Err(error) => {
            let _ = ready.send(Err(error.clone()));
            return Err(error);
        }
    };
    let mut coverage_error = watches.discover(&cancel, Limits::default()).err();
    cancel.check()?;
    shared.change(generation, |s| {
        s.state = if coverage_error.is_some() {
            LocalDetectorState::Degraded
        } else {
            LocalDetectorState::Running
        };
        s.activity = LocalActivity::Watching;
        s.last_error = coverage_error.as_ref().map(|e| e.kind);
        s.last_coverage = Some(if coverage_error.is_some() {
            LocalScanCoverage::Partial
        } else {
            LocalScanCoverage::Complete
        });
    });
    let mut initial = envelope(&mut ids, &scope.sync_id, None, None, false)?;
    initial.event.uncertainty = Some(if coverage_error.is_some() {
        ObservationUncertainty::CoverageIncomplete
    } else {
        ObservationUncertainty::CaptureStarted
    });
    let mut pending = Some(initial);
    let mut changes = Changes::new(watches.entries.clone(), coverage_error.is_none());
    let mut recheck: Option<ObservationUncertainty> = None;
    let mut deadline = Some(Instant::now());
    let mut first_hint: Option<Instant> = None;
    let mut error = coverage_error.as_ref().map(|e| e.kind);
    let mut retry_delay = Duration::from_millis(250);
    loop {
        cancel.check()?;
        let requested = std::mem::take(
            &mut *shared
                .requested
                .lock()
                .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::Internal))?,
        );
        if !requested.is_empty() {
            for path in requested {
                if let Some(path) = path {
                    changes.request_recheck(path);
                } else {
                    coverage_error = watches.discover(&cancel, Limits::default()).err();
                    changes.replace_known(watches.entries.clone(), coverage_error.is_none());
                    recheck = Some(ObservationUncertainty::CoverageIncomplete);
                }
            }
            deadline = Some(Instant::now());
        }
        changes.expire(Instant::now());
        if changes.has_pending() && deadline.is_none() {
            deadline = Some(Instant::now());
        }
        if changes.has_open_write() && deadline.is_some_and(|at| Instant::now() >= at) {
            if let Some(first) = first_hint {
                let bound = first
                    + Duration::from_millis(settings.debounce_ms.unwrap_or(80))
                        .max(Duration::from_millis(500));
                if Instant::now() < bound {
                    deadline = Some(bound);
                }
            }
        }
        if deadline.is_some_and(|at| Instant::now() >= at) {
            shared.change(generation, |s| {
                s.activity = LocalActivity::Scanning;
                s.observed_in_current_round = 0;
                s.emitted_in_current_round = 0;
            });
            let mut failed = false;
            let mut round_coverage = if coverage_error.is_some() {
                LocalScanCoverage::Partial
            } else {
                LocalScanCoverage::Complete
            };
            loop {
                cancel.check()?;
                if pending.is_none() {
                    if let Some(reason) = recheck.take() {
                        let mut event = envelope(&mut ids, &scope.sync_id, None, None, false)?;
                        event.event.uncertainty = Some(reason);
                        pending = Some(event);
                    } else if let Some((path, change)) = changes.pop() {
                        if change.kind == FileEventKind::Deleted {
                            let mut event = envelope(
                                &mut ids,
                                &scope.sync_id,
                                Some(path.clone()),
                                None,
                                false,
                            )?;
                            event.event.kind = FileEventKind::Deleted;
                            event.event.uncertainty = None;
                            event.event.previous = Some(FileLocation::Local(path));
                            event.validate().map_err(|_| {
                                LocalDetectorFailure::new(LocalDetectorError::ModelInvalid)
                            })?;
                            pending = Some(event);
                            continue;
                        }
                        let result = LocalMetadataCollector::collect(
                            collector.as_ref(),
                            LocalMetadataRequest {
                                relative_path: path.clone(),
                                cancellation: cancel.clone(),
                            },
                        );
                        shared.change(generation, |s| s.observed_in_current_round += 1);
                        pending = Some(match result {
                            Ok(result) => {
                                if result.coverage != LocalScanCoverage::Complete {
                                    round_coverage = LocalScanCoverage::Partial;
                                    if result.file.is_some() {
                                        error = Some(LocalDetectorError::PartialObservation);
                                    }
                                }
                                let mut event = envelope(
                                    &mut ids,
                                    &scope.sync_id,
                                    Some(path),
                                    Some(result),
                                    false,
                                )?;
                                if event.data.is_none() {
                                    event.event.uncertainty =
                                        Some(change.uncertainty.unwrap_or(
                                            ObservationUncertainty::ObservationUnstable,
                                        ));
                                }
                                if event.data.is_some() {
                                    event.event.kind = change.kind;
                                    event.event.uncertainty = change.uncertainty;
                                    event.event.previous = change.previous.map(FileLocation::Local);
                                    event.validate().map_err(|_| {
                                        LocalDetectorFailure::new(LocalDetectorError::ModelInvalid)
                                    })?;
                                }
                                event
                            }
                            Err(failure) if failure.kind == LocalDetectorError::Cancelled => {
                                return Err(failure)
                            }
                            Err(failure) => {
                                error = Some(failure.kind);
                                round_coverage = LocalScanCoverage::Partial;
                                envelope(&mut ids, &scope.sync_id, Some(path), None, true)?
                            }
                        });
                    } else {
                        break;
                    }
                }
                let file = pending.as_ref().unwrap();
                shared.change(generation, |s| {
                    s.last_observation_at = Some(file.observed_at);
                });
                match sink.notify(file.clone()) {
                    Ok(()) => {
                        retry_delay = Duration::from_millis(250);
                        if error == Some(LocalDetectorError::NotificationFailed) {
                            error = coverage_error.as_ref().map(|e| e.kind);
                        }
                        if file.coverage != ObservationCoverage::Complete {
                            round_coverage = LocalScanCoverage::Partial;
                        }
                        shared.change(generation, |s| s.emitted_in_current_round += 1);
                        pending = None;
                    }
                    Err(failure) => {
                        error = Some(LocalDetectorError::NotificationFailed);
                        if !failure.retryable {
                            return Err(LocalDetectorFailure::new(
                                LocalDetectorError::NotificationFailed,
                            ));
                        }
                        failed = true;
                        break;
                    }
                }
            }
            shared.change(generation, |s| {
                s.activity = LocalActivity::Watching;
                s.last_coverage = Some(round_coverage);
                s.last_error = error;
                s.state = if error.is_some() {
                    LocalDetectorState::Degraded
                } else {
                    LocalDetectorState::Running
                };
                s.observed_in_current_round = 0;
                s.emitted_in_current_round = 0;
            });
            deadline = if failed {
                Some(Instant::now() + retry_delay)
            } else {
                None
            };
            if failed {
                retry_delay = (retry_delay * 2).min(Duration::from_secs(30));
            }
            first_hint = None;
            if !failed && round_coverage == LocalScanCoverage::Complete {
                error = None;
                shared.change(generation, |s| {
                    s.last_error = None;
                    s.state = LocalDetectorState::Running;
                });
            }
        }
        let wait_deadline = match (deadline, changes.deadline()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let timeout = wait_deadline.map_or(250, |at| {
            at.saturating_duration_since(Instant::now())
                .as_millis()
                .min(250) as i32
        });
        let hints = match watches.wait(&wake, &cancel, timeout) {
            Ok(hints) => hints,
            Err(failure) if failure.kind == LocalDetectorError::Cancelled => return Err(failure),
            Err(failure) => {
                cancel.check()?;
                let event = envelope(&mut ids, &scope.sync_id, None, None, true)?;
                let _ = sink.notify(event);
                return Err(failure);
            }
        };
        if !hints.any() {
            continue;
        }
        let Hints {
            events: incoming,
            structural,
            recheck: required,
            overflow,
        } = hints;
        if overflow {
            error = Some(LocalDetectorError::PartialObservation);
        }
        if overflow {
            changes.reset(Default::default());
        }
        for hint in incoming.into_iter().filter(|_| !overflow) {
            changes.push(hint);
            if changes.over_budget(settings.max_pending_paths as usize) {
                changes.reset(Default::default());
                recheck = Some(ObservationUncertainty::ResourceLimit);
                error = Some(LocalDetectorError::ResourceLimit);
                break;
            }
        }
        if structural || overflow {
            shared.change(generation, |s| s.activity = LocalActivity::Recovering);
            let mut next = match Watches::root(collector.clone(), &cancel) {
                Ok(next) => next,
                Err(failure) => {
                    cancel.check()?;
                    let event = envelope(&mut ids, &scope.sync_id, None, None, true)?;
                    let _ = sink.notify(event);
                    return Err(failure);
                }
            };
            coverage_error = next.discover(&cancel, Limits::default()).err();
            error = coverage_error.as_ref().map(|e| e.kind).or(error);
            changes.replace_known(next.entries.clone(), coverage_error.is_none());
            watches = next;
            recheck = Some(if overflow {
                ObservationUncertainty::NativeOverflow
            } else if recheck == Some(ObservationUncertainty::ResourceLimit) {
                ObservationUncertainty::ResourceLimit
            } else if coverage_error.is_some() {
                ObservationUncertainty::CoverageIncomplete
            } else {
                ObservationUncertainty::CaptureRebuilt
            });
        }
        if required && recheck.is_none() {
            recheck = Some(ObservationUncertainty::UnclassifiedSignal);
        }
        if changes.over_budget(settings.max_pending_paths as usize) {
            changes.reset(Default::default());
            recheck = Some(ObservationUncertainty::ResourceLimit);
            error = Some(LocalDetectorError::ResourceLimit);
        }
        let now = Instant::now();
        let first = *first_hint.get_or_insert(now);
        let quiet = Duration::from_millis(settings.debounce_ms.unwrap_or(80));
        let maximum = quiet.max(Duration::from_millis(500));
        deadline = Some((now + quiet).min(first + maximum));
    }
}
