//! Minimal composition of the connection API and one local detector.
//!
//! This is not the full synchronization coordinator: no inventory, scanner,
//! remote detector or analysis is created. Persistent observation owns a local
//! queue and an explicit diagnostic consumer. The caller supplies an
//! existing connection handle and a local scope with its notification sink.
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
    history: Arc<Mutex<History>>,
}
enum Capture {
    Local(LocalDetector),
    Persistent(Arc<PersistentDetector>),
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
            history,
        })
    }
    /// Pauses only new delivery reservations. Existing accepted events remain durable.
    pub fn pause_delivery(&self) -> QueueResult<()> {
        let Capture::Persistent(detector) = &self.detector else {
            return Err(QueueFailure::new(QueueError::InvalidRequest));
        };
        detector.pause()
    }
    pub fn resume_delivery(&self) -> QueueResult<()> {
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
        let mut snapshot = self
            .history
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot();
        if let Capture::Persistent(detector) = &self.detector {
            let queue = detector.queue_status();
            snapshot.events_detected = queue.detected;
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

    /// Starts local capture; repeated operational starts are idempotent.
    /// This synchronous setup must run off the UI thread. Returns the actual
    /// detector result; acceptance does not mean coverage verification finished.
    pub fn start(&self, request: LocalStartRequest) -> LocalDetectorResult<LocalStartResult> {
        match &self.detector {
            Capture::Local(d) => d.start(request),
            Capture::Persistent(d) => d.start(request),
        }
    }

    /// Stops capture and waits for in-flight callbacks; run off the UI thread.
    /// Accepted sink events and the connector are preserved. Do not call this
    /// from the detector's own notification callback.
    pub fn stop(&self, request: LocalStopRequest) -> LocalDetectorResult<LocalStopResult> {
        match &self.detector {
            Capture::Local(d) => d.stop(request),
            Capture::Persistent(d) => d.stop(request),
        }
    }

    /// Reads the local detector snapshot without I/O or waiting for its sink.
    /// The result reports local observation only, not overall sync readiness.
    pub fn status(&self, request: LocalStatusRequest) -> LocalDetectorResult<LocalStatusResult> {
        match &self.detector {
            Capture::Local(d) => d.status(request),
            Capture::Persistent(d) => d.status(request),
        }
    }
}
