//! Minimal composition of the connection API and one local detector.
//!
//! This is not the full synchronization coordinator: no inventory, scanner,
//! remote detector, event queue or analysis is created. The caller supplies an
//! existing connection handle and a local scope with its notification sink.
mod diagnostics;
use crate::{detectors::*, Engine};
pub use diagnostics::{DetectorDiagnostics, DetectorEventSummary};
use diagnostics::{History, ObservationOnly, RecordingSink};
use std::sync::{Arc, Mutex};

/// Owns one local producer and exposes the existing connector to an external UI.
///
/// Construction does not start observation. Drop stops the owned detector;
/// other clones of the connector remain usable. Scope/account association is
/// supplied by the caller, not inferred from paths or the account list.
pub struct Orchestrator {
    connector: Engine,
    detector: LocalDetector,
    history: Arc<Mutex<History>>,
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
            detector,
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

    /// Returns per-account event context and counters without I/O. Retries of the
    /// current occurrence do not inflate the count. At most 100 events survive;
    /// pending_events is None because durable queue telemetry is not available.
    pub fn diagnostics(&self) -> DetectorDiagnostics {
        self.history
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot()
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
        self.detector.start(request)
    }

    /// Stops capture and waits for in-flight callbacks; run off the UI thread.
    /// Accepted sink events and the connector are preserved. Do not call this
    /// from the detector's own notification callback.
    pub fn stop(&self, request: LocalStopRequest) -> LocalDetectorResult<LocalStopResult> {
        self.detector.stop(request)
    }

    /// Reads the local detector snapshot without I/O or waiting for its sink.
    /// The result reports local observation only, not overall sync readiness.
    pub fn status(&self, request: LocalStatusRequest) -> LocalDetectorResult<LocalStatusResult> {
        self.detector.status(request)
    }
}
