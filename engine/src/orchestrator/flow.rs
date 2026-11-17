//! Continuous observation ingestion. Detector owns the queue; Analyzer owns decisions.
//! This worker never claims Inventory operations or executes physical synchronization.
use super::{analysis, ingestion, PreparationFailure, PreparationState};
use crate::{detectors::*, inventory::*, models::SyncId, scanner::*};
use std::{
    collections::BTreeSet,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Condvar, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};

/// Wakeups carry availability only; no in-memory event backlog exists here.
pub(super) struct Wake {
    version: Mutex<u64>,
    changed: Condvar,
    external: Option<Arc<dyn EventAvailableSink>>,
}
impl Wake {
    pub fn new(external: Option<Arc<dyn EventAvailableSink>>) -> Arc<Self> {
        Arc::new(Self {
            version: Mutex::new(0),
            changed: Condvar::new(),
            external,
        })
    }
    pub fn signal(&self) {
        let mut v = self.version.lock().unwrap_or_else(|e| e.into_inner());
        *v = v.wrapping_add(1);
        self.changed.notify_all();
    }
    fn version(&self) -> u64 {
        *self.version.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn wait(&self, version: u64, stop: &AtomicBool) {
        let guard = self.version.lock().unwrap_or_else(|e| e.into_inner());
        // Heartbeat also revisits leases left by an unclean previous process.
        let _ = self
            .changed
            .wait_timeout_while(guard, Duration::from_secs(1), |v| {
                *v == version && !stop.load(Ordering::Acquire)
            });
    }
}
impl EventAvailableSink for Wake {
    fn notify_events_available(&self, sync_id: SyncId) {
        self.signal();
        if let Some(external) = &self.external {
            external.notify_events_available(sync_id);
        }
    }
}
pub(super) struct Worker {
    stop: Arc<AtomicBool>,
    wake: Arc<Wake>,
    thread: Option<JoinHandle<()>>,
}
impl Worker {
    fn cancel(&self) {
        self.stop.store(true, Ordering::Release);
        self.wake.signal();
    }
    pub fn join(mut self) {
        self.cancel();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
pub(super) struct Flow {
    pub wake: Arc<Wake>,
    worker: Mutex<Option<Worker>>,
}
impl Flow {
    pub fn new(wake: Arc<Wake>) -> Self {
        Self {
            wake,
            worker: Mutex::new(None),
        }
    }
    pub fn retire(&self) -> Option<Worker> {
        let worker = self.worker.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(worker) = &worker {
            worker.cancel();
        }
        worker
    }
    pub fn cancel(&self) {
        if let Some(worker) = self
            .worker
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            worker.cancel();
        }
    }
    pub async fn start(&self, context: Context) -> Result<(), PreparationFailure> {
        let rx = {
            let mut slot = self.worker.lock().unwrap_or_else(|e| e.into_inner());
            if slot.is_some() {
                return Err(PreparationFailure::AlreadyRunning);
            }
            let stop = Arc::new(AtomicBool::new(false));
            let stopping = stop.clone();
            let wake = self.wake.clone();
            let waking = wake.clone();
            let (tx, rx) = tokio::sync::oneshot::channel();
            let thread = std::thread::Builder::new()
                .name("sync-observation-flow".into())
                .spawn(move || run(context, stopping, waking, tx))
                .map_err(|_| PreparationFailure::Internal)?;
            *slot = Some(Worker {
                stop,
                wake,
                thread: Some(thread),
            });
            rx
        };
        rx.await.map_err(|_| PreparationFailure::Internal)?
    }
}
impl Drop for Flow {
    fn drop(&mut self) {
        self.cancel();
    }
}
pub(super) struct Context {
    pub connector: crate::Engine,
    pub execution: Arc<super::execution::ExecutionControl>,
    pub detector: Arc<Detector>,
    pub scanner: Arc<Scanner>,
    pub inventory: Arc<InventoryManager>,
    pub identity: SyncIdentity,
    pub epoch: Arc<AtomicU64>,
    pub expected: u64,
    pub control: Arc<Mutex<()>>,
    pub snapshot: Arc<super::status::Snapshot>,
    pub busy: Arc<AtomicBool>,
}
impl Context {
    fn current(&self, stop: &AtomicBool) -> Result<(), PreparationFailure> {
        if stop.load(Ordering::Acquire) {
            Err(PreparationFailure::Internal)
        } else {
            analysis::current(&self.epoch, self.expected)
        }
    }
    fn state(&self, state: PreparationState, error: Option<PreparationFailure>) {
        let mut snapshot = self.snapshot.lock().unwrap_or_else(|e| e.into_inner());
        if self.epoch.load(Ordering::Acquire) == self.expected
            && snapshot.state != PreparationState::Stopped
            && !self.busy.load(Ordering::Acquire)
        {
            snapshot.state = state;
            snapshot.error = error;
        }
    }
}
/// Drop releases the entire batch on any error, cancellation, partial scan or panic.
/// A crash leaves the lease to expire; no Processed acknowledgement precedes analysis.
struct Lease {
    detector: Arc<Detector>,
    id: Option<BatchId>,
}
impl Lease {
    fn acknowledge(&mut self) -> Result<u32, PreparationFailure> {
        let id = self
            .id
            .as_ref()
            .ok_or(PreparationFailure::Internal)?
            .clone();
        let count = self
            .detector
            .acknowledge(AcknowledgeEventsRequest {
                batch_id: id,
                disposition: EventDisposition::Processed,
            })
            .map_err(PreparationFailure::Detector)?
            .confirmed_count;
        self.id = None;
        Ok(count)
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(id) = self.id.take() {
            let _ = self
                .detector
                .release_events(ReleaseEventsRequest { batch_id: id });
        }
    }
}
fn run(
    context: Context,
    stop: Arc<AtomicBool>,
    wake: Arc<Wake>,
    started: tokio::sync::oneshot::Sender<Result<(), PreparationFailure>>,
) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let mut started = Some(started);
    let result = (|| {
        let runtime = runtime.map_err(|_| PreparationFailure::Internal)?;
        {
            let _control = context
                .control
                .lock()
                .map_err(|_| PreparationFailure::Internal)?;
            context.current(&stop)?;
            context
                .detector
                .resume(DetectorResumeRequest {})
                .map_err(PreparationFailure::Detector)?;
        }
        let _ = started.take().unwrap().send(Ok(()));
        while context.busy.load(Ordering::Acquire) {
            context.current(&stop)?;
            let version = wake.version();
            if context.busy.load(Ordering::Acquire) {
                wake.wait(version, &stop);
            }
        }
        consume(&runtime, &context, &stop, &wake)
    })();
    if let Some(started) = started {
        let _ = started.send(result.clone());
    }
    // Dropping a cancelled Scanner future requests cancellation of its finite
    // worker. Wait for that worker before allowing replacement preparation.
    while context.scanner.status().state == ScannerState::Running {
        std::thread::sleep(Duration::from_millis(5));
    }
    // Pausing delivery does not stop source observation or lose queued events.
    let _control = context.control.lock().unwrap_or_else(|e| e.into_inner());
    let _ = context.detector.pause(DetectorPauseRequest {});
    if let Err(error) = result {
        context.state(PreparationState::Failed, Some(error));
    }
}
fn consume(
    runtime: &tokio::runtime::Runtime,
    context: &Context,
    stop: &AtomicBool,
    wake: &Wake,
) -> Result<(), PreparationFailure> {
    loop {
        context.current(stop)?;
        let version = wake.version();
        let outcome = {
            let _control = context
                .control
                .lock()
                .map_err(|_| PreparationFailure::Internal)?;
            context.current(stop)?;
            // Bounded to the public queue maximum; five minutes covers normal
            // reconciliation scans. Expiry is an explicit safe failure, never an ack.
            let batch = context
                .detector
                .take_events(TakeEventsRequest {
                    max_events: 64,
                    lease_duration: Duration::from_secs(300),
                })
                .map_err(PreparationFailure::Detector)?
                .batch;
            if batch.files.is_empty() {
                None
            } else {
                context.state(PreparationState::ProcessingEvents, None);
                Some(runtime.block_on(process(context, stop, batch))?)
            }
        };
        match outcome {
            Some(true) => context.state(PreparationState::ReadyForAnalyzer, None),
            Some(false) => {
                context.state(PreparationState::Partial, None);
                return Ok(());
            }
            None => {
                if context.execution.snapshot().enabled {
                    runtime.block_on(analysis::retry_pending(
                        context.inventory.clone(),
                        context.identity.clone(),
                        context.epoch.clone(),
                        context.expected,
                        context.connector.clone(),
                        context.execution.clone(),
                    ))?;
                }
                context.state(PreparationState::ReadyForAnalyzer, None);
                wake.wait(version, stop);
            }
        }
    }
}
async fn process(
    context: &Context,
    stop: &AtomicBool,
    batch: FileBatch,
) -> Result<bool, PreparationFailure> {
    let mut lease = Lease {
        detector: context.detector.clone(),
        id: batch.batch_id,
    };
    let mut ids = BTreeSet::new();
    let mut scan_local = false;
    let mut all = false;
    for file in batch.files {
        context.current(stop)?;
        let ingested = ingestion::ingest(&context.inventory, &file, &context.identity)?;
        if let Some(id) = ingested.item {
            ids.insert(id);
        }
        let already_scanned = matches!(file.scope, crate::models::FileScope::WholeSync)
            && context.scanner.status().last_report_status == Some(ScanStatus::Completed)
            && context
                .scanner
                .status()
                .started_at
                .is_some_and(|time| time.unix_millis >= file.observed_at.unix_millis);
        scan_local |= ingested.scan_local && !already_scanned;
        all |= ingested.analyze_all;
    }
    if scan_local {
        let scan = context.scanner.scan(ScanRequest {
            sync_id: context.identity.sync_id.clone(),
        });
        tokio::pin!(scan);
        let report = loop {
            tokio::select! {
                report=&mut scan=>break report.map_err(PreparationFailure::Scanner)?,
                _=tokio::time::sleep(Duration::from_millis(100))=>context.current(stop)?,
            }
        };
        let complete = report.ready_for_analysis();
        if context.epoch.load(Ordering::Acquire) == context.expected {
            context
                .snapshot
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .report = Some(Arc::new(report));
        }
        if !complete {
            return Ok(false);
        }
        all = true;
    }
    context.current(stop)?;
    if all {
        analysis::analyze_all(
            context.inventory.clone(),
            context.identity.clone(),
            context.epoch.clone(),
            context.expected,
            context.connector.clone(),
            context.execution.clone(),
        )
        .await?;
    } else {
        analysis::analyze_selected(
            context.inventory.clone(),
            context.identity.clone(),
            context.epoch.clone(),
            context.expected,
            ids,
            context.connector.clone(),
            context.execution.clone(),
        )
        .await?;
    }
    context.current(stop)?;
    let count = lease.acknowledge()?;
    let mut snapshot = context.snapshot.lock().unwrap_or_else(|e| e.into_inner());
    if context.epoch.load(Ordering::Acquire) == context.expected {
        snapshot.events_processed = snapshot.events_processed.saturating_add(u64::from(count));
    }
    Ok(true)
}
