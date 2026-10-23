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
/// A completed download cycle. Issues/skipped items are not synchronization success.
#[derive(Debug, Clone, Default)]
pub struct DownloadReport {
    pub downloaded: usize,
    pub updated: usize,
    /// Relative paths to prior versions retained beside updated files.
    pub preserved_copies: Vec<PathBuf>,
    pub recovered: usize,
    pub unchanged: usize,
    pub directories: usize,
    pub skipped: usize,
    pub issues: Vec<DownloadIssue>,
}
/// Current item stage. Receiving bytes is distinct from verification/installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadStage {
    Checking,
    Downloading,
    Verifying,
    Installing,
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
/// Cycle progress counts processed inventory items, including folders, skipped
/// items and failures. It is not a measure of successful installation or time.
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub completed_items: usize,
    pub total_items: usize,
    pub current: Option<DownloadItemProgress>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Identity {
    pub device: u64,
    pub inode: u64,
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
