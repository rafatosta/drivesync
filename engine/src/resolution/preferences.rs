//! Opt-in rules for proven same-path concurrent binary edits only.
use crate::{ChangeKind, ConflictRecord, ConflictStatus, ResolutionChoice, ResolutionRequest};
use serde::{Deserialize, Serialize};

/// Per-folder content-conflict preference. Missing settings always mean Ask.
/// Deletion, location conflicts and unknown evidence cannot opt into this rule.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictPreference {
    #[default]
    Ask,
    KeepLocal,
    KeepRemote,
    KeepBoth,
}
impl ConflictPreference {
    /// Selects exact pending evidence; the executor must still prove continuity,
    /// preserve both versions and revalidate before each conditional effect.
    pub(crate) fn request(self, record: &ConflictRecord) -> Option<ResolutionRequest> {
        if record.status != ConflictStatus::Pending
            || record.diagnosis != ChangeKind::ConcurrentChanges
        {
            return None;
        }
        let choice = match self {
            Self::Ask => return None,
            Self::KeepLocal => ResolutionChoice::KeepLocal,
            Self::KeepRemote => ResolutionChoice::KeepRemote,
            Self::KeepBoth => ResolutionChoice::KeepBoth,
        };
        ResolutionRequest::from_record(record, choice).ok()
    }
}

/// Requires a proven content conflict with unchanged names, parents and identity.
/// Unknown, moves, replacements and deletions cannot reach the executor.
pub(crate) fn candidate(
    item: &crate::InventoryEntry,
    entry: &crate::PlanEntry,
    base: &crate::ConfirmedBase,
    local: &crate::local::Inventory,
    request: &ResolutionRequest,
) -> Option<super::planning::Candidate> {
    if entry.action != crate::PlanAction::Blocked(crate::ConflictReason::ConcurrentChanges) {
        return None;
    }
    let local_path = entry.relative_path.as_ref()?;
    let location = base.location.as_ref()?;
    let crate::LocalObservation::Verified(fingerprint) = local.observe(local_path) else {
        return None;
    };
    let (_, dev, ino) = crate::planner::fingerprint_parts(&base.local_fingerprint)?;
    let (_, current_dev, current_ino) = crate::planner::fingerprint_parts(&fingerprint)?;
    let aliases = local
        .entries
        .values()
        .filter(|e| {
            matches!(e, crate::local::Evidence::Stable { stamp, .. }
            if (stamp.device, stamp.inode) == (dev, ino))
        })
        .count();
    if item.kind != crate::InventoryKind::Binary
        || item.id != entry.remote_id
        || request.record.remote_id.as_ref() != Some(&item.id)
        || request.record.local_fingerprint.as_ref() != Some(&fingerprint)
        || request.record.remote_version != item.version
        || request.record.remote_parent_id.as_ref() != Some(&item.parent_id)
        || request.record.remote_name.as_ref() != Some(&item.name)
        || request.record.relative_path.as_ref() != Some(local_path)
        || local_path != &location.relative_path
        || item.parent_id != location.remote_parent
        || item.name != location.remote_name
        || local_path.file_name()?.to_str()? != item.name
        || (dev, ino) != (current_dev, current_ino)
        || item.version? <= base.remote_version
        || aliases != 1
    {
        return None;
    }
    Some(super::planning::Candidate {
        origin: local_path.clone(),
        target: local_path.clone(),
        fingerprint,
        choice: request.choice,
    })
}
