//! Explicit diagnostic terminal consumer; never stands in for Inventory ingestion.
use crate::detectors::*;
use std::{
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};
pub(super) trait DiagnosticQueue: Send + Sync {
    fn refresh(&self) -> QueueResult<()>;
    fn delivery(&self) -> DeliveryState;
    fn take(&self, request: TakeEventsRequest) -> QueueResult<FileBatch>;
    fn confirm(&self, batch: &BatchId) -> QueueResult<u32>;
}
impl DiagnosticQueue for PersistentDetector {
    fn refresh(&self) -> QueueResult<()> {
        self.refresh_queue()
    }
    fn delivery(&self) -> DeliveryState {
        self.delivery()
    }
    fn take(&self, request: TakeEventsRequest) -> QueueResult<FileBatch> {
        self.take_events(request)
    }
    fn confirm(&self, batch: &BatchId) -> QueueResult<u32> {
        self.acknowledge(batch, EventDisposition::Processed)
    }
}
impl DiagnosticQueue for RemoteDetector {
    fn refresh(&self) -> QueueResult<()> {
        self.refresh_queue()
    }
    fn delivery(&self) -> DeliveryState {
        self.diagnostics().queue.delivery
    }
    fn take(&self, request: TakeEventsRequest) -> QueueResult<FileBatch> {
        self.take_events(request)
    }
    fn confirm(&self, batch: &BatchId) -> QueueResult<u32> {
        self.acknowledge(batch, EventDisposition::Processed)
    }
}
pub(super) struct DiagnosticDelivery {
    signal: Arc<(Mutex<bool>, Condvar)>,
    worker: Option<JoinHandle<()>>,
}
impl DiagnosticDelivery {
    pub fn new(detector: Arc<dyn DiagnosticQueue>) -> QueueResult<Self> {
        let signal = Arc::new((Mutex::new(false), Condvar::new()));
        let running = signal.clone();
        let worker = thread::Builder::new()
            .name("detector-diagnostic-delivery".into())
            .spawn(move || {
                loop {
                    if *running.0.lock().unwrap_or_else(|e| e.into_inner()) {
                        break;
                    }
                    let _ = detector.refresh();
                    if detector.delivery() == DeliveryState::Enabled {
                        match detector.take(TakeEventsRequest {
                            max_events: 32,
                            lease_duration: Duration::from_secs(30),
                        }) {
                            Ok(batch) if !batch.files.is_empty() => {
                                // Decode and validation have completed. Diagnostic observation is terminal;
                                // this does not mean Inventory ingestion, upload or synchronization.
                                if let Some(id) = batch.batch_id {
                                    let _ = detector.confirm(&id);
                                }
                                continue;
                            }
                            _ => (),
                        }
                    }
                    let stop = running.0.lock().unwrap_or_else(|e| e.into_inner());
                    if *stop {
                        break;
                    }
                    let _guard = running
                        .1
                        .wait_timeout(stop, Duration::from_millis(250))
                        .unwrap_or_else(|e| e.into_inner());
                }
            })
            .map_err(|_| QueueFailure::new(QueueError::Internal))?;
        Ok(Self {
            signal,
            worker: Some(worker),
        })
    }
    pub fn wake(&self) {
        self.signal.1.notify_all();
    }
}
impl Drop for DiagnosticDelivery {
    fn drop(&mut self) {
        *self.signal.0.lock().unwrap_or_else(|e| e.into_inner()) = true;
        self.signal.1.notify_all();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
