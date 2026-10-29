//! Transfer results and durable evidence, independent of GTK.
use crate::{EngineError, InventoryEntry};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Per-item failures preserve local data and allow independent items to proceed.
#[derive(Debug, Clone)]
pub struct DownloadIssue {
    pub relative_path: Option<PathBuf>,
    pub error: EngineError,
}
/// Informational record of a previously observed remote path. A missing current
/// path means it is no longer represented in the selected root, not proof of
/// deletion. Download-only synchronization never removes the previous local copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviousPath {
    pub previous_path: Option<PathBuf>,
    pub current_path: Option<PathBuf>,
}
/// A completed download cycle. Issues/skipped items are not synchronization success.
#[derive(Debug, Clone, Default)]
pub struct DownloadReport {
    pub downloaded: usize,
    pub uploaded: usize,
    pub created_remote_folders: usize,
    pub updated: usize,
    /// Relative paths to prior versions retained beside updated files.
    pub preserved_copies: Vec<PathBuf>,
    pub recovered: usize,
    pub unchanged: usize,
    pub directories: usize,
    /// Workspace browser shortcuts made available in the local tree.
    pub linked: usize,
    pub skipped: usize,
    pub issues: Vec<DownloadIssue>,
    /// Historical path activity, independent of actionable download failures.
    pub previous_paths: Vec<PreviousPath>,
    /// Pre-execution observations; these are not confirmation that planned work ran.
    pub reconciliation: Vec<crate::ReconciliationEntry>,
}
/// Current item stage. Receiving bytes, installing binaries, and linking remote
/// Workspace documents are reported as separate operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadStage {
    Checking,
    Downloading,
    Uploading,
    Verifying,
    Installing,
    Linking,
}
/// Item-local details; byte totals never describe the complete cycle.
#[derive(Debug, Clone)]
pub struct DownloadItemProgress {
    pub relative_path: PathBuf,
    pub stage: DownloadStage,
    pub received: u64,
    /// Binary byte size; None for folders or unsupported items.
    pub total: Option<u64>,
}
/// Observable preparation work before the complete transfer plan is known.
/// Counters are observations, never percentages or confirmed downloads.
#[derive(Debug, Clone)]
pub enum PreparationProgress {
    Connecting,
    ReadingChanges,
    Scanning {
        files: usize,
        folders: usize,
        current_path: PathBuf,
    },
    CheckingLocal {
        checked: usize,
    },
    SavingState,
    ScanningUploads,
    PreparingUpload {
        current_path: PathBuf,
    },
}
/// Cycle progress counts processed inventory items, including folders, skipped
/// items and failures. It is not a measure of successful installation or time.
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub preparation: Option<PreparationProgress>,
    pub completed_items: usize,
    pub total_items: usize,
    pub current: Option<DownloadItemProgress>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Identity {
    pub device: u64,
    pub inode: u64,
}
impl Identity {
    /// Binds a content hash to the installed file's stable local identity.
    pub(crate) fn fingerprint(&self, sha256: &str) -> String {
        format!("{sha256}:{}:{}", self.device, self.inode)
    }
}
/// Verified prior installation used only to authorize a managed-file update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Previous {
    pub identity: Identity,
    pub sha256: String,
    pub size: u64,
    pub version: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Phase {
    Started,
    Ready {
        identity: Identity,
        sha256: String,
    },
    Installed {
        identity: Identity,
        sha256: String,
    },
    Updating {
        previous: Previous,
    },
    UpdateReady {
        identity: Identity,
        sha256: String,
        previous: Previous,
        backup: String,
    },
    Updated {
        identity: Identity,
        sha256: String,
        previous: Previous,
        backup: String,
    },
    UpdateAbandoned {
        backup: String,
    },
}
/// Persisted before effects; Ready carries evidence for replay after rename.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Operation {
    pub id: String,
    pub item: InventoryEntry,
    pub relative: PathBuf,
    pub temporary: String,
    pub root: Identity,
    pub parent: Identity,
    pub phase: Phase,
}

impl DownloadProgress {
    pub(crate) fn preparing(stage: PreparationProgress) -> Self {
        Self {
            preparation: Some(stage),
            completed_items: 0,
            total_items: 0,
            current: None,
        }
    }
}
