//! Combines complete local observations with remote plans without inferring deletion.
use super::Notice;
use crate::{
    local::{Evidence, Inventory},
    DownloadPlan,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
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
    /// Local work differs from B while the remote identity cannot be observed.
    LocalChangedRemoteUnavailable,
    /// Explicit trash observation, without authority to delete the local version.
    RemoteDeleted,
    /// Local work conflicts with an explicit remote trash observation.
    EditDeleteConflict,
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
            Self::ConcurrentChanges => "Different edits were verified in the local file and the Google Drive file. Both were left in place; no conflict backup was created. Review both versions. Choosing a version in DriveSync is not available yet.",
            Self::Conciliable => "The name, location or content changes can be reconciled. Applying this combination is not available yet; the available versions were left intact.",
            Self::LocationConflict => "This file has incompatible name or location changes. Review the destinations; no version was selected.",
            Self::Equivalent => "The same content was verified on this computer and Google Drive. No copy or replacement was needed.",
            Self::LocalIdentityChanged | Self::ConcurrentIdentityChanges => "This local file was replaced. Its contents were preserved; matching names or content do not authorize adoption.",
            Self::ExistingLocalData => "This file already exists locally and has no confirmed synchronization history. Its contents were preserved.",
            Self::LocalMissingRemoteChanged => "The local copy is missing and Google Drive has changed since the last confirmed synchronization. The Drive version was preserved; review this item before restoring a local copy.",
            Self::LocalMissing => "No local copy was observed. This does not request deletion on Google Drive.",
            Self::LocalNew => "A local item was found. Upload requires this folder’s upload setting and a verified available destination.",
            Self::RemoteDeleted => "This Drive item was observed in the trash. Local data was preserved; review before resolving. This observation does not authorize deletion.",
            Self::EditDeleteConflict => "Local changes or a changed descendant conflict with a Drive item observed in the trash. Local work was preserved. Review before resolving; no conflict backup was created.",
            Self::LocalChangedRemoteUnavailable => "Local changes or a changed descendant were found while the Drive item is unavailable. Deletion, a move and lost access cannot be distinguished from this observation. Local work was preserved; restore access and review before resolving. No conflict backup was created.",
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
                | Self::RemoteDeleted
                | Self::EditDeleteConflict
                | Self::RemoteUnavailable
                | Self::LocalChangedRemoteUnavailable
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
    bases: &HashMap<String, crate::ConfirmedBase>,
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
            relative_path: match &entry.action {
                crate::PlanAction::Classified { local_path, .. } => Some(local_path.clone()),
                _ => entry.relative_path.clone(),
            },
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
    // Confirmed locations survive even without an observation-only notice.
    let current_ids: HashSet<_> = plan.entries.iter().map(|e| e.remote_id.as_str()).collect();
    for (id, base) in bases {
        if !current_ids.contains(id.as_str()) {
            if let Some(location) = &base.location {
                historical.insert((location.relative_path.clone(), id.clone()), ());
            }
        }
    }
    let mut relocated: HashMap<String, Option<PathBuf>> = HashMap::new();
    for (path, evidence) in &local.entries {
        if let Evidence::Stable { fingerprint, .. } = evidence {
            relocated
                .entry(fingerprint.to_ascii_lowercase())
                .and_modify(|candidate| *candidate = None)
                .or_insert_with(|| Some(path.clone()));
        }
    }
    let deleted: HashSet<_> = history
        .iter()
        .filter(|n| n.deleted)
        .map(|n| (&n.id, &n.path))
        .collect();
    let mut moved = BTreeMap::new();
    for (path, id) in historical.keys() {
        let mut kind = ChangeKind::RemoteUnavailable;
        if !current_ids.contains(id.as_str()) {
            if let Some(base) = bases.get(id) {
                if base
                    .location
                    .as_ref()
                    .is_some_and(|l| l.relative_path == *path)
                {
                    match local.observe(path) {
                        crate::LocalObservation::Verified(ref fingerprint) => {
                            kind = match (
                                crate::planner::fingerprint_parts(fingerprint),
                                crate::planner::fingerprint_parts(&base.local_fingerprint),
                            ) {
                                (Some((hash, dev, ino)), Some((bh, bd, bi)))
                                    if (dev, ino) == (bd, bi) =>
                                {
                                    if hash.eq_ignore_ascii_case(bh) {
                                        ChangeKind::RemoteUnavailable
                                    } else {
                                        ChangeKind::LocalChangedRemoteUnavailable
                                    }
                                }
                                _ => ChangeKind::LocalIdentityChanged,
                            };
                        }
                        crate::LocalObservation::Absent => {
                            if let Some(Some(destination)) =
                                relocated.get(&base.local_fingerprint.to_ascii_lowercase())
                            {
                                represented.insert(destination.clone());
                                moved.insert(
                                    (destination.clone(), id.clone()),
                                    if deleted.contains(&(id, &Some(path.clone()))) {
                                        ChangeKind::EditDeleteConflict
                                    } else {
                                        ChangeKind::LocalChangedRemoteUnavailable
                                    },
                                );
                                kind = ChangeKind::LocalChangedRemoteUnavailable;
                            }
                        }
                        _ => kind = ChangeKind::Unknown,
                    }
                }
            }
        }
        if !current_ids.contains(id.as_str()) && deleted.contains(&(id, &Some(path.clone()))) {
            kind = match kind {
                ChangeKind::RemoteUnavailable => ChangeKind::RemoteDeleted,
                ChangeKind::LocalChangedRemoteUnavailable => ChangeKind::EditDeleteConflict,
                other => other,
            };
        }
        result.push(ReconciliationEntry {
            relative_path: Some(path.clone()),
            remote_id: Some(id.clone()),
            kind,
        });
        represented.insert(path.clone());
    }
    let mut result: BTreeMap<_, _> = result
        .into_iter()
        .map(|e| ((e.relative_path.clone(), e.remote_id.clone()), e))
        .collect();
    for ((path, id), kind) in moved {
        result.insert(
            (Some(path.clone()), Some(id.clone())),
            ReconciliationEntry {
                relative_path: Some(path),
                remote_id: Some(id),
                kind,
            },
        );
    }
    let mut result: Vec<_> = result.into_values().collect();
    // An inode match alone cannot establish continuity after a rename/edit,
    // but it must not turn managed work into a new upload candidate either.
    let managed: HashSet<_> = bases
        .values()
        .filter_map(|base| {
            crate::planner::fingerprint_parts(&base.local_fingerprint).map(|(_, d, i)| (d, i))
        })
        .collect();
    for (path, evidence) in &local.entries {
        if !represented.contains(path) {
            result.push(ReconciliationEntry {
                relative_path: Some(path.clone()),
                remote_id: None,
                kind: evidence_kind(Some(evidence)).unwrap_or_else(|| {
                    if matches!(evidence, Evidence::Stable { stamp, .. }
                        if managed.contains(&(stamp.device, stamp.inode)))
                    {
                        ChangeKind::Unknown
                    } else {
                        ChangeKind::LocalNew
                    }
                }),
            });
        }
    }
    // Propagate attention through unavailable historical directories using path
    // components: "docs-old" is not a descendant of "docs".
    let ancestors: HashSet<_> = result
        .iter()
        .filter(|e| {
            matches!(
                e.kind,
                ChangeKind::LocalChangedRemoteUnavailable
                    | ChangeKind::EditDeleteConflict
                    | ChangeKind::LocalIdentityChanged
                    | ChangeKind::LocalNew
            )
        })
        .filter_map(|e| e.relative_path.as_ref())
        .flat_map(|p| p.ancestors().skip(1).map(|a| a.to_path_buf()))
        .collect();
    let uncertain_ancestors: HashSet<_> = result
        .iter()
        .filter(|e| {
            matches!(
                e.kind,
                ChangeKind::Unknown | ChangeKind::Writing | ChangeKind::Unsupported
            )
        })
        .filter_map(|e| e.relative_path.as_ref())
        .flat_map(|p| p.ancestors().skip(1).map(|a| a.to_path_buf()))
        .collect();
    for entry in &mut result {
        if matches!(
            entry.kind,
            ChangeKind::RemoteUnavailable | ChangeKind::RemoteDeleted
        ) && entry
            .relative_path
            .as_ref()
            .is_some_and(|p| ancestors.contains(p))
        {
            entry.kind = if entry.kind == ChangeKind::RemoteDeleted {
                ChangeKind::EditDeleteConflict
            } else {
                ChangeKind::LocalChangedRemoteUnavailable
            };
        } else if matches!(
            entry.kind,
            ChangeKind::RemoteUnavailable | ChangeKind::RemoteDeleted
        ) && entry
            .relative_path
            .as_ref()
            .is_some_and(|p| uncertain_ancestors.contains(p))
        {
            entry.kind = ChangeKind::Unknown;
        }
    }
    result.sort_by(|a, b| (&a.relative_path, &a.remote_id).cmp(&(&b.relative_path, &b.remote_id)));
    result
}
