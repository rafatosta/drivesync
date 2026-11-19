//! Starts both observation sources and the persistent observation consumer.
use super::*;
use crate::inventory::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

/// Configuration of one synchronization, using the Detector's account/root identity.
pub struct OrchestratorCreateRequest {
    pub detector: DetectorCreateRequest,
    /// Account-scoped Analyzer implementation receiving each detector file.
    pub analyzer: Arc<dyn crate::analyzer::Analyzer>,
    /// Enables Analyzer-owned operations after live verification and policy checks.
    pub execute_operations: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparationState {
    Idle,
    Starting,
    ProcessingEvents,
    ReadyForAnalyzer,
    Failed,
    Cancelled,
    Stopped,
}

#[derive(Debug, Clone)]
pub enum PreparationFailure {
    Detector(DetectorFailure),
    Inventory(InventoryFailure),
    Analyzer(crate::analyzer::AnalysisError),
    AlreadyRunning,
    Internal,
}
impl std::fmt::Display for PreparationFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Detector(e) => write!(f, "Detector: {e}"),
            Self::Inventory(e) => write!(f, "Inventory: {e}"),
            Self::Analyzer(e) => write!(f, "Analyzer: {e}"),
            Self::AlreadyRunning => f.write_str("Preparation is already running"),
            Self::Internal => f.write_str("Could not prepare synchronization"),
        }
    }
}
impl std::error::Error for PreparationFailure {}

/// Cached preparation data; readiness describes the event consumer, not completed synchronization.
#[derive(Clone)]
pub struct PreparationStatus {
    pub state: PreparationState,
    pub error: Option<PreparationFailure>,
    /// Events acknowledged after the Analyzer returned for every file in the batch.
    pub events_processed: u64,
}

pub(super) struct Preparation {
    pub snapshot: Arc<super::status::Snapshot>,
    pub busy: Arc<AtomicBool>,
    pub epoch: Arc<std::sync::atomic::AtomicU64>,
    pub control: Arc<Mutex<()>>,
}
impl Preparation {
    pub fn new(publisher: Arc<crate::synchronization::Publisher>) -> Self {
        Self {
            snapshot: Arc::new(super::status::Snapshot::new(
                PreparationStatus {
                    state: PreparationState::Idle,
                    error: None,
                    events_processed: 0,
                },
                publisher,
            )),
            busy: Arc::new(AtomicBool::new(false)),
            epoch: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            control: Arc::new(Mutex::new(())),
        }
    }
}

struct CancelPreparation<'a> {
    owner: &'a Orchestrator,
    finished: bool,
}
impl Drop for CancelPreparation<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.owner.preparation.epoch.fetch_add(1, Ordering::AcqRel);
            let mut snapshot = self
                .owner
                .preparation
                .snapshot
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if snapshot.state != PreparationState::Stopped {
                snapshot.state = PreparationState::Cancelled;
                snapshot.error = None;
            }
        }
        self.owner.preparation.busy.store(false, Ordering::Release);
        self.owner.flow.wake.signal();
    }
}

