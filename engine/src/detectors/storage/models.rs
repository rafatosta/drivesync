//! Public queue contracts; persistence paths, connections and schema stay private.
use crate::models::{File, Timestamp};
use std::time::Duration;
/// Sanitized durable-queue errors, distinct from local filesystem capture failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueError {
    IdentityMismatch,
    Unavailable,
    Corrupt,
    SchemaUnsupported,
    Conflict,
    LeaseExpired,
    InvalidLease,
    InvalidRequest,
    CapacityExceeded,
    DeliveryPaused,
    Internal,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueFailure {
    pub kind: QueueError,
    pub retryable: bool,
}
impl QueueFailure {
    pub(crate) fn new(kind: QueueError) -> Self {
        Self {
            kind,
            retryable: matches!(kind, QueueError::Unavailable | QueueError::CapacityExceeded),
        }
    }
}
impl std::fmt::Display for QueueFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Detector queue: {:?}", self.kind)
    }
}
impl std::error::Error for QueueFailure {}
pub type QueueResult<T> = Result<T, QueueFailure>;
/// Opaque reservation capability; its identity is scoped to one queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchId(pub(crate) String);
/// Full events in durable insertion order. Empty batches have no reservation.
#[derive(Debug)]
pub struct FileBatch {
    pub batch_id: Option<BatchId>,
    pub files: Vec<File>,
    pub first_observed_at: Option<Timestamp>,
    pub last_observed_at: Option<Timestamp>,
}
impl FileBatch {
    pub(crate) fn new(batch_id: Option<BatchId>, files: Vec<File>) -> Self {
        let first_observed_at = files
            .iter()
            .map(|file| file.observed_at.unix_millis)
            .min()
            .map(|unix_millis| Timestamp { unix_millis });
        let last_observed_at = files
            .iter()
            .map(|file| file.observed_at.unix_millis)
            .max()
            .map(|unix_millis| Timestamp { unix_millis });
        Self {
            batch_id,
            files,
            first_observed_at,
            last_observed_at,
        }
    }
}
/// Successful processing is explicit; discard is never an automatic error policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventDisposition {
    Processed,
    Discarded,
}
/// Delivery suspension does not stop filesystem observation or durable enqueue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeliveryState {
    Enabled,
    #[default]
    Paused,
}
/// Snapshot counts. Confirmed includes explicit processed/discarded events, not sync success.
#[derive(Debug, Clone, Default)]
pub struct QueueStats {
    pub pending: u64,
    pub leased: u64,
    pub confirmed: u64,
    pub detected: u64,
    pub initial_observations: u64,
    pub changes_detected: u64,
    pub last_error: Option<QueueError>,
    pub delivery: DeliveryState,
}
pub struct TakeEventsRequest {
    pub max_events: u32,
    pub lease_duration: Duration,
}

/// Provenance is assigned by the owning producer sink, including scope-only warnings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectorOrigin {
    Local,
    Remote,
}
