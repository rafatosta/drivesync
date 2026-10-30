//! Combines complete local observations with remote plans without inferring deletion.
use super::Notice;
use crate::{
    local::{Evidence, Inventory},
    DownloadPlan,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
};

/// A diagnosis is observational and never grants permission to write user data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeKind {
    Unchanged,
    RemoteChanged,
    LocalChanged,
    ConcurrentChanges,
    Equivalent,
    Conciliable,
    LocationConflict,
    LocalIdentityChanged,
    ConcurrentIdentityChanges,
    ExistingLocalData,
    LocalMissing,
    LocalMissingRemoteChanged,
    LocalNew,
    RemoteUnavailable,
    Temporary,
    Writing,
    Unsupported,
    Unknown,
}
impl ChangeKind {
    /// User-facing consequences, independent of internal hashes or database formats.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Unchanged => "No content change was detected.",
            Self::RemoteChanged => "A Google Drive version is available. Local installation still requires verification.",
            Self::LocalChanged => "Local changes were preserved. Updating an existing Drive file is not available yet.",
            Self::ConcurrentChanges => "The local and Drive versions differ. Decide which content to keep; no version was selected.",
            Self::Conciliable => "The name, location or content changes can be reconciled. Applying this combination is not available yet; the available versions were left intact.",
            Self::LocationConflict => "This file has incompatible name or location changes. Review the destinations; no version was selected.",
            Self::Equivalent => "The same content was verified on this computer and Google Drive. No copy or replacement was needed.",
            Self::LocalIdentityChanged | Self::ConcurrentIdentityChanges => "This local file was replaced. Its contents were preserved; matching names or content do not authorize adoption.",
            Self::ExistingLocalData => "This file already exists locally and has no confirmed synchronization history. Its contents were preserved.",
            Self::LocalMissingRemoteChanged => "The local copy is missing and Google Drive has changed since the last confirmed synchronization. The Drive version was preserved; review this item before restoring a local copy.",
            Self::LocalMissing => "No local copy was observed. This does not request deletion on Google Drive.",
            Self::LocalNew => "A local item was found. Upload requires this folder’s upload setting and a verified available destination.",
            Self::RemoteUnavailable => "This item is no longer listed in the selected Drive folder. This may reflect a move or lost access; local data was preserved.",
            Self::Temporary => "This looks like a temporary file. Wait for the application to save its final file.",
            Self::Writing => "This file is still changing. Synchronization will check it again on the next cycle.",
            Self::Unsupported => "This item’s type or path cannot be synchronized safely. Existing data was preserved.",
            Self::Unknown => "This item could not be verified. Check access and synchronize again; no deletion was inferred.",
        }
    }
    /// Independent content changes, unstable input and missing evidence need attention.
    pub fn needs_attention(&self) -> bool {
        matches!(
            self,
            Self::LocalChanged
                | Self::Conciliable
                | Self::LocationConflict
                | Self::ConcurrentChanges
                | Self::LocalMissingRemoteChanged
                | Self::LocalIdentityChanged
                | Self::ConcurrentIdentityChanges
                | Self::ExistingLocalData
                | Self::Temporary
                | Self::Writing
                | Self::Unsupported
                | Self::Unknown
        )
    }
}
/// One scoped item may retain several historical paths without adopting a new object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconciliationEntry {
    pub relative_path: Option<PathBuf>,
    pub remote_id: Option<String>,
    pub kind: ChangeKind,
}
fn evidence_kind(evidence: Option<&Evidence>) -> Option<ChangeKind> {
    match evidence {
        Some(Evidence::Temporary) => Some(ChangeKind::Temporary),
        Some(Evidence::Writing) => Some(ChangeKind::Writing),
        Some(Evidence::Unknown) => Some(ChangeKind::Unknown),
        Some(Evidence::Unsupported) => Some(ChangeKind::Unsupported),
        _ => None,
    }
}
/// Repeated inputs produce the same ordered diagnostics. Missing remote observations
/// and old paths remain unavailable, never remote deletion or a new upload identity.
pub(crate) fn classify(
    local: &Inventory,
    plan: &DownloadPlan,
    history: &[Notice],
) -> Vec<ReconciliationEntry> {
    let mut represented = HashSet::new();
    let mut result = Vec::new();
    for entry in &plan.entries {
        if let Some(path) = &entry.relative_path {
            represented.insert(path.clone());
        }
        if let crate::PlanAction::Classified { local_path, .. } = &entry.action {
            represented.insert(local_path.clone());
        }
        let evidence = entry
            .relative_path
            .as_ref()
            .and_then(|p| local.entries.get(p));
        let kind = evidence_kind(evidence).unwrap_or_else(|| {
            crate::conflicts::plan_diagnosis(&entry.action, &entry.expected_local)
        });
        result.push(ReconciliationEntry {
            relative_path: entry.relative_path.clone(),
            remote_id: Some(entry.remote_id.clone()),
            kind,
        });
    }
    let mut historical = BTreeMap::new();
    for notice in history {
        if let Some(path) = &notice.path {
            if !represented.contains(path) {
                historical.insert((path.clone(), notice.id.clone()), ());
            }
        }
    }
    for (path, id) in historical.keys() {
        result.push(ReconciliationEntry {
            relative_path: Some(path.clone()),
            remote_id: Some(id.clone()),
            kind: ChangeKind::RemoteUnavailable,
        });
        represented.insert(path.clone());
    }
    for (path, evidence) in &local.entries {
        if !represented.contains(path) {
            result.push(ReconciliationEntry {
                relative_path: Some(path.clone()),
                remote_id: None,
                kind: evidence_kind(Some(evidence)).unwrap_or(ChangeKind::LocalNew),
            });
        }
    }
    result.sort_by(|a, b| (&a.relative_path, &a.remote_id).cmp(&(&b.relative_path, &b.remote_id)));
    result
}
