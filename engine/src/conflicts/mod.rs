//! Account-scoped conflict choices and exact evidence matching. No provider writes occur here.
use crate::{
    inventory::models::{LocalState, RemoteState},
    models::*,
};
use serde::{Deserialize, Serialize};

/// Select content and location; original file bytes or directory locations are archived first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictChoice {
    KeepLocal,
    KeepRemote,
}
/// A persisted conflict in the selected account's active synchronization scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictSummary {
    pub id: Id,
    pub item_id: ItemId,
    pub revision: u64,
    pub local_path: Option<Path>,
    pub reason: String,
    pub local: ConflictVersion,
    pub remote: ConflictVersion,
    /// Private recovery directory, published before material replacement.
    pub archive_path: Path,
    pub can_resolve: bool,
    pub resolution_pending: bool,
}
/// Observed metadata for one replica at the listed conflict revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictVersion {
    pub size: Option<u64>,
    pub checksum: Option<Checksum>,
    pub modified_at: Option<DateTime>,
    pub provider_version: Option<u64>,
}

pub(crate) fn observed<T>(evidence: Evidence<T>) -> Option<T> {
    match evidence {
        Evidence::Known(value) => Some(value),
        _ => None,
    }
}

/// Optimistic authorization for the exact conflict revision displayed to the user.
#[derive(Debug, Clone)]
pub struct ResolveConflictRequest {
    pub account_id: AccountId,
    pub conflict_id: Id,
    pub expected_revision: u64,
    pub choice: ConflictChoice,
}
/// Sanitized conflict API failures; stale choices never become write permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictError {
    NotStarted,
    Unavailable,
    NotFound,
    Stale,
    Unsupported,
}

pub(crate) fn local_matches(a: &LocalState, b: &LocalState) -> bool {
    a.state == b.state
        && a.path == b.path
        && a.name == b.name
        && a.parent == b.parent
        && a.kind == b.kind
        && a.identity == b.identity
        && a.size == b.size
        && a.checksum == b.checksum
        && a.modified_at == b.modified_at
        && a.changed_at == b.changed_at
}
pub(crate) fn remote_matches(a: &RemoteState, b: &RemoteState) -> bool {
    a.state == b.state
        && a.id == b.id
        && a.name == b.name
        && a.parents == b.parents
        && a.mime_type == b.mime_type
        && a.trashed == b.trashed
        && a.version == b.version
        && a.size == b.size
        && a.checksum == b.checksum
}
pub(crate) fn resolvable(local: &LocalState, remote: &RemoteState) -> bool {
    if local.state != FileState::Present
        || remote.state != FileState::Present
        || remote.trashed != Evidence::Known(false)
        || !matches!(&local.identity, Evidence::Known(_))
        || local.path.is_none()
        || remote.id.is_none()
        || !matches!(&local.name,Evidence::Known(name) if !name.is_empty())
        || !matches!(&remote.name,Evidence::Known(name) if !name.is_empty())
        || !matches!(&local.parent, Evidence::Known(_))
        || !matches!(&remote.parents,Evidence::Known(parents) if parents.len()==1)
        || !matches!(remote.version,Evidence::Known(version) if version>0)
    {
        return false;
    }
    if local.kind == Evidence::Known(FileKind::Directory)
        && remote.mime_type == Evidence::Known("application/vnd.google-apps.folder".into())
    {
        return true;
    }
    local.kind == Evidence::Known(FileKind::Regular)
        && matches!(&remote.mime_type,Evidence::Known(m) if !m.starts_with("application/vnd.google-apps."))
        && matches!((&local.checksum,&remote.checksum),(Evidence::Known(a),Evidence::Known(b)) if a.algorithm=="sha256" && b.algorithm=="sha256")
}
