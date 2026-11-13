//! Local capture backed by a durable queue. Remote observation remains unimplemented.
use super::{storage::Store, *};
use crate::models::File;
use std::{path::PathBuf, sync::Arc};
/// App-private base directory and local scope; database names are generated internally.
pub struct PersistentDetectorRequest {
    pub scope: LocalDetectorScope,
    pub settings: LocalDetectorSettings,
    pub data_directory: PathBuf,
}
/// Persistent local detector with explicit reservation/acknowledgement delivery.
/// Source stop and delivery pause are independent. Drop ends capture, retaining the queue.
pub struct PersistentDetector {
    local: LocalDetector,
    store: Arc<Store>,
}
struct Noop;
impl LocalNotificationSink for Noop {
    fn notify(&self, _: File) -> LocalDetectorResult<()> {
        Ok(())
    }
}
struct QueueSink {
    store: Arc<Store>,
    observer: Arc<dyn LocalNotificationSink>,
}
impl LocalNotificationSink for QueueSink {
    fn notify(&self, file: File) -> LocalDetectorResult<()> {
        self.store
            .enqueue(&file)
            .map_err(|error| LocalDetectorFailure {
                kind: LocalDetectorError::NotificationFailed,
                message: "Durable event queue did not accept the occurrence".into(),
                retryable: error.retryable,
                progress: None,
            })?;
        // Observer failures cannot undo a committed event; retries remain idempotent.
        self.observer.notify(file)
    }
}
impl PersistentDetector {
    /// Opens and validates private SQLite state; run outside the UI thread.
    /// A new queue starts paused; reopens restore its persisted delivery policy.
    /// Capture remains stopped. Never replaces invalid storage.
    pub fn open(request: PersistentDetectorRequest) -> QueueResult<Self> {
        Self::open_with_observer(request, Arc::new(Noop))
    }
    pub(crate) fn open_with_observer(
        request: PersistentDetectorRequest,
        observer: Arc<dyn LocalNotificationSink>,
    ) -> QueueResult<Self> {
        // Validate scope/settings using the existing pure constructor before any storage I/O.
        LocalDetector::new(LocalDetectorCreateRequest {
            scope: request.scope.clone(),
            settings: request.settings.clone(),
            notifications: Arc::new(Noop),
        })
        .map_err(|_| QueueFailure::new(QueueError::InvalidRequest))?;
        let store = Arc::new(Store::open(&request.data_directory, &request.scope)?);
        let local = LocalDetector::new(LocalDetectorCreateRequest {
            scope: request.scope,
            settings: request.settings,
            notifications: Arc::new(QueueSink {
                store: store.clone(),
                observer,
            }),
        })
        .map_err(|_| QueueFailure::new(QueueError::InvalidRequest))?
        .detector;
        Ok(Self { local, store })
    }
    pub fn start(&self, request: LocalStartRequest) -> LocalDetectorResult<LocalStartResult> {
        self.local.start(request)
    }
    /// Stops only physical observation. Delivery policy and committed records survive.
    pub fn stop(&self, request: LocalStopRequest) -> LocalDetectorResult<LocalStopResult> {
        self.local.stop(request)
    }
    pub fn status(&self, request: LocalStatusRequest) -> LocalDetectorResult<LocalStatusResult> {
        self.local.status(request)
    }
    /// Prevents new claims atomically; earlier reservations may still be acknowledged/released.
    pub fn pause(&self) -> QueueResult<()> {
        self.store.set_delivery(DeliveryState::Paused)
    }
    /// Persists enabled delivery. This performs SQLite I/O; run off the UI thread.
    pub fn resume(&self) -> QueueResult<()> {
        self.store.set_delivery(DeliveryState::Enabled)
    }
    /// Cached delivery policy; refresh_queue observes changes made by another process.
    pub fn delivery(&self) -> DeliveryState {
        self.store.snapshot().delivery
    }
    pub(crate) fn queue_is_new(&self) -> bool {
        self.store.created
    }
    /// Synchronous SQLite operation; claim does not consume or confirm a File.
    pub fn take_events(&self, request: TakeEventsRequest) -> QueueResult<FileBatch> {
        self.store.claim(request)
    }
    /// Confirms a valid lease atomically and idempotently; this is not sync success.
    /// Expired/foreign leases never confirm events. Performs SQLite I/O.
    pub fn acknowledge(&self, batch: &BatchId, disposition: EventDisposition) -> QueueResult<u32> {
        self.store.acknowledge(batch, disposition)
    }
    /// Returns a valid lease to pending order without modifying its events.
    /// Repeating a successful release is idempotent. Performs SQLite I/O.
    pub fn release_events(&self, batch: &BatchId) -> QueueResult<u32> {
        self.store.release(batch)
    }
    /// Cached counts only; this getter performs no filesystem or SQLite I/O.
    pub fn queue_status(&self) -> QueueStats {
        self.store.snapshot()
    }
    /// Refreshes counts from SQLite, including expiry and commits by another process.
    pub fn refresh_queue(&self) -> QueueResult<()> {
        self.store.refresh()
    }
    pub(crate) fn recent_events(&self) -> QueueResult<Vec<File>> {
        self.store.recent()
    }
}
