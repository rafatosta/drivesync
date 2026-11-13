//! Remote lifecycle, scheduling and diagnostics without provider credentials.
use crate::{detectors::QueueStats, models::*};
use std::path::PathBuf;
use std::time::Duration;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteDetectorScope {
    pub account_id: AccountId,
    pub sync_id: SyncId,
    pub remote_root_id: String,
}
#[derive(Debug, Clone)]
pub struct RemoteDetectorSettings {
    pub polling_interval: Duration,
    pub page_budget: u32,
}
impl Default for RemoteDetectorSettings {
    fn default() -> Self {
        Self {
            polling_interval: Duration::from_secs(30),
            page_budget: 20,
        }
    }
}
/// Construction performs private storage I/O but never network or OAuth interaction.
pub struct RemoteDetectorCreateRequest {
    pub connector: crate::Engine,
    pub scope: RemoteDetectorScope,
    pub settings: RemoteDetectorSettings,
    pub data_directory: PathBuf,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteState {
    Stopped,
    Starting,
    Running,
    Degraded,
    Stopping,
    Failed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteActivity {
    Inactive,
    Polling,
    Waiting,
    Recovering,
    Unavailable,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemotePollMode {
    Initial,
    Incremental,
    Recovery,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteCoverage {
    Complete,
    Partial,
    Unavailable,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RemoteError {
    InvalidScope,
    InvalidState,
    RemoteUnavailable,
    PermissionDenied,
    AuthorizationRequired,
    CursorExpired,
    PageIncomplete,
    SinkUnavailable,
    CheckpointUnavailable,
    Cancelled,
    Internal,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFailure {
    pub kind: RemoteError,
    pub retryable: bool,
    pub retry_after: Option<Duration>,
}
impl RemoteFailure {
    pub(crate) fn new(kind: RemoteError) -> Self {
        Self {
            kind,
            retryable: matches!(
                kind,
                RemoteError::RemoteUnavailable
                    | RemoteError::PageIncomplete
                    | RemoteError::SinkUnavailable
                    | RemoteError::CheckpointUnavailable
            ),
            retry_after: None,
        }
    }
}
impl std::fmt::Display for RemoteFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Remote detector: {:?}", self.kind)
    }
}
impl std::error::Error for RemoteFailure {}
pub type RemoteResult<T> = Result<T, RemoteFailure>;
pub struct RemoteStartRequest {
    pub mode: RemotePollMode,
}
#[derive(Debug)]
pub struct RemoteStartResult {
    pub state: RemoteState,
    pub scheduled: bool,
}
pub struct RemotePollRequest {
    pub mode: RemotePollMode,
    pub page_budget: Option<u32>,
}
#[derive(Debug, Clone)]
pub struct RemotePollResult {
    pub coverage: RemoteCoverage,
    pub observed: u64,
    pub emitted: u64,
    pub has_more: bool,
    /// Sanitized fingerprint; raw provider cursors stay private.
    pub checkpoint_confirmed: Option<String>,
}
pub struct RemoteStopRequest {}
#[derive(Debug)]
pub struct RemoteStopResult {
    pub state: RemoteState,
}
pub struct RemoteStatusRequest {}
#[derive(Debug, Clone)]
pub struct RemoteStatusResult {
    pub state: RemoteState,
    pub activity: RemoteActivity,
    pub coverage: RemoteCoverage,
    pub polling_interval: Duration,
    pub observed_in_current_round: u64,
    pub emitted_in_current_round: u64,
    pub last_observed_at: Option<Timestamp>,
    pub last_poll_started_at: Option<Timestamp>,
    pub next_poll_at: Option<Timestamp>,
    pub last_poll_duration: Option<Duration>,
    pub cursor: Option<String>,
    pub consecutive_failures: u32,
    pub recovery_attempts: u64,
    pub last_error: Option<RemoteError>,
}
/// Bounded event context for inspection, without provider payloads or file contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteEventSummary {
    pub id: FileId,
    pub observed_at: Timestamp,
    pub event: FileEvent,
    pub scope: FileScope,
    pub entry_kind: Option<RemoteEntryKind>,
}
#[derive(Debug, Clone)]
pub struct RemoteDiagnostics {
    pub status: RemoteStatusResult,
    pub queue: QueueStats,
    pub events: Vec<RemoteEventSummary>,
}
