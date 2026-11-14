//! Minimal connection and Detector composition for terminal developer diagnostics.
//!
//! This is not the full synchronization coordinator: no Inventory, Scanner or
//! Analyzer is created. The current path consumes the unified Detector facade.
//! Earlier source-specific constructors remain compatibility APIs for legacy queues.
//! Source health stays separate; no diagnostic acknowledgement means synchronization.
mod delivery;
mod diagnostics;
use crate::{detectors::*, Engine};
use delivery::DiagnosticDelivery;
pub use diagnostics::{DetectorDiagnostics, DetectorEventSummary};
use diagnostics::{History, ObservationOnly, RecordingSink};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Owns one local producer and exposes the existing connector to an external UI.
///
/// Construction does not start observation. Drop stops the owned detector;
/// other clones of the connector remain usable. Scope/account association is
/// supplied by the caller, not inferred from paths or the account list.
pub struct Orchestrator {
    connector: Engine,
    detector: Capture,
    delivery_worker: Option<DiagnosticDelivery>,
    remote: Option<Arc<RemoteDetector>>,
    remote_delivery_worker: Option<DiagnosticDelivery>,
    history: Arc<Mutex<History>>,
}
enum Capture {
    Local(LocalDetector),
    Persistent(Arc<PersistentDetector>),
    Unified {
        detector: Arc<Detector>,
        delivery: DeliveryState,
    },
}
impl Orchestrator {
    /// Composes an existing connector and creates a stopped local detector.
    /// Performs only the detector's in-memory configuration validation. No
    /// authentication, filesystem scan, event delivery or worker is started.
    /// Invalid scope/settings preserve the detector's typed error.
    pub fn new(
        connector: Engine,
        mut local: LocalDetectorCreateRequest,
    ) -> LocalDetectorResult<Self> {
        let history = Arc::new(Mutex::new(History::new(&local.scope)));
        local.notifications = Arc::new(RecordingSink {
            history: history.clone(),
            receiver: local.notifications,
        });
        let detector = LocalDetector::new(local)?.detector;
        Ok(Self {
            connector,
            detector: Capture::Local(detector),
            delivery_worker: None,
            remote: None,
            remote_delivery_worker: None,
            history,
        })
    }

    /// Creates an explicitly observation-only session for developer inspection.
    /// Events are terminal diagnostic observations, not synchronization work.
    /// History is bounded and volatile; no durable queue or replay is promised.
    pub fn observe(
        connector: Engine,
        scope: LocalDetectorScope,
        settings: LocalDetectorSettings,
    ) -> LocalDetectorResult<Self> {
        Self::new(
            connector,
            LocalDetectorCreateRequest {
                scope,
                settings,
                notifications: Arc::new(ObservationOnly),
            },
        )
    }

