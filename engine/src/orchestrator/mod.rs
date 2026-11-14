//! Minimal connection and unified Detector composition for developer diagnostics.
//! No Inventory, Scanner, Analyzer or synchronization executor is created.
mod delivery;
mod diagnostics;
use crate::{detectors::*, Engine};
use delivery::DiagnosticDelivery;
pub use diagnostics::{DetectorDiagnostics, DetectorEventSummary};
use std::sync::Arc;

/// Owns both producers through Detector and an explicit diagnostic terminal consumer.
/// Construction leaves capture stopped; dropping stops capture and preserves the queue.
pub struct Orchestrator {
    connector: Engine,
    detector: Arc<Detector>,
    delivery_worker: DiagnosticDelivery,
    initial_delivery: DeliveryState,
}
impl Orchestrator {
    /// Opens the queue without starting capture. Reopens preserve stored delivery policy.
    /// Performs blocking filesystem work and belongs off the UI thread.
    pub fn observe_detector(
        request: DetectorCreateRequest,
        delivery: DeliveryState,
    ) -> DetectorResult<Self> {
        let connector = request.connector.clone();
        let detector = Arc::new(Detector::new(request)?.detector);
        let initial_delivery = if detector.queue_is_new() {
            delivery
        } else {
            detector.status(DetectorStatusRequest {})?.status.delivery
        };
        let delivery_worker = DiagnosticDelivery::new(detector.clone())?;
        Ok(Self {
            connector,
            detector,
            delivery_worker,
            initial_delivery,
        })
    }
    /// Borrows the single public detection facade without exposing its children.
    pub fn detector(&self) -> &Detector {
        &self.detector
    }
    pub fn start_detector(&self, mode: DetectorStartMode) -> DetectorResult<DetectorStartResult> {
        self.detector.start(DetectorStartRequest {
            mode,
            delivery: self.initial_delivery,
        })
    }
    pub fn stop_detector(&self, mode: DetectorStopMode) -> DetectorResult<DetectorStopResult> {
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
            .map_err(delivery::local_error)?
            .local
            .local
            .ok_or_else(|| {
                delivery::local_error(DetectorFailure {
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
    /// Pauses only consumption for the account; both sources keep capturing durably.
    pub fn pause_delivery(&self) -> QueueResult<()> {
        self.detector
            .pause(DetectorPauseRequest {})
            .map(|_| ())
            .map_err(delivery::queue_error)
    }
    pub fn resume_delivery(&self) -> QueueResult<()> {
        self.detector
            .resume(DetectorResumeRequest {})
            .map_err(delivery::queue_error)?;
        self.delivery_worker.wake();
        Ok(())
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
