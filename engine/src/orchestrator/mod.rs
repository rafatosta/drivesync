//! Coordinates paused detection and local Scanner preparation, without consuming events.
mod diagnostics;
mod startup;
use crate::{detectors::*, Engine};
pub use diagnostics::{DetectorDiagnostics, DetectorEventSummary};
pub use startup::*;
use std::sync::Arc;

/// Owns capture and the preparation barrier. No Analyzer or diagnostic consumer is attached.
pub struct Orchestrator {
    connector: Engine,
    detector: Arc<Detector>,
    preparation: startup::Preparation,
}
impl Orchestrator {
    /// Borrows the single public detection facade without exposing its children.
    pub fn detector(&self) -> &Detector {
        &self.detector
    }
    /// Stops capture and invalidates in-flight setup. The owning task must be cancelled first.
    pub fn stop_detector(&self, mode: DetectorStopMode) -> DetectorResult<DetectorStopResult> {
        self.preparation
            .epoch
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        self.preparation
            .snapshot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .state = PreparationState::Stopped;
        let _control = self
            .preparation
            .control
            .lock()
            .map_err(|_| DetectorFailure {
                kind: DetectorError::Internal,
                retryable: false,
                source: None,
                queue: None,
            })?;
        self.detector.stop(DetectorStopRequest { mode })
    }
    pub fn detector_status(&self) -> DetectorResult<DetectorStatus> {
        self.detector
            .status(DetectorStatusRequest {})
            .map(|result| result.status)
    }
    /// Returns the cached local source snapshot, not overall synchronization readiness.
    pub fn local_status(&self) -> LocalDetectorResult<LocalStatusResult> {
        self.detector_status()
            .map_err(diagnostics::local_error)?
            .local
            .local
            .ok_or_else(|| {
                diagnostics::local_error(DetectorFailure {
                    kind: DetectorError::Internal,
                    retryable: false,
                    source: None,
                    queue: None,
                })
            })
    }
    pub fn remote_diagnostics(&self) -> Option<RemoteDiagnostics> {
        self.detector.remote_diagnostics()
    }
    /// Reads cached per-source counters and the bounded tail without I/O.
    pub fn diagnostics(&self) -> DetectorDiagnostics {
        let queue = self.detector.source_queue_status(DetectorOrigin::Local);
        let events: Vec<_> = self
            .detector
            .recent_events(DetectorOrigin::Local)
            .into_iter()
            .map(diagnostics::summarize)
            .collect();
        let scope = self.detector.scope();
        DetectorDiagnostics {
            account_id: scope.account_id.clone(),
            sync_id: scope.sync_id.clone(),
            events_detected: queue.detected,
            initial_observations: queue.initial_observations,
            last_event_at: events.last().map(|event| event.observed_at),
            pending_events: Some(queue.pending.saturating_add(queue.leased)),
            leased_events: Some(queue.leased),
            confirmed_events: Some(queue.confirmed),
            delivery: Some(queue.delivery),
            queue_error: queue.last_error,
            events,
        }
    }
    /// The same shared connection service remains available independently of capture.
    pub fn connector(&self) -> &Engine {
        &self.connector
    }
}