    /// Developer diagnostic mode with a durable queue and an explicit terminal consumer.
    /// The caller selects delivery for a new queue; reopens preserve its stored policy.
    /// Opening/validation performs I/O; run off the UI thread. Queue contents survive Drop.
    pub fn observe_persistent(
        connector: Engine,
        scope: LocalDetectorScope,
        settings: LocalDetectorSettings,
        data_directory: PathBuf,
        delivery: DeliveryState,
    ) -> QueueResult<Self> {
        let history = Arc::new(Mutex::new(History::new(&scope)));
        let observer = Arc::new(RecordingSink {
            history: history.clone(),
            receiver: Arc::new(ObservationOnly),
        });
        let detector = Arc::new(PersistentDetector::open_with_observer(
            PersistentDetectorRequest {
                scope,
                settings,
                data_directory,
            },
            observer.clone(),
        )?);
        for file in detector.recent_events()? {
            observer
                .notify(file)
                .map_err(|_| QueueFailure::new(QueueError::Internal))?;
        }
        if detector.queue_is_new() && delivery == DeliveryState::Enabled {
            detector.resume()?;
        }
        let delivery_worker = Some(DiagnosticDelivery::new(detector.clone())?);
        Ok(Self {
            connector,
            detector: Capture::Persistent(detector),
            delivery_worker,
            remote: None,
            remote_delivery_worker: None,
            history,
        })
    }
    /// Composes only the public unified Detector. The terminal consumer is diagnostic.
    pub fn observe_detector(
        request: DetectorCreateRequest,
        delivery: DeliveryState,
    ) -> DetectorResult<Self> {
        let connector = request.connector.clone();
        let local_scope = LocalDetectorScope {
            account_id: request.scope.account_id.clone(),
            sync_id: request.scope.sync_id.clone(),
            local_root: request.scope.local_root.clone(),
        };
        let detector = Arc::new(Detector::new(request)?.detector);
        let delivery = if detector.queue_is_new() {
            delivery
        } else {
            detector.status(DetectorStatusRequest {})?.status.delivery
        };
        let delivery_worker = DiagnosticDelivery::new(detector.clone())?;
        Ok(Self {
            connector,
            detector: Capture::Unified { detector, delivery },
            delivery_worker: Some(delivery_worker),
            remote: None,
            remote_delivery_worker: None,
            history: Arc::new(Mutex::new(History::new(&local_scope))),
        })
    }
    /// Access to the composed public facade, without exposing its children.
    pub fn detector(&self) -> Option<&Detector> {
        if let Capture::Unified { detector, .. } = &self.detector {
            Some(detector.as_ref())
        } else {
            None
        }
    }
    pub fn start_detector(&self, mode: DetectorStartMode) -> DetectorResult<DetectorStartResult> {
        let Capture::Unified { detector, delivery } = &self.detector else {
            return Err(DetectorFailure {
                kind: DetectorError::InvalidState,
                retryable: false,
                source: None,
                queue: None,
            });
        };
        detector.start(DetectorStartRequest {
            mode,
            delivery: *delivery,
        })
    }
    pub fn stop_detector(&self, mode: DetectorStopMode) -> DetectorResult<DetectorStopResult> {
        self.detector()
            .ok_or(DetectorFailure {
                kind: DetectorError::InvalidState,
                retryable: false,
                source: None,
                queue: None,
            })?
            .stop(DetectorStopRequest { mode })
    }
    pub fn detector_status(&self) -> Option<DetectorStatus> {
        if let Capture::Unified { detector, .. } = &self.detector {
            detector
                .status(DetectorStatusRequest {})
                .ok()
                .map(|result| result.status)
        } else {
            None
        }
    }
    /// Attaches a stopped remote producer for Developer diagnostics. No HTTP runs here.
    /// Queues stay separate by origin; this is not the full synchronization coordinator.
    pub fn attach_remote(&mut self, request: RemoteDetectorCreateRequest) -> RemoteResult<()> {
        if matches!(self.detector, Capture::Unified { .. }) {
            return Err(RemoteFailure::new(RemoteError::InvalidState));
        }
        if self.remote.is_some() {
            return Err(RemoteFailure::new(RemoteError::InvalidState));
        }
        if request.scope.account_id
            != self
                .history
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .account
        {
            return Err(RemoteFailure::new(RemoteError::InvalidScope));
        }
        let remote = Arc::new(RemoteDetector::new(request)?);
        if remote.queue_is_new() {
            remote
                .resume()
                .map_err(|_| RemoteFailure::new(RemoteError::SinkUnavailable))?;
        }
        let delivery = DiagnosticDelivery::new(remote.clone())
            .map_err(|_| RemoteFailure::new(RemoteError::SinkUnavailable))?;
        self.remote = Some(remote);
        self.remote_delivery_worker = Some(delivery);
        Ok(())
    }
    pub fn remote_diagnostics(&self) -> Option<RemoteDiagnostics> {
        if let Capture::Unified { detector, .. } = &self.detector {
            return detector.remote_diagnostics();
        }
        self.remote.as_ref().map(|remote| remote.diagnostics())
    }
    pub fn pause_remote_delivery(&self) -> QueueResult<()> {
        if matches!(self.detector, Capture::Unified { .. }) {
            return self.pause_delivery();
        }
        self.remote
            .as_ref()
            .ok_or_else(|| QueueFailure::new(QueueError::InvalidRequest))?
            .pause()
    }
    pub fn resume_remote_delivery(&self) -> QueueResult<()> {
        if matches!(self.detector, Capture::Unified { .. }) {
            return self.resume_delivery();
        }
        self.remote
            .as_ref()
            .ok_or_else(|| QueueFailure::new(QueueError::InvalidRequest))?
            .resume()?;
        if let Some(worker) = &self.remote_delivery_worker {
            worker.wake();
        }
        Ok(())
    }
    /// Pauses only new delivery reservations. Existing accepted events remain durable.
    pub fn pause_delivery(&self) -> QueueResult<()> {
        if let Capture::Unified { detector, .. } = &self.detector {
            return detector
                .pause(DetectorPauseRequest {})
                .map(|_| ())
                .map_err(delivery::queue_error);
        }
        let Capture::Persistent(detector) = &self.detector else {
            return Err(QueueFailure::new(QueueError::InvalidRequest));
        };
        detector.pause()
    }
    pub fn resume_delivery(&self) -> QueueResult<()> {
        if let Capture::Unified { detector, .. } = &self.detector {
            detector
                .resume(DetectorResumeRequest {})
                .map_err(delivery::queue_error)?;
            if let Some(worker) = &self.delivery_worker {
                worker.wake();
            }
            return Ok(());
        }
        let Capture::Persistent(detector) = &self.detector else {
            return Err(QueueFailure::new(QueueError::InvalidRequest));
        };
        detector.resume()?;
        if let Some(worker) = &self.delivery_worker {
            worker.wake();
        }
        Ok(())
    }