impl Orchestrator {
    /// Starts Local and Remote with delivery paused, opens Inventory, and starts consumption.
    /// Blocking setup work runs on a worker so callers do not need a Tokio runtime.
    /// Acknowledgement means durable observation processing, never physical synchronization.
    pub async fn prepare(&self) -> Result<(), PreparationFailure> {
        if self.preparation.busy.swap(true, Ordering::AcqRel) {
            return Err(PreparationFailure::AlreadyRunning);
        }
        let mut guard = CancelPreparation {
            owner: self,
            finished: false,
        };
        let retired = {
            let mut snapshot = self
                .preparation
                .snapshot
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            snapshot.state = PreparationState::Starting;
            snapshot.error = None;
            self.flow.retire()
        };
        let detector = self.detector.clone();
        let epoch = self.preparation.epoch.fetch_add(1, Ordering::AcqRel) + 1;
        let epoch_token = self.preparation.epoch.clone();
        let control = self.preparation.control.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        let spawned = std::thread::Builder::new()
            .name("sync-session-preparation".into())
            .spawn(move || {
                if let Some(worker) = retired {
                    worker.join();
                }
                let work = (|| {
                    let _control = control.lock().map_err(|_| PreparationFailure::Internal)?;
                    if epoch_token.load(Ordering::Acquire) != epoch {
                        return Err(PreparationFailure::Internal);
                    }
                    let scope = detector.scope();
                    let inventory = Arc::new(
                        InventoryManager::open(OpenInventoryRequest {
                            local_root: scope.local_root.clone(),
                            expected: SyncIdentity {
                                sync_id: scope.sync_id.clone(),
                                account_id: scope.account_id.clone(),
                                remote_root_id: scope.remote_root_id.clone(),
                            },
                            create_if_missing: true,
                        })
                        .map_err(PreparationFailure::Inventory)?
                        .inventory,
                    );
                    inventory.set_order_source(detector.clone());
                    detector
                        .pause(DetectorPauseRequest {})
                        .map_err(PreparationFailure::Detector)?;
                    let previous = detector
                        .status(DetectorStatusRequest {})
                        .map_err(PreparationFailure::Detector)?
                        .status;
                    if matches!(
                        previous.state,
                        DetectorState::Running | DetectorState::Degraded
                    ) && previous.local.local.as_ref().is_some_and(|local| {
                        matches!(
                            local.state,
                            LocalDetectorState::Failed | LocalDetectorState::Stopped
                        )
                    }) {
                        detector
                            .stop(DetectorStopRequest {
                                mode: DetectorStopMode::StopNow,
                            })
                            .map_err(PreparationFailure::Detector)?;
                    }
                    detector
                        .start(DetectorStartRequest {
                            mode: DetectorStartMode::Restart,
                            delivery: DeliveryState::Paused,
                        })
                        .map_err(PreparationFailure::Detector)?;
                    loop {
                        if epoch_token.load(Ordering::Acquire) != epoch {
                            return Err(PreparationFailure::Internal);
                        }
                        let status = detector
                            .status(DetectorStatusRequest {})
                            .map_err(PreparationFailure::Detector)?
                            .status;
                        match status.local.local.as_ref().map(|local| local.state) {
                            Some(LocalDetectorState::Starting) => {
                                std::thread::sleep(std::time::Duration::from_millis(5));
                            }
                            Some(LocalDetectorState::Running | LocalDetectorState::Degraded) => {
                                break;
                            }
                            _ => {
                                return Err(PreparationFailure::Detector(DetectorFailure {
                                    kind: DetectorError::SourceUnavailable,
                                    retryable: true,
                                    source: Some(DetectorSourceError::Local(
                                        LocalDetectorError::RootUnavailable,
                                    )),
                                    queue: None,
                                }));
                            }
                        }
                    }
                    Ok(inventory)
                })();
                let _ = tx.send(work);
            });

        let result = async {
            spawned.map_err(|_| PreparationFailure::Internal)?;
            let inventory = rx.await.map_err(|_| PreparationFailure::Internal)??;
            if self.preparation.epoch.load(Ordering::Acquire) != epoch {
                std::thread::spawn(move || drop(inventory));
                return Err(PreparationFailure::Internal);
            }
            if let Ok(status) = self.detector.status(DetectorStatusRequest {}) {
                self.preparation.snapshot.publish_detector(&status.status);
            }
            self.preparation
                .snapshot
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .state = PreparationState::ProcessingEvents;
            self.flow
                .start(flow::Context {
                    analyzer: self.analyzer.clone(),
                    execution: self.execution.clone(),
                    detector: self.detector.clone(),
                    inventory,
                    epoch: self.preparation.epoch.clone(),
                    expected: epoch,
                    control: self.preparation.control.clone(),
                    snapshot: self.preparation.snapshot.clone(),
                    busy: self.preparation.busy.clone(),
                })
                .await?;
            if self.preparation.epoch.load(Ordering::Acquire) != epoch {
                return Err(PreparationFailure::Internal);
            }
            Ok::<_, PreparationFailure>(())
        }
        .await;

        {
            let mut snapshot = self
                .preparation
                .snapshot
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if snapshot.state != PreparationState::Stopped {
                match &result {
                    Ok(()) => snapshot.state = PreparationState::ReadyForAnalyzer,
                    Err(error) => {
                        snapshot.state = PreparationState::Failed;
                        snapshot.error = Some(error.clone());
                    }
                }
            }
        }
        guard.finished = true;
        result
    }

    /// Reads only cached snapshots; no SQL, worker join or filesystem access.
    pub fn preparation_status(&self) -> PreparationStatus {
        self.preparation
            .snapshot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}
