//! Local scan requests, truthful coverage and sanitized diagnostics.
use crate::{
    inventory::{InventoryManager, SyncItemId},
    models::{CausalOrderSource, LocalRelativePath, SyncId, Timestamp},
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
pub type ScannerResult<T> = Result<T, ScannerError>;
/// A configured Inventory and the exact authorized local scope; no detector or provider dependency.
pub struct ScannerConfig {
    pub inventory: Arc<InventoryManager>,
    pub root: LocalRoot,
    /// Shared with live detector events so scan observations cannot race past them.
    pub order_source: Arc<dyn CausalOrderSource>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalRoot {
    pub sync_id: SyncId,
    pub path: PathBuf,
}
#[derive(Debug, Clone)]
pub struct ScanRequest {
    pub sync_id: SyncId,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScannerState {
    Idle,
    Running,
    Completed,
    Failed,
    Cancelled,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScannerStatus {
    pub state: ScannerState,
    pub started_at: Option<Timestamp>,
    pub finished_at: Option<Timestamp>,
    pub files_observed: u64,
    pub directories_observed: u64,
    pub items_recorded: u64,
    pub issues_recorded: u64,
    pub current_directory: Option<LocalRelativePath>,
    pub last_report_status: Option<ScanStatus>,
    pub last_error: Option<ScannerErrorKind>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScanStatus {
    Completed,
    Partial,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanReport {
    pub sync_id: SyncId,
    pub status: ScanStatus,
    pub files_observed: u64,
    pub directories_observed: u64,
    pub items_recorded: u64,
    pub issues: Vec<ScanIssue>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanIssue {
    pub location: ScanIssueLocation,
    pub kind: ScanIssueKind,
    pub impact: ScanIssueImpact,
    pub message: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScanIssueLocation {
    Item(SyncItemId),
    RelativePath(LocalRelativePath),
    ScanRoot,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScanIssueKind {
    AccessDenied,
    ReadFailed,
    MetadataUnavailable,
    EntryDisappeared,
    EntryChanged,
    UnsupportedEntryType,
    SymlinkProblem,
    ResourceExhausted,
    InventoryRecordFailed,
    MountBoundary,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScanIssueImpact {
    ItemIncomplete,
    SubtreeUnobserved,
    ObservationNotPersisted,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScannerErrorKind {
    InvalidConfiguration,
    AlreadyRunning,
    WrongSyncScope,
    RootNotFound,
    RootInaccessible,
    StorageUnavailable,
    InventoryUnavailable,
    InventoryUnusable,
    Cancelled,
    Internal,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScannerError {
    pub kind: ScannerErrorKind,
    pub message: String,
}
impl ScannerError {
    pub(super) fn new(kind: ScannerErrorKind) -> Self {
        Self {
            kind,
            message: "local scan could not complete".into(),
        }
    }
}
impl std::fmt::Display for ScannerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.kind)
    }
}
impl std::error::Error for ScannerError {}
impl Default for ScannerStatus {
    fn default() -> Self {
        Self {
            state: ScannerState::Idle,
            started_at: None,
            finished_at: None,
            files_observed: 0,
            directories_observed: 0,
            items_recorded: 0,
            issues_recorded: 0,
            current_directory: None,
            last_report_status: None,
            last_error: None,
        }
    }
}
impl ScanReport {
    /// Only complete coverage/persistence may release analysis. Item issues alone need not block it.
    /// This is a readiness result, never a command to resume a diagnostic detector consumer.
    pub fn ready_for_analysis(&self) -> bool {
        self.status == ScanStatus::Completed
    }
}
