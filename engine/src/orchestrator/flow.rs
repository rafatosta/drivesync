//! Continuous observation ingestion. Detector owns the queue; Analyzer owns decisions.
//! This worker never claims Inventory operations or executes physical synchronization.
use super::{analysis, ingestion, PreparationFailure, PreparationState};
use crate::{
    detectors::*,
    inventory::*,
    models::{File as Observation, FileData, FileLocation, FileScope, SyncId},
    synchronization::{
        SynchronizationAction, SynchronizationActivity, SynchronizationEvent, SynchronizationItem,
        SynchronizationLocation, SynchronizationProgress, SynchronizationProgressUnit,
        SynchronizationState, SynchronizationWork, SynchronizationWorkDetail,
    },
};
use std::os::unix::ffi::OsStringExt;
use std::{
    collections::BTreeSet,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Condvar, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};
const EVENT_LEASE_DURATION: Duration = Duration::from_secs(300);
const EVENT_LEASE_RENEWAL_INTERVAL: Duration = Duration::from_secs(60);

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
        let changed = if self.epoch.load(Ordering::Acquire) == self.expected
            && !matches!(
                snapshot.state,
                PreparationState::Stopped | PreparationState::Cancelled
            )
            && !self.busy.load(Ordering::Acquire)
        {
            snapshot.state = state;
            snapshot.error = error;
            true
        } else {
            false
        };
        drop(snapshot);
        if changed {
            self.publish_sources();
        }
    }
    fn publish_sources(&self) {
        match self.detector.status(DetectorStatusRequest {}) {
            Ok(result) => self.snapshot.publish_detector(&result.status),
            Err(error) => self.snapshot.update_status(|status| {
                status.issue = Some(super::status::detector_issue(&error));
                if status.state == SynchronizationState::Waiting {
                    status.work = Some(SynchronizationWork {
                        action: SynchronizationAction::Reconcile,
                        item: None,
                        location: None,
                        detail: SynchronizationWorkDetail::Waiting(
                            crate::synchronization::SynchronizationWaitReason::SourceRecovery,
                        ),
                        event: None,
                    });
                }
            }),
        }
    }
    async fn await_with_source_updates<T>(
        &self,
        future: impl std::future::Future<Output = T>,
    ) -> T {
        tokio::pin!(future);
        loop {
            tokio::select! {
                result = &mut future => return result,
                _ = tokio::time::sleep(Duration::from_millis(100)) => self.publish_sources(),
            }
        }
    }
    fn publish_event(&self, file: &Observation, processed: u64, total: u64) {
        let item = observation_item(file);
        let event = SynchronizationEvent {
            kind: file.event.kind,
            previous: file.event.previous.clone(),
            current: file.event.current.clone(),
            observed_at: file.observed_at,
        };
        let location = file
            .event
            .current
            .as_ref()
            .map(observation_location)
            .or_else(|| {
                item.as_ref()
                    .and_then(|item| item.local_path.clone().map(SynchronizationLocation::Local))
            });
        let action = match &file.scope {
            FileScope::Local(_) | FileScope::WholeSync => SynchronizationAction::ObserveLocal,
            FileScope::Remote(_) => SynchronizationAction::ObserveRemote,
        };
        self.snapshot.update_status(|status| {
            status.state = SynchronizationState::Running;
            status.activity = SynchronizationActivity::ProcessingEvents;
            status.work = Some(SynchronizationWork {
                action,
                item,
                location,
                detail: SynchronizationWorkDetail::Preparation,
                event: Some(event),
            });
            status.progress = Some(SynchronizationProgress {
                completed: processed,
                total: Some(total),
                unit: SynchronizationProgressUnit::Events,
                phase: None,
            });
            status.counters.events_processed = processed;
            status.counters.events_total = Some(total);
        });
    }
}
/// Drop releases the entire batch on any error, cancellation or panic.
/// A crash leaves the lease to expire; no Processed acknowledgement precedes analysis.
struct Lease {
    detector: Arc<Detector>,
    id: Option<BatchId>,
}
impl Lease {
    fn renew(&self) -> Result<(), PreparationFailure> {
        self.detector
            .renew_events(
                self.id.as_ref().ok_or(PreparationFailure::Internal)?,
                EVENT_LEASE_DURATION,
            )
            .map_err(PreparationFailure::Detector)
    }
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
            context.publish_sources();
        }
        let _ = started.take().unwrap().send(Ok(()));
        while context.busy.load(Ordering::Acquire) {
            context.current(&stop)?;
            let version = wake.version();
            if context.busy.load(Ordering::Acquire) {
                wake.wait(version, &stop);
                context.publish_sources();
            }
        }
        consume(&runtime, &context, &stop, &wake)
    })();
    if let Some(started) = started {
        let _ = started.send(result.clone());
    }
    // Pausing delivery does not stop source observation or lose queued events.
    let _control = context.control.lock().unwrap_or_else(|e| e.into_inner());
    let _ = context.detector.pause(DetectorPauseRequest {});
    context.publish_sources();
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
        context.publish_sources();
        let version = wake.version();
        let outcome = {
            let _control = context
                .control
                .lock()
                .map_err(|_| PreparationFailure::Internal)?;
            context.current(stop)?;
            // The batch is renewed while its observations and resulting operations run.
            let batch = context
                .detector
                .take_events(TakeEventsRequest {
                    max_events: 64,
                    lease_duration: EVENT_LEASE_DURATION,
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
            Some(()) => context.state(PreparationState::ReadyForAnalyzer, None),
            None => {
                if context.execution.snapshot().enabled {
                    runtime.block_on(context.await_with_source_updates(
                        analysis::retry_pending(
                            context.inventory.clone(),
                            context.identity.clone(),
                            context.epoch.clone(),
                            context.expected,
                            context.connector.clone(),
                            context.execution.clone(),
                        ),
                    ))?;
                }
                context.state(PreparationState::ReadyForAnalyzer, None);
                if !context.execution.snapshot().prerequisite_retries.is_empty() {
                    context.snapshot.update_status(|status| {
                        if let Some(work) = &mut status.work {
                            work.detail = SynchronizationWorkDetail::Waiting(
                                crate::synchronization::SynchronizationWaitReason::RetryBackoff,
                            );
                        }
                    });
                }
                wake.wait(version, stop);
                context.publish_sources();
            }
        }
    }
}
async fn process(
    context: &Context,
    stop: &AtomicBool,
    batch: FileBatch,
) -> Result<(), PreparationFailure> {
    let mut lease = Lease {
        detector: context.detector.clone(),
        id: batch.batch_id,
    };
    let processing = process_files(context, stop, batch.files);
    tokio::pin!(processing);
    loop {
        tokio::select! {
            result = &mut processing => break result?,
            _ = tokio::time::sleep(EVENT_LEASE_RENEWAL_INTERVAL) => {
                context.current(stop)?;
                lease.renew()?;
            },
        }
    }
    context.current(stop)?;
    let count = lease.acknowledge()?;
    let mut snapshot = context.snapshot.lock().unwrap_or_else(|e| e.into_inner());
    if context.epoch.load(Ordering::Acquire) == context.expected {
        snapshot.events_processed = snapshot.events_processed.saturating_add(u64::from(count));
    }
    Ok(())
}

