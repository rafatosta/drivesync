//! Unified ownership of both producers and one transactional, durable event queue.
mod models;
pub use models::*;
mod interfaces;
use interfaces::Source;
mod sources;
use super::{storage::Store, *};
use crate::models::*;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

/// Owns both source lifecycles. It never acknowledges observations automatically.
/// All commands except status/diagnostics perform blocking work and belong on workers.
pub struct Detector {
    scope: DetectorScope,
    store: Arc<Store>,
    local: Arc<dyn Source>,
    remote: Arc<dyn Source>,
    remote_handle: Option<Arc<RemoteDetector>>,
    control: Mutex<()>,
    state: Mutex<DetectorState>,
    source_errors: Mutex<[Option<DetectorSourceError>; 2]>,
    history: Arc<Mutex<[VecDeque<File>; 2]>>,
}
struct QueueSink(Arc<Store>);
impl LocalNotificationSink for QueueSink {
    fn notify(&self, file: File) -> LocalDetectorResult<()> {
        self.0
            .enqueue_from(&file, DetectorOrigin::Local)
            .map(|_| ())
            .map_err(|error| LocalDetectorFailure {
                kind: LocalDetectorError::NotificationFailed,
                message: "Unified durable queue did not accept the occurrence".into(),
                retryable: error.retryable,
                progress: None,
            })
    }
}
fn observed_at() -> Timestamp {
    Timestamp {
        unix_millis: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(i64::MAX as u128) as i64,
    }
}
impl Detector {
    /// Constructs stopped producers using a provider selected from Accounts. No HTTP runs.
    #[allow(
        clippy::new_ret_no_self,
        reason = "The public creation contract returns initial state with the detector"
    )]
    pub fn new(request: DetectorCreateRequest) -> DetectorResult<DetectorCreateResult> {
        let scope = LocalDetectorScope {
            account_id: request.scope.account_id.clone(),
            sync_id: request.scope.sync_id.clone(),
            local_root: request.scope.local_root.clone(),
        };
        struct Noop;
        impl LocalNotificationSink for Noop {
            fn notify(&self, _: File) -> LocalDetectorResult<()> {
                Ok(())
            }
        }
        LocalDetector::new(LocalDetectorCreateRequest {
            scope: scope.clone(),
            settings: request.local_settings.clone(),
            notifications: Arc::new(Noop),
        })
        .map_err(|_| DetectorFailure::new(DetectorError::InvalidScope))?;
        let remote_request = RemoteDetectorCreateRequest {
            connector: request.connector,
            scope: RemoteDetectorScope {
                account_id: request.scope.account_id.clone(),
                sync_id: request.scope.sync_id.clone(),
                remote_root_id: request.scope.remote_root_id.clone(),
            },
            settings: request.remote_settings,
            data_directory: request.data_directory.clone(),
        };
        let provider = super::remote::validate_configuration(&remote_request).map_err(|error| {
            DetectorFailure {
                kind: DetectorError::InvalidScope,
                retryable: error.retryable,
                source: Some(DetectorSourceError::Remote(error.kind)),
                queue: None,
            }
        })?;
        let store = Arc::new(Store::open_unified(
            &request.data_directory,
            &scope,
            &provider.0,
            &request.scope.remote_root_id,
        )?);
        let local = LocalDetector::new(LocalDetectorCreateRequest {
            scope,
            settings: request.local_settings,
            notifications: Arc::new(QueueSink(store.clone())),
        })
        .map_err(|_| DetectorFailure::new(DetectorError::InvalidScope))?
        .detector;
        let remote = Arc::new(
            RemoteDetector::with_store(remote_request, store.clone()).map_err(|error| {
                DetectorFailure {
                    kind: DetectorError::SourceUnavailable,
                    retryable: error.retryable,
                    source: Some(DetectorSourceError::Remote(error.kind)),
                    queue: None,
                }
            })?,
        );
        let mut detector = Self::compose(
            request.scope,
            store,
            Arc::new(sources::Local(local)),
            Arc::new(sources::Remote(remote.clone())),
            request.event_available_sink,
        )?;
        detector.remote_handle = Some(remote);
        Ok(DetectorCreateResult {
            detector,
            state: DetectorState::Stopped,
        })
    }
    fn compose(
        scope: DetectorScope,
        store: Arc<Store>,
        local: Arc<dyn Source>,
        remote: Arc<dyn Source>,
        sink: Option<Arc<dyn EventAvailableSink>>,
    ) -> DetectorResult<Self> {
        let history: Arc<Mutex<[VecDeque<File>; 2]>> = Arc::new(Mutex::new([
            store.recent_for(DetectorOrigin::Local)?.into(),
            store.recent_for(DetectorOrigin::Remote)?.into(),
        ]));
        let recording = history.clone();
        let sync = scope.sync_id.clone();
        store.set_notification(Arc::new(move |file, origin| {
            {
                let mut history = recording.lock().unwrap_or_else(|e| e.into_inner());
                let events = &mut history[usize::from(origin == DetectorOrigin::Remote)];
                if events.len() == 100 {
                    events.pop_front();
                }
                events.push_back(file.clone());
            }
            if let Some(sink) = &sink {
                sink.notify_events_available(sync.clone());
            }
        }));
        Ok(Self {
            scope,
            store,
            local,
            remote,
            remote_handle: None,
            control: Mutex::new(()),
            state: Mutex::new(DetectorState::Stopped),
            source_errors: Mutex::new([None, None]),
            history,
        })
    }
    pub fn start(&self, request: DetectorStartRequest) -> DetectorResult<DetectorStartResult> {
        let _control = self
            .control
            .lock()
            .map_err(|_| DetectorFailure::new(DetectorError::Internal))?;
        let state = *self.state.lock().unwrap_or_else(|e| e.into_inner());
        if matches!(
            state,
            DetectorState::Running | DetectorState::Degraded | DetectorState::Starting
        ) {
            return Ok(DetectorStartResult {
                state: self.status(DetectorStatusRequest {})?.status.state,
            });
        }
        if state == DetectorState::Stopping {
            return Err(DetectorFailure::new(DetectorError::InvalidState));
        }
        // Delivery choice commits before either producer is started; no start/pause window exists.
        self.store.set_delivery(request.delivery)?;
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = DetectorState::Starting;
        let local = self
            .local
            .start(request.mode)
            .err()
            .and_then(|error| error.source);
        let remote = self
            .remote
            .start(request.mode)
            .err()
            .and_then(|error| error.source);
        *self.source_errors.lock().unwrap_or_else(|e| e.into_inner()) = [local, remote];
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = DetectorState::Running;
        Ok(DetectorStartResult {
            state: self.status(DetectorStatusRequest {})?.status.state,
        })
    }
    pub fn pause(&self, _: DetectorPauseRequest) -> DetectorResult<DetectorPauseResult> {
        let _control = self
            .control
            .lock()
            .map_err(|_| DetectorFailure::new(DetectorError::Internal))?;
        self.store.set_delivery(DeliveryState::Paused)?;
        Ok(DetectorPauseResult {
            delivery: DeliveryState::Paused,
        })
    }
    pub fn resume(&self, _: DetectorResumeRequest) -> DetectorResult<DetectorResumeResult> {
        let _control = self
            .control
            .lock()
            .map_err(|_| DetectorFailure::new(DetectorError::Internal))?;
        if matches!(
            *self.state.lock().unwrap_or_else(|e| e.into_inner()),
            DetectorState::Stopped | DetectorState::Stopping
        ) {
            return Err(DetectorFailure::new(DetectorError::InvalidState));
        }
        self.store.set_delivery(DeliveryState::Enabled)?;
        Ok(DetectorResumeResult {
            delivery: DeliveryState::Enabled,
        })
    }
    pub fn stop(&self, request: DetectorStopRequest) -> DetectorResult<DetectorStopResult> {
        let _control = self
            .control
            .lock()
            .map_err(|_| DetectorFailure::new(DetectorError::Internal))?;
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = DetectorState::Stopping;
        let paused = self.store.set_delivery(DeliveryState::Paused);
        let local = self.local.stop(request.mode);
        let remote = self.remote.stop(request.mode);
        let in_flight = u32::from(local.is_err()) + u32::from(remote.is_err());
        let state = if in_flight == 0 {
            DetectorState::Stopped
        } else {
            DetectorState::Stopping
        };
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = state;
        paused?;
        Ok(DetectorStopResult { state, in_flight })
    }
    /// Cached queue and individual source snapshots; no filesystem, SQLite or HTTP I/O.
    pub fn status(&self, _: DetectorStatusRequest) -> DetectorResult<DetectorStatusResult> {
        let lifecycle = *self.state.lock().unwrap_or_else(|e| e.into_inner());
        let mut local = self.local.status();
        let mut remote = self.remote.status();
        let errors = *self.source_errors.lock().unwrap_or_else(|e| e.into_inner());
        for (source, error) in [(&mut local, errors[0]), (&mut remote, errors[1])] {
            if let Some(error) = error {
                if !matches!(lifecycle, DetectorState::Stopped | DetectorState::Stopping) {
                    source.state = DetectorState::Failed;
                }
                source.last_error = Some(error);
            }
        }
        let queue = self.store.snapshot();
        let state = if matches!(lifecycle, DetectorState::Stopped | DetectorState::Stopping) {
            lifecycle
        } else if lifecycle == DetectorState::Starting {
            DetectorState::Starting
        } else if queue.last_error.is_some() {
            DetectorState::Degraded
        } else if local.state == DetectorState::Failed && remote.state == DetectorState::Failed {
            DetectorState::Failed
        } else if [local.state, remote.state].iter().any(|state| {
            matches!(
                state,
                DetectorState::Failed | DetectorState::Degraded | DetectorState::Stopped
            )
        }) {
            DetectorState::Degraded
        } else if [local.state, remote.state].contains(&DetectorState::Starting) {
            DetectorState::Starting
        } else {
            DetectorState::Running
        };
        Ok(DetectorStatusResult {
            status: DetectorStatus {
                state,
                delivery: queue.delivery,
                local,
                remote,
                queue,
            },
        })
    }
    pub fn request_scan(&self, request: DetectorScanRequest) -> DetectorResult<DetectorScanResult> {
        let _control = self
            .control
            .lock()
            .map_err(|_| DetectorFailure::new(DetectorError::Internal))?;
        if matches!(
            *self.state.lock().unwrap_or_else(|e| e.into_inner()),
            DetectorState::Stopped | DetectorState::Stopping
        ) {
            return Err(DetectorFailure::new(DetectorError::InvalidState));
        }
        if request
            .request_id
            .as_ref()
            .is_some_and(|id| id.is_empty() || id.len() > 256 || id.chars().any(char::is_control))
        {
            return Err(DetectorFailure::new(DetectorError::InvalidScope));
        }
        if let DetectorScanScope::Scopes(scopes) = &request.scope {
            if scopes.is_empty() || scopes.len()>4096 || scopes.iter().any(|scope| matches!(scope, FileScope::Remote(reference) if reference.account_id != self.scope.account_id || reference.file_id.0.trim().is_empty())) { return Err(DetectorFailure::new(DetectorError::InvalidScope)); }
        }
        let local = self.local.request_scan(&request);
        let remote = self.remote.request_scan(&request);
        local?;
        remote?;
        Ok(DetectorScanResult {
            accepted: true,
            correlation_id: request.request_id,
        })
    }
    pub fn take_events(&self, request: TakeEventsRequest) -> DetectorResult<TakeEventsResult> {
        Ok(TakeEventsResult {
            batch: self.store.claim(request)?,
        })
    }
    pub fn acknowledge(
        &self,
        request: AcknowledgeEventsRequest,
    ) -> DetectorResult<AcknowledgeEventsResult> {
        Ok(AcknowledgeEventsResult {
            confirmed_count: self
                .store
                .acknowledge(&request.batch_id, request.disposition)?,
        })
    }
    pub fn release_events(
        &self,
        request: ReleaseEventsRequest,
    ) -> DetectorResult<ReleaseEventsResult> {
        Ok(ReleaseEventsResult {
            released_count: self.store.release(&request.batch_id)?,
        })
    }
    pub fn refresh_queue(&self) -> DetectorResult<()> {
        self.store.refresh()?;
        *self.history.lock().unwrap_or_else(|e| e.into_inner()) = [
            self.store.recent_for(DetectorOrigin::Local)?.into(),
            self.store.recent_for(DetectorOrigin::Remote)?.into(),
        ];
        if let Some(remote) = &self.remote_handle {
            remote.refresh_queue()?;
        }
        Ok(())
    }
    pub fn source_queue_status(&self, origin: DetectorOrigin) -> QueueStats {
        self.store.snapshot_for(origin)
    }
    /// Bounded complete observations for diagnostics; returns cached clones without I/O.
    pub fn recent_events(&self, origin: DetectorOrigin) -> Vec<File> {
        self.history.lock().unwrap_or_else(|e| e.into_inner())
            [usize::from(origin == DetectorOrigin::Remote)]
        .iter()
        .cloned()
        .collect()
    }
    pub fn remote_diagnostics(&self) -> Option<RemoteDiagnostics> {
        self.remote_handle
            .as_ref()
            .map(|remote| remote.diagnostics())
    }
    pub(crate) fn queue_is_new(&self) -> bool {
        self.store.created
    }
}
impl Drop for Detector {
    fn drop(&mut self) {
        let _ = self.stop(DetectorStopRequest {
            mode: DetectorStopMode::StopNow,
        });
    }
}
#[cfg(test)]
mod tests;
