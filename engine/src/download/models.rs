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
/// A completed manual cycle. Issues/skipped items are not synchronization success.
#[derive(Debug, Clone, Default)]
pub struct DownloadReport {
    pub downloaded: usize,
    pub recovered: usize,
    pub unchanged: usize,
    pub directories: usize,
    pub skipped: usize,
    pub issues: Vec<DownloadIssue>,
}
/// Progress is observational; bytes received do not imply installation.
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub relative_path: PathBuf,
    pub received: u64,
    pub total: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Identity {
    pub device: u64,
    pub inode: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Phase {
    Started,
    Ready { identity: Identity, sha256: String },
    Installed { identity: Identity, sha256: String },
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
