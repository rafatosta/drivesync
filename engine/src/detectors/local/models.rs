//! Collector contracts exported through the parent detector namespace.
use crate::models::{AccountId, LocalFile, LocalRelativePath, SyncId};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

/// Immutable operational binding; not a filesystem or logical item identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalDetectorScope {
    pub sync_id: SyncId,
    pub account_id: AccountId,
    pub local_root: PathBuf,
}

/// Read coverage, never proof of absence, identity or content integrity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalScanCoverage {
    Complete,
    Partial,
    Unavailable,
}

/// Shared detector errors; only collector-relevant variants are exercised here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalDetectorError {
    InvalidScope,
    InvalidState,
    RootUnavailable,
    PermissionDenied,
    PartialObservation,
    WatcherUnavailable,
    ResourceLimit,
    Busy,
    ModelInvalid,
    PreconditionFailed,
    Cancelled,
    NotificationFailed,
    Internal,
}

/// Planned scan counters, without implementing or scheduling a scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalScanResult {
    pub coverage: LocalScanCoverage,
    pub observed_count: u64,
    pub emitted_count: u64,
}

/// Sanitized failure; a single metadata collection has no scan progress.
#[derive(Debug, Clone)]
pub struct LocalDetectorFailure {
    pub kind: LocalDetectorError,
    pub message: String,
    pub retryable: bool,
    pub progress: Option<LocalScanResult>,
}
impl LocalDetectorFailure {
    /// Uses fixed diagnostics, never OS errors or user-controlled paths.
    pub(super) fn new(kind: LocalDetectorError) -> Self {
        use LocalDetectorError::*;
        let (message, retryable) = match kind {
            InvalidScope => ("invalid local scope", false),
            RootUnavailable => ("local root unavailable", true),
            PermissionDenied => ("local metadata access denied", true),
            PartialObservation => ("local observation requires recheck", true),
            ResourceLimit => ("local observation resource limit", true),
            ModelInvalid => ("invalid local metadata", false),
            Cancelled => ("local observation cancelled", false),
            InvalidState => ("invalid local detector state", false),
            WatcherUnavailable => ("local watcher unavailable", true),
            NotificationFailed => ("local event delivery failed", true),
            _ => ("local detector failure", false),
        };
        Self {
            kind,
            message: message.into(),
            retryable,
            progress: None,
        }
    }
}
impl std::fmt::Display for LocalDetectorFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for LocalDetectorFailure {}

/// Result of internal local observation operations.
pub type LocalDetectorResult<T> = Result<T, LocalDetectorFailure>;

/// Cloneable token for caller-controlled cooperative collection cancellation.
#[derive(Debug, Clone)]
pub struct LocalCancellation(Arc<AtomicBool>);
impl LocalCancellation {
    /// Creates an independent, uncancelled generation.
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }
    /// Cancels this generation irreversibly, including every existing clone.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    /// Reads the cancellation flag without I/O or side effects.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    /// Checks cancellation before starting another I/O operation.
    pub(super) fn check(&self) -> LocalDetectorResult<()> {
        if self.is_cancelled() {
            Err(LocalDetectorFailure::new(LocalDetectorError::Cancelled))
        } else {
            Ok(())
        }
    }
}

/// One validated entry path and its cooperative cancellation generation.
pub struct LocalMetadataRequest {
    pub relative_path: LocalRelativePath,
    pub cancellation: LocalCancellation,
}
/// Complete selected metadata, or explicit uncertainty; no parallel event list.
#[derive(Debug)]
pub struct LocalMetadataResult {
    pub file: Option<LocalFile>,
    pub coverage: LocalScanCoverage,
}
impl Default for LocalCancellation {
    fn default() -> Self {
        Self::new()
    }
}

/// Stable internal collection boundary independent of LocalFile's field layout.
#[allow(
    dead_code,
    reason = "The scope getter is retained for internal composition; production currently uses collect"
)]
pub(crate) trait LocalMetadataCollector: Send + Sync {
    /// Returns the original configuration without accessing the filesystem.
    fn scope(&self) -> LocalDetectorScope;
    /// Observes one entry without following symlinks or reading file contents.
    fn collect(&self, request: LocalMetadataRequest) -> LocalDetectorResult<LocalMetadataResult>;
}

/// Lifecycle of the local event producer, independent of remote observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalDetectorState {
    Stopped,
    Starting,
    Running,
    Degraded,
    Stopping,
    Failed,
}
/// Current activity; status reads this snapshot without filesystem access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalActivity {
    Inactive,
    Watching,
    Scanning,
    Recovering,
    Unavailable,
}
/// Start intent, without any promise of reconstructing the interval while stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalStartMode {
    Initial,
    Resume,
}
/// Reason for ending observation, not permission to discard accepted events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalStopReason {
    UserRequest,
    Shutdown,
    Unavailable,
}
/// Quiet interval for normalization: None uses 80 ms; zero disables settling.
/// The default maximum window is 500 ms, including ongoing writes as typed rechecks.
#[derive(Debug, Clone)]
pub struct LocalDetectorSettings {
    pub debounce_ms: Option<u64>,
    pub max_pending_paths: u32,
}
impl Default for LocalDetectorSettings {
    fn default() -> Self {
        Self {
            debounce_ms: None,
            max_pending_paths: 4096,
        }
    }
}
/// Single-event acceptance boundary; Ok means the receiver assumed durable responsibility.
/// Do not reenter blocking methods of the same detector from this callback.
pub trait LocalNotificationSink: Send + Sync {
    fn notify(&self, file: crate::models::File) -> LocalDetectorResult<()>;
}
/// Public local construction; physical collector composition remains private.
pub struct LocalDetectorCreateRequest {
    pub scope: LocalDetectorScope,
    pub notifications: Arc<dyn LocalNotificationSink>,
    pub settings: LocalDetectorSettings,
}
/// Created producer, with no watcher or disk traversal started.
pub struct LocalDetectorCreateResult {
    pub detector: super::runtime::LocalDetector,
    pub state: LocalDetectorState,
}
/// Start request timestamp is caller context, not a replacement for observation time.
pub struct LocalStartRequest {
    pub mode: LocalStartMode,
    pub requested_at: crate::models::Timestamp,
}
/// Accepted start; coverage verification may still be running.
#[derive(Debug)]
pub struct LocalStartResult {
    pub state: LocalDetectorState,
    pub scheduled_scan: bool,
}
/// Stop request; no event acknowledgement or queue deletion is performed.
pub struct LocalStopRequest {
    pub reason: LocalStopReason,
    pub requested_at: crate::models::Timestamp,
}
/// Successful stop guarantees zero in-flight callbacks from the stopped generation.
#[derive(Debug)]
pub struct LocalStopResult {
    pub state: LocalDetectorState,
    pub in_flight: u32,
}
/// Snapshot query without I/O.
pub struct LocalStatusRequest {}
/// Operational snapshot. Counters describe the active observation round only.
#[derive(Debug, Clone)]
pub struct LocalStatusResult {
    pub state: LocalDetectorState,
    pub activity: LocalActivity,
    pub observed_in_current_round: u64,
    pub emitted_in_current_round: u64,
    pub last_observation_at: Option<crate::models::Timestamp>,
    pub last_coverage: Option<LocalScanCoverage>,
    pub last_error: Option<LocalDetectorError>,
}
