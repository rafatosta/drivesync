//! Public unified detector contracts. Sources and storage are composed privately.
use crate::{detectors::*, models::*, Engine};
use std::{path::PathBuf, sync::Arc};
#[derive(Debug, Clone)]
pub struct DetectorScope {
    pub account_id: AccountId,
    pub sync_id: SyncId,
    pub local_root: PathBuf,
    pub remote_root_id: String,
}
pub struct DetectorCreateRequest {
    pub connector: Engine,
    pub scope: DetectorScope,
    pub local_settings: LocalDetectorSettings,
    pub remote_settings: RemoteDetectorSettings,
    pub data_directory: PathBuf,
    pub event_available_sink: Option<Arc<dyn EventAvailableSink>>,
}
/// Emits a lightweight availability signal. Do not reenter blocking control methods
/// of the same Detector; notification is neither delivery nor acknowledgement.
pub trait EventAvailableSink: Send + Sync {
    fn notify_events_available(&self, sync_id: SyncId);
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectorState {
    Stopped,
    Starting,
    Running,
    Degraded,
    Stopping,
    Failed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectorStartMode {
    Initial,
    Restart,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectorStopMode {
    DrainInFlight,
    StopNow,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectorError {
    InvalidState,
    InvalidScope,
    SourceUnavailable,
    StoreUnavailable,
    DeliveryPaused,
    InvalidBatch,
    LeaseExpired,
    Cancelled,
    Internal,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectorSourceError {
    Local(LocalDetectorError),
    Remote(RemoteError),
}
#[derive(Debug, Clone)]
pub struct DetectorFailure {
    pub kind: DetectorError,
    pub retryable: bool,
    pub source: Option<DetectorSourceError>,
    pub queue: Option<QueueError>,
}
impl DetectorFailure {
    pub(super) fn new(kind: DetectorError) -> Self {
        Self {
            kind,
            retryable: false,
            source: None,
            queue: None,
        }
    }
}
impl std::fmt::Display for DetectorFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Detector: {:?}", self.kind)
    }
}
impl std::error::Error for DetectorFailure {}
impl From<QueueFailure> for DetectorFailure {
    fn from(error: QueueFailure) -> Self {
        Self {
            kind: match error.kind {
                QueueError::DeliveryPaused => DetectorError::DeliveryPaused,
                QueueError::LeaseExpired => DetectorError::LeaseExpired,
                QueueError::InvalidLease => DetectorError::InvalidBatch,
                QueueError::InvalidRequest | QueueError::IdentityMismatch => {
                    DetectorError::InvalidScope
                }
                _ => DetectorError::StoreUnavailable,
            },
            retryable: error.retryable,
            source: None,
            queue: Some(error.kind),
        }
    }
}
pub type DetectorResult<T> = Result<T, DetectorFailure>;
pub struct DetectorCreateResult {
    pub detector: super::Detector,
    pub state: DetectorState,
}
pub struct DetectorStartRequest {
    pub mode: DetectorStartMode,
    pub delivery: DeliveryState,
}
#[derive(Debug)]
pub struct DetectorStartResult {
    pub state: DetectorState,
}
pub struct DetectorPauseRequest {}
#[derive(Debug)]
pub struct DetectorPauseResult {
    pub delivery: DeliveryState,
}
pub struct DetectorResumeRequest {}
#[derive(Debug)]
pub struct DetectorResumeResult {
    pub delivery: DeliveryState,
}
pub struct DetectorStopRequest {
    pub mode: DetectorStopMode,
}
#[derive(Debug)]
pub struct DetectorStopResult {
    pub state: DetectorState,
    pub in_flight: u32,
}
pub struct DetectorStatusRequest {}
#[derive(Debug, Clone)]
pub struct DetectorSourceStatus {
    pub state: DetectorState,
    pub last_error: Option<DetectorSourceError>,
    pub local: Option<LocalStatusResult>,
    pub remote: Option<RemoteStatusResult>,
}
#[derive(Debug, Clone)]
pub struct DetectorStatus {
    pub state: DetectorState,
    pub delivery: DeliveryState,
    pub local: DetectorSourceStatus,
    pub remote: DetectorSourceStatus,
    pub queue: QueueStats,
}
#[derive(Debug)]
pub struct DetectorStatusResult {
    pub status: DetectorStatus,
}
#[derive(Clone)]
pub enum DetectorScanScope {
    All,
    Scopes(Vec<FileScope>),
}
#[derive(Clone, Copy)]
pub enum DetectorScanReason {
    Manual,
    Recovery,
    ConsistencyCheck,
}
#[derive(Clone)]
pub struct DetectorScanRequest {
    pub scope: DetectorScanScope,
    pub reason: DetectorScanReason,
    pub request_id: Option<String>,
}
#[derive(Debug)]
pub struct DetectorScanResult {
    pub accepted: bool,
    pub correlation_id: Option<String>,
}
#[derive(Debug)]
pub struct TakeEventsResult {
    pub batch: FileBatch,
}
pub struct AcknowledgeEventsRequest {
    pub batch_id: BatchId,
    pub disposition: EventDisposition,
}
#[derive(Debug)]
pub struct AcknowledgeEventsResult {
    pub confirmed_count: u32,
}
pub struct ReleaseEventsRequest {
    pub batch_id: BatchId,
}
#[derive(Debug)]
pub struct ReleaseEventsResult {
    pub released_count: u32,
}
