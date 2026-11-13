//! Explicit diagnostic terminal consumer; never stands in for Inventory ingestion.
use crate::detectors::*;
use std::{
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};
pub(super) struct DiagnosticDelivery {
    signal: Arc<(Mutex<bool>, Condvar)>,
    worker: Option<JoinHandle<()>>,
}
impl DiagnosticDelivery {
    pub fn new(detector: Arc<PersistentDetector>) -> QueueResult<Self> {
        let signal = Arc::new((Mutex::new(false), Condvar::new()));
        let running = signal.clone();
        let worker = thread::Builder::new()
            .name("detector-diagnostic-delivery".into())
            .spawn(move || {
                loop {
                    if *running.0.lock().unwrap_or_else(|e| e.into_inner()) {
                        break;
                    }
                    let _ = detector.refresh_queue();
                    if detector.delivery() == DeliveryState::Enabled {
                        match detector.take_events(TakeEventsRequest {
                            max_events: 32,
                            lease_duration: Duration::from_secs(30),
                        }) {
                            Ok(batch) if !batch.files.is_empty() => {
                                // Decode and validation have completed. Diagnostic observation is terminal;
                                // this does not mean Inventory ingestion, upload or synchronization.
                                if let Some(id) = batch.batch_id {
                                    let _ = detector.acknowledge(&id, EventDisposition::Processed);
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