    /// Returns per-account event context and counters without I/O. Retries of the
    /// current occurrence do not inflate the count. At most 100 events survive;
    /// Ephemeral sessions have no queue counts; persistent sessions use cached SQLite counts.
    pub fn diagnostics(&self) -> DetectorDiagnostics {
        if let Capture::Unified { detector, .. } = &self.detector {
            let scope = self.history.lock().unwrap_or_else(|e| e.into_inner());
            let queue = detector.source_queue_status(DetectorOrigin::Local);
            let events: Vec<_> = detector
                .recent_events(DetectorOrigin::Local)
                .into_iter()
                .map(diagnostics::summarize)
                .collect();
            return DetectorDiagnostics {
                account_id: scope.account.clone(),
                sync_id: scope.sync.clone(),
                events_detected: queue.detected,
                initial_observations: queue.initial_observations,
                last_event_at: events.last().map(|event| event.observed_at),
                pending_events: Some(queue.pending + queue.leased),
                leased_events: Some(queue.leased),
                confirmed_events: Some(queue.confirmed),
                delivery: Some(queue.delivery),
                queue_error: queue.last_error,
                events,
            };
        }
        let mut snapshot = self
            .history
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot();
        if let Capture::Persistent(detector) = &self.detector {
            let queue = detector.queue_status();
            snapshot.events_detected = queue.detected;
            snapshot.initial_observations = queue.initial_observations;
            snapshot.pending_events = Some(queue.pending.saturating_add(queue.leased));
            snapshot.leased_events = Some(queue.leased);
            snapshot.confirmed_events = Some(queue.confirmed);
            snapshot.delivery = Some(detector.delivery());
            snapshot.queue_error = queue.last_error;
        }
        snapshot
    }

    /// Borrows the same connection service supplied at construction.
    /// Its existing account/sign-in/folder APIs remain available without UI
    /// dependencies. This getter performs no I/O and starts no synchronization.
    /// Cloning the handle shares the connection service, not the detector.
    pub fn connector(&self) -> &Engine {
        &self.connector
    }

    /// Starts the configured producers; local operational starts remain idempotent.
    /// This synchronous setup must run off the UI thread. Returns the actual
    /// detector result; acceptance does not mean coverage verification finished.
    pub fn start(&self, request: LocalStartRequest) -> LocalDetectorResult<LocalStartResult> {
        if let Some(remote) = &self.remote {
            let _ = remote.start(RemoteStartRequest {
                mode: RemotePollMode::Incremental,
            });
        }
        match &self.detector {
            Capture::Local(d) => d.start(request),
            Capture::Persistent(d) => d.start(request),
            Capture::Unified { detector, delivery } => detector
                .start(DetectorStartRequest {
                    mode: if matches!(request.mode, LocalStartMode::Initial) {
                        DetectorStartMode::Initial
                    } else {
                        DetectorStartMode::Restart
                    },
                    delivery: *delivery,
                })
                .map(|_| LocalStartResult {
                    state: detector
                        .status(DetectorStatusRequest {})
                        .ok()
                        .and_then(|s| s.status.local.local)
                        .map(|s| s.state)
                        .unwrap_or(LocalDetectorState::Failed),
                    scheduled_scan: true,
                })
                .map_err(delivery::local_error),
        }
    }

    /// Stops both configured producers and waits for in-flight callbacks; run off the UI thread.
    /// Accepted sink events and the connector are preserved. Do not call this
    /// from the detector's own notification callback.
    pub fn stop(&self, request: LocalStopRequest) -> LocalDetectorResult<LocalStopResult> {
        if let Some(remote) = &self.remote {
            let _ = remote.stop(RemoteStopRequest {});
        }
        match &self.detector {
            Capture::Local(d) => d.stop(request),
            Capture::Persistent(d) => d.stop(request),
            Capture::Unified { detector, .. } => detector
                .stop(DetectorStopRequest {
                    mode: DetectorStopMode::StopNow,
                })
                .map(|result| LocalStopResult {
                    state: LocalDetectorState::Stopped,
                    in_flight: result.in_flight,
                })
                .map_err(delivery::local_error),
        }
    }

    /// Reads the local detector snapshot without I/O or waiting for its sink.
    /// The result reports local observation only, not overall sync readiness.
    pub fn status(&self, request: LocalStatusRequest) -> LocalDetectorResult<LocalStatusResult> {
        match &self.detector {
            Capture::Local(d) => d.status(request),
            Capture::Persistent(d) => d.status(request),
            Capture::Unified { detector, .. } => detector
                .status(DetectorStatusRequest {})
                .map_err(delivery::local_error)?
                .status
                .local
                .local
                .ok_or_else(|| {
                    delivery::local_error(DetectorFailure {
                        kind: DetectorError::Internal,
                        retryable: false,
                        source: None,
                        queue: None,
                    })
                }),
        }
    }
}
