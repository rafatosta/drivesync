//! Paused capture, local scanning and a finite decision pass over the persisted inventory.
use super::*;
use crate::{inventory::*, scanner::*};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

/// Configuration of one synchronization, using the Detector's account/root identity.
pub struct OrchestratorCreateRequest {
    pub detector: DetectorCreateRequest,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparationState {
    Idle,
    Starting,
    Scanning,
    Analyzing,
    ProcessingEvents,
    ReadyForAnalyzer,
    Partial,
    Failed,
    Cancelled,
    Stopped,
}
#[derive(Debug, Clone)]
pub enum PreparationFailure {
    Detector(DetectorFailure),
    Inventory(InventoryFailure),
    Scanner(ScannerError),
    Analyzer(crate::analyzer::AnalyzerError),
    InvalidObservation(crate::models::FileId),
    AlreadyRunning,
    Internal,
}
impl std::fmt::Display for PreparationFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Detector(e) => write!(f, "Detector: {e}"),
            Self::Inventory(e) => write!(f, "Inventory: {e}"),
            Self::Scanner(e) => write!(f, "Scanner: {e}"),
            Self::Analyzer(e) => write!(f, "Analyzer: {e}"),
            Self::InvalidObservation(_) => {
                f.write_str("Observation could not be safely associated with an inventory item")
            }
            Self::AlreadyRunning => f.write_str("Preparation is already running"),
            Self::Internal => f.write_str("Could not prepare synchronization"),
        }
    }
}
impl std::error::Error for PreparationFailure {}
/// Cached preparation data; ReadyForAnalyzer is a local prerequisite, not completed synchronization.
#[derive(Clone)]
pub struct PreparationStatus {
    pub state: PreparationState,
    pub scanner: Option<ScannerStatus>,
    pub report: Option<Arc<ScanReport>>,
    pub error: Option<PreparationFailure>,
    /// Events acknowledged after durable ingestion and analysis by this Orchestrator.
    pub events_processed: u64,
}
pub(super) struct Preparation {
    pub scanner: Mutex<Option<Arc<Scanner>>>,
    pub snapshot: Arc<Mutex<PreparationStatus>>,
    pub busy: Arc<AtomicBool>,
    pub epoch: Arc<std::sync::atomic::AtomicU64>,
    pub control: Arc<Mutex<()>>,
}
impl Preparation {
    pub fn new() -> Self {
        Self {
            scanner: Mutex::new(None),
            snapshot: Arc::new(Mutex::new(PreparationStatus {
                state: PreparationState::Idle,
                scanner: None,
                report: None,
                error: None,
                events_processed: 0,
            })),
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
            let mut s = self
                .owner
                .preparation
                .snapshot
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if s.state != PreparationState::Stopped {
                s.state = PreparationState::Cancelled;
                s.error = None;
            }
        }
        self.owner.preparation.busy.store(false, Ordering::Release);
        self.owner.flow.wake.signal();
    }
}
impl Orchestrator {
    /// Opens the durable detection queue without capture, consumption or Inventory initialization.
    /// Blocking: construct off the GTK thread. Existing delivery settings cannot enable a consumer.
    pub fn new(mut request: OrchestratorCreateRequest) -> DetectorResult<Self> {
        let connector = request.detector.connector.clone();
        let wake = flow::Wake::new(request.detector.event_available_sink.take());
        request.detector.event_available_sink = Some(wake.clone());
        let detector = Arc::new(Detector::new(request.detector)?.detector);
        detector.pause(DetectorPauseRequest {})?;
        Ok(Self {
            connector,
            detector,
            preparation: Preparation::new(),
            flow: flow::Flow::new(wake),
        })
    }
    /// Starts both sources with delivery paused before creating/opening Inventory and scanning.
    /// Every blocking setup operation is delegated to a finite worker. The caller needs no Tokio runtime.
    /// A complete scan is followed by initial analysis and continuous event delivery.
    /// Acknowledgement means durable observation processing, never physical synchronization.
    pub async fn prepare(&self) -> Result<Arc<ScanReport>, PreparationFailure> {
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
            let scanner = self
                .preparation
                .scanner
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
            let report = snapshot.report.take();
            snapshot.state = PreparationState::Starting;
            snapshot.error = None;
            (scanner, report, self.flow.retire())
        };
        let detector = self.detector.clone();
        let epoch = self.preparation.epoch.fetch_add(1, Ordering::AcqRel) + 1;
        let epoch_token = self.preparation.epoch.clone();
        let control = self.preparation.control.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        let spawned = std::thread::Builder::new()
            .name("sync-local-preparation".into())
            .spawn(move || {
                // Closing old SQLite handles and freeing large reports must also stay off GTK.
                let (scanner, report, worker) = retired;
                if let Some(worker) = worker {
                    worker.join();
                }
                if let Some(scanner) = &scanner {
                    while scanner.status().state == ScannerState::Running {
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                }
                drop((scanner, report));
                let work = (|| {
                    let _control = control.lock().map_err(|_| PreparationFailure::Internal)?;
                    if epoch_token.load(Ordering::Acquire) != epoch {
                        return Err(PreparationFailure::Internal);
                    }
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
                        // A failed source must be stopped before LocalDetector can start it again.
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
                                std::thread::sleep(std::time::Duration::from_millis(5))
                            }
                            Some(LocalDetectorState::Running | LocalDetectorState::Degraded) => {
                                break
                            }
                            _ => {
                                return Err(PreparationFailure::Detector(DetectorFailure {
                                    kind: DetectorError::SourceUnavailable,
                                    retryable: true,
                                    source: Some(DetectorSourceError::Local(
                                        LocalDetectorError::RootUnavailable,
                                    )),
                                    queue: None,
                                }))
                            }
                        }
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
                    Scanner::new(ScannerConfig {
                        inventory: inventory.clone(),
                        root: LocalRoot {
                            sync_id: scope.sync_id.clone(),
                            path: scope.local_root.clone(),
                        },
                    })
                    .map(|scanner| (Arc::new(scanner), inventory))
                    .map_err(PreparationFailure::Scanner)
                })();
                let _ = tx.send(work);
            });
        let result = async {
            spawned.map_err(|_| PreparationFailure::Internal)?;
            let (scanner, inventory) = rx.await.map_err(|_| PreparationFailure::Internal)??;
            if self.preparation.epoch.load(Ordering::Acquire) != epoch {
                std::thread::spawn(move || drop((scanner, inventory)));
                return Err(PreparationFailure::Internal);
            }
            {
                let mut snapshot = self
                    .preparation
                    .snapshot
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                *self
                    .preparation
                    .scanner
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()) = Some(scanner.clone());
                snapshot.state = PreparationState::Scanning;
            }
            let report = scanner
                .scan(ScanRequest {
                    sync_id: self.detector.scope().sync_id.clone(),
                })
                .await
                .map_err(PreparationFailure::Scanner)?;
            if self.preparation.epoch.load(Ordering::Acquire) != epoch {
                return Err(PreparationFailure::Internal);
            }
            let report = Arc::new(report);
            if report.ready_for_analysis() {
                {
                    let mut snapshot = self
                        .preparation
                        .snapshot
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    if self.preparation.epoch.load(Ordering::Acquire) != epoch {
                        return Err(PreparationFailure::Internal);
                    }
                    snapshot.state = PreparationState::Analyzing;
                    snapshot.report = Some(report.clone());
                }
                self.analyze_inventory(inventory.clone(), epoch).await?;
                let scope = self.detector.scope();
                self.flow
                    .start(flow::Context {
                        detector: self.detector.clone(),
                        scanner,
                        inventory,
                        identity: SyncIdentity {
                            sync_id: scope.sync_id.clone(),
                            account_id: scope.account_id.clone(),
                            remote_root_id: scope.remote_root_id.clone(),
                        },
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
            }
            Ok::<_, PreparationFailure>(report)
        }
        .await;
        {
            let mut s = self
                .preparation
                .snapshot
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if s.state != PreparationState::Stopped {
                match &result {
                    Ok(report) => {
                        s.state = if report.ready_for_analysis() {
                            PreparationState::ReadyForAnalyzer
                        } else {
                            PreparationState::Partial
                        };
                        s.report = Some(report.clone());
                    }
                    Err(error) => {
                        s.state = PreparationState::Failed;
                        s.error = Some(error.clone());
                    }
                }
            }
        }
        guard.finished = true;
        result
    }
    /// Reads only cached snapshots; no SQL, worker join or filesystem access.
    pub fn preparation_status(&self) -> PreparationStatus {
        let snapshot = self
            .preparation
            .snapshot
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut value = snapshot.clone();
        value.scanner = self
            .preparation
            .scanner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|s| s.status());
        value
    }
}