async fn process_files(
    context: &Context,
    stop: &AtomicBool,
    files: Vec<Observation>,
) -> Result<(), PreparationFailure> {
    let mut ids = BTreeSet::new();
    let mut all = false;
    let total = files.len() as u64;
    for (index, file) in files.into_iter().enumerate() {
        context.current(stop)?;
        context.publish_event(&file, index as u64, total);
        let ingested = ingestion::ingest(&context.inventory, &file, &context.identity)?;
        context.publish_event(&file, index as u64 + 1, total);
        if let Some(id) = ingested.item {
            ids.insert(id);
        }
        all |= ingested.analyze_all;
    }
    context.current(stop)?;
    if all {
        context
            .await_with_source_updates(analysis::analyze_all(
                context.inventory.clone(),
                context.identity.clone(),
                context.epoch.clone(),
                context.expected,
                context.connector.clone(),
                context.execution.clone(),
            ))
            .await?;
    } else {
        context
            .await_with_source_updates(analysis::analyze_selected(
                context.inventory.clone(),
                context.identity.clone(),
                context.epoch.clone(),
                context.expected,
                ids,
                context.connector.clone(),
                context.execution.clone(),
            ))
            .await?;
    }
    context.current(stop)?;
    Ok(())
}

fn observation_location(location: &FileLocation) -> SynchronizationLocation {
    match location {
        FileLocation::Local(path) => SynchronizationLocation::Local(path.clone()),
        FileLocation::Remote {
            reference,
            parent_id,
            name,
        } => SynchronizationLocation::Remote {
            reference: reference.clone(),
            parent_id: parent_id.clone(),
            name: name.clone(),
        },
    }
}

fn observation_item(file: &Observation) -> Option<SynchronizationItem> {
    let mut item = SynchronizationItem {
        id: None,
        kind: None,
        local_name: None,
        remote_name: None,
        local_path: None,
        remote_reference: None,
    };
    if let Some(FileData::Local(local)) = &file.data {
        item.local_path = Some(local.relative_path.clone());
        let name = local
            .relative_path
            .as_bytes()
            .rsplit(|byte| *byte == b'/')
            .next()?;
        item.local_name = Some(std::ffi::OsString::from_vec(name.to_vec()));
        item.kind = match local.kind {
            crate::models::MetadataValue::Known(crate::models::LocalEntryKind::Directory) => {
                Some(ItemKind::Directory)
            }
            crate::models::MetadataValue::Known(crate::models::LocalEntryKind::Regular) => {
                Some(ItemKind::File)
            }
            crate::models::MetadataValue::Known(crate::models::LocalEntryKind::Symlink) => {
                Some(ItemKind::Symlink)
            }
            _ => None,
        };
    }
    if let Some(FileData::Remote(remote)) = &file.data {
        item.remote_reference = Some(remote.reference.clone());
        item.remote_name = match &remote.name {
            crate::models::MetadataValue::Known(name) => Some(name.clone()),
            _ => None,
        };
    }
    match file.event.current.as_ref().or(file.event.previous.as_ref()) {
        Some(FileLocation::Local(path)) => {
            item.local_path.get_or_insert_with(|| path.clone());
            if item.local_name.is_none() {
                if let Some(name) = path.as_bytes().rsplit(|byte| *byte == b'/').next() {
                    item.local_name = Some(std::ffi::OsString::from_vec(name.to_vec()));
                }
            }
        }
        Some(FileLocation::Remote {
            reference, name, ..
        }) => {
            item.remote_reference
                .get_or_insert_with(|| reference.clone());
            if item.remote_name.is_none() {
                item.remote_name = name.clone();
            }
        }
        None => {}
    }
    if item.local_path.is_none() && item.remote_reference.is_none() {
        None
    } else {
        Some(item)
    }
}
