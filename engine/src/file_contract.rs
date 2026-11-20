//! Standalone contract between detection/context and decision-making.
//! No I/O, decision rules or provider calls belong here.
//! Sources: Google Drive API v3 Files/Changes, Rust std::fs::Metadata,
//! Unix MetadataExt, Linux statx/inotify and GIO FileInfo.

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::SystemTime;

/// Unknown is not absence; unsupported attributes are not failed observations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fact<T> {
    Known(T),
    Unknown(UnavailableReason),
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnavailableReason {
    Offline,
    PermissionDenied,
    NotFoundOrAccessLost,
    IoFailure,
    ProviderFailure,
    NotObserved,
    AmbiguousIdentity,
    UnstableSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileState<T> {
    Present(T),
    Absent,
    OutsideScope(T),
    Unavailable(UnavailableReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalIdentity {
    pub device_id: u64,
    pub inode: u64,
    pub mount_id: Fact<u64>,
    pub birth_time: Fact<SystemTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileIdentity {
    pub account_id: String,
    pub item_id: Option<String>,
    pub remote_id: Fact<String>,
    pub local_identity: Fact<LocalIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalKind {
    Regular,
    Directory,
    Symlink,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checksums {
    pub md5: Fact<[u8; 16]>,
    pub sha1: Fact<[u8; 20]>,
    pub sha256: Fact<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalCapabilities {
    pub can_read: Fact<bool>,
    pub can_write: Fact<bool>,
    pub can_rename: Fact<bool>,
    pub can_delete: Fact<bool>,
    pub can_trash: Fact<bool>,
    pub can_create_children: Fact<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFile {
    pub identity: LocalIdentity,
    pub path: PathBuf,
    pub name: OsString,
    pub parent: PathBuf,
    pub kind: LocalKind,
    pub size: Fact<u64>,
    pub created_at: Fact<SystemTime>,
    pub modified_at: Fact<SystemTime>,
    pub changed_at: Fact<SystemTime>,
    pub content_type: Fact<String>,
    pub etag: Fact<String>,
    pub checksums: Checksums,
    pub symlink_target: Fact<PathBuf>,
    pub mode: Fact<u32>,
    pub uid: Fact<u32>,
    pub gid: Fact<u32>,
    pub hard_link_count: Fact<u64>,
    pub capabilities: LocalCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteCapabilities {
    pub can_download: Fact<bool>,
    pub can_edit: Fact<bool>,
    pub can_modify_content: Fact<bool>,
    pub can_rename: Fact<bool>,
    pub can_move: Fact<bool>,
    pub can_trash: Fact<bool>,
    pub can_untrash: Fact<bool>,
    pub can_delete: Fact<bool>,
    pub can_add_children: Fact<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteShortcut {
    pub target_id: String,
    pub target_mime_type: Fact<String>,
    pub target_resource_key: Fact<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFile {
    pub id: String,
    pub name: String,
    pub parents: Fact<Vec<String>>,
    pub drive_id: Fact<String>,
    pub resource_key: Fact<String>,
    pub mime_type: String,
    pub size: Fact<u64>,
    pub checksums: Checksums,
    pub version: Fact<u64>,
    pub head_revision_id: Fact<String>,
    pub created_at: Fact<SystemTime>,
    pub modified_at: Fact<SystemTime>,
    pub trashed: Fact<bool>,
    pub explicitly_trashed: Fact<bool>,
    pub shortcut: Fact<RemoteShortcut>,
    pub web_view_link: Fact<String>,
    pub export_links: Fact<Vec<(String, String)>>,
    pub content_restricted: Fact<bool>,
    pub capabilities: RemoteCapabilities,
}

pub type LocalState = FileState<LocalFile>;
pub type RemoteState = FileState<RemoteFile>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Projection {
    Native,
    WebShortcut,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncedState {
    pub local: LocalState,
    pub remote: RemoteState,
    pub confirmed_at: SystemTime,
    pub projection: Fact<Projection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Baseline {
    NeverSynced,
    Synced(Box<SyncedState>),
    Unavailable(UnavailableReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    Scanner,
    Local,
    Remote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventKind {
    Created,
    Modified,
    Removed,
    Moved,
    AttributesChanged,
    Observed,
    AccessLost,
    RemovedOrAccessLost,
    Overflow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub origin: Origin,
    pub kind: EventKind,
    pub observed_at: SystemTime,
    pub old_path: Option<PathBuf>,
    pub new_path: Option<PathBuf>,
    pub remote_id: Option<String>,
    pub move_cookie: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observations {
    pub events: Vec<Observation>,
    pub local_coverage_complete: Fact<bool>,
    pub remote_coverage_complete: Fact<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Environment {
    pub local_root: PathBuf,
    pub remote_root_id: String,
    pub local_root_available: Fact<bool>,
    pub remote_available: Fact<bool>,
    pub destination_local_parent_available: Fact<bool>,
    pub destination_remote_parent_id: Fact<String>,
    pub destination_local_occupied: Fact<bool>,
    pub destination_remote_matches: Fact<Vec<String>>,
    pub prior_conflict: Fact<bool>,
}

/// A contextualized item; contains evidence, not a synchronization decision.
/// Missing facts must be handled explicitly by the analyzer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    pub identity: FileIdentity,
    pub local: LocalState,
    pub remote: RemoteState,
    pub baseline: Baseline,
    pub observations: Observations,
    pub environment: Environment,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_is_not_absent() {
        assert_ne!(
            FileState::<LocalFile>::Absent,
            FileState::Unavailable(UnavailableReason::PermissionDenied)
        );
    }

    #[test]
    fn unknown_is_not_false_or_unsupported() {
        let unknown = Fact::Unknown(UnavailableReason::Offline);
        assert_ne!(unknown, Fact::Known(false));
        assert_ne!(unknown, Fact::NotApplicable);
    }

    #[test]
    fn missing_baseline_is_not_never_synced() {
        assert_ne!(Baseline::NeverSynced,
            Baseline::Unavailable(UnavailableReason::IoFailure));
    }
}
