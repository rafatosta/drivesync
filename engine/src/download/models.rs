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
/// A user-visible pending item, combining execution failures and observations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadAttention {
    pub relative_path: Option<PathBuf>,
    pub message: String,
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
    /// Verified remote conflict copies with their original local/Drive binding.
    /// These remain pending and never establish a synchronized baseline.
    pub conflict_copies: Vec<ConflictCopy>,
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
impl DownloadReport {
    /// Combines pending diagnoses with specific execution errors without repeating observations.
    /// Pre-execution equivalence never hides a later remote or integrity failure.
    pub fn attention(&self) -> Vec<DownloadAttention> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for issue in &self.issues {
            seen.insert(issue.relative_path.clone());
            let diagnosis = self.reconciliation.iter().find(|entry| {
                entry.relative_path == issue.relative_path && entry.kind.needs_attention()
            });
            let message = if matches!(
                issue.error,
                EngineError::DownloadConflict
                    | EngineError::ObservationUnknown
                    | EngineError::LocalWriting
                    | EngineError::LocalTemporary
            ) {
                diagnosis
                    .map(|entry| entry.kind.description().to_owned())
                    .unwrap_or_else(|| issue.error.to_string())
            } else {
                issue.error.to_string()
            };
            let message = if let Some(copy) = self.conflict_copies.iter().find(|copy| {
                matches!(issue.error, EngineError::DownloadConflict)
                    && Some(&copy.original_path) == issue.relative_path.as_ref()
            }) {
                format!("Content conflict between the local file and Google Drive file. Remote version {} preserved at: {}. The local file remains at its original path; resolution is pending.", copy.remote_version, copy.copy_path.display())
            } else {
                message
            };
            result.push(DownloadAttention {
                relative_path: issue.relative_path.clone(),
                message,
            });
        }
        for entry in &self.reconciliation {
            if entry.kind.needs_attention() && seen.insert(entry.relative_path.clone()) {
                result.push(DownloadAttention {
                    relative_path: entry.relative_path.clone(),
                    message: entry.kind.description().to_owned(),
                });
            }
        }
        result
    }
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

#[cfg(test)]
mod reconciliation_tests {
    use super::*;
    use crate::{ChangeKind, ReconciliationEntry};
    #[test]
    fn pending_diagnoses_deduplicate_execution_failures_and_keep_specific_errors() {
        let mut report = DownloadReport {
            issues: vec![
                DownloadIssue {
                    relative_path: Some("local".into()),
                    error: EngineError::DownloadConflict,
                },
                DownloadIssue {
                    relative_path: Some("equal".into()),
                    error: EngineError::RemoteChanged,
                },
            ],
            reconciliation: vec![
                ReconciliationEntry {
                    relative_path: Some("local".into()),
                    remote_id: Some("id".into()),
                    kind: ChangeKind::LocalChanged,
                },
                ReconciliationEntry {
                    relative_path: Some("writing".into()),
                    remote_id: None,
                    kind: ChangeKind::Writing,
                },
                ReconciliationEntry {
                    relative_path: Some("equal".into()),
                    remote_id: Some("id2".into()),
                    kind: ChangeKind::Equivalent,
                },
            ],
            ..Default::default()
        };
        let attention = report.attention();
        assert_eq!(attention.len(), 3);
        assert_eq!(attention[0].message, ChangeKind::LocalChanged.description());
        assert_eq!(attention[1].message, EngineError::RemoteChanged.to_string());
        assert_eq!(attention[2].message, ChangeKind::Writing.description());
        report.issues.clear();
        report
            .reconciliation
            .retain(|e| e.kind == ChangeKind::Equivalent);
        assert!(report.attention().is_empty());
    }
}

/// A verified recoverable remote version; the local origin remains untouched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictCopy {
    pub original_path: PathBuf,
    pub copy_path: PathBuf,
    pub remote_id: String,
    pub remote_version: u64,
}

/// Scoped preservation journal, deliberately excluded from confirmed bases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Preservation {
    pub original: PathBuf,
    pub operation: Operation,
}
