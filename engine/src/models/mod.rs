//! Shared identities and synchronization value types. This module performs no I/O.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Declares a string-backed identifier whose value is opaque to consumers.
macro_rules! string_id {
    ($($name:ident),+ $(,)?) => {
        $(
            #[doc = "Opaque identifier scoped to its corresponding DriveSync entity."]
            #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
            #[serde(transparent)]
            pub struct $name(pub String);
        )+
    };
}

string_id!(Id, SyncRootId, ItemId, RemoteId);

/// Stable application account identity, independent of presentation fields.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AccountId(pub String);
/// External service identifier; not an account identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProviderId(pub String);
/// UTC instant with millisecond precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timestamp {
    pub unix_millis: i64,
}

/// UTC instant with millisecond precision, matching the existing account timestamp contract.
pub type DateTime = Timestamp;

/// Relative path beneath a configured local synchronization root.
///
/// `PathBuf` retains platform-native path data, including non-UTF-8 components on Unix.
pub type Path = PathBuf;

/// Describes whether an attribute was observed and how trustworthy that observation is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Evidence<T> {
    /// The source observed a value, including meaningful false, zero, or empty values.
    Known(T),
    /// The source did not collect this attribute.
    NotCollected,
    /// The attribute does not apply to this item.
    NotApplicable,
    /// The source does not provide this attribute.
    Unsupported,
    /// The query could not determine the value temporarily.
    Unavailable,
    /// Access to this attribute was denied.
    PermissionDenied,
}

/// Presence or observability of an item within its declared scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileState {
    /// The item was observed to exist.
    Present,
    /// Absence was confirmed within the correct scope.
    Absent,
    /// The available evidence does not determine presence.
    Unknown,
    /// The source or scope could not be accessed.
    Unavailable,
    /// The item is known but lies outside the managed scope.
    OutsideScope,
}

/// Broad filesystem entry kind, independent of any remote provider's MIME model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileKind {
    /// A regular filesystem file.
    Regular,
    /// A directory.
    Directory,
    /// A symbolic link.
    Symlink,
    /// A filesystem entry outside the supported kinds.
    Unsupported,
}

/// Filesystem identity evidence, scoped by its device and optional mount.
///
/// This is auxiliary evidence only; it must not replace `ItemId` or act as a universal key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalIdentity {
    /// Device or filesystem identifier.
    pub device_id: u64,
    /// Inode number on that device.
    pub inode: u64,
    /// Mount identifier when the platform can provide it.
    pub mount_id: Option<u64>,
}

/// Content digest normalized together with the algorithm that produced it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checksum {
    /// Digest algorithm name, such as `md5`.
    pub algorithm: String,
    /// Normalized digest value.
    pub value: String,
}

/// Observation source category, without implying a synchronization action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationOrigin {
    /// Initial local scan.
    Scanner,
    /// Continuous local observation.
    Local,
    /// Remote provider observation.
    Remote,
}

/// Kind of signal reported by an observation source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationKind {
    /// Item appeared in a snapshot.
    Snapshot,
    /// A creation signal was observed.
    Created,
    /// A content change signal was observed.
    Modified,
    /// A rename signal was observed.
    Renamed,
    /// A move signal was observed.
    Moved,
    /// A removal signal was observed; this does not prove permanent deletion.
    Removed,
    /// Metadata changed without a more specific signal.
    MetadataChanged,
    /// The source could not classify the signal.
    Unknown,
    /// A fresh observation is required.
    RecheckRequired,
}

/// Optional normalized context for a local or remote observation.
///
/// A removal event is only a signal and must not be treated as proof of permanent deletion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationEvent {
    /// Source category that reported the signal.
    pub origin: ObservationOrigin,
    /// Signal classification.
    pub kind: ObservationKind,
    /// Time at which the signal was observed.
    pub observed_at: DateTime,
    /// Previous local path when applicable.
    pub old_path: Option<Path>,
    /// Current local path when applicable.
    pub new_path: Option<Path>,
    /// Associated remote item identity when known.
    pub remote_id: Option<RemoteId>,
    /// Local move correlation token when available.
    pub move_cookie: Option<u64>,
}

/// Capabilities observed for a local item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalCapabilities {
    /// Whether content can be read.
    pub can_read: Evidence<bool>,
    /// Whether content can be modified.
    pub can_write: Evidence<bool>,
    /// Whether the item can be renamed.
    pub can_rename: Evidence<bool>,
    /// Whether the item can be removed using the supported safe operation.
    pub can_delete: Evidence<bool>,
}

/// Capabilities reported by a remote provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteCapabilities {
    /// Whether content can be downloaded.
    pub can_download: Evidence<bool>,
    /// Whether remote content can be modified.
    pub can_modify_content: Evidence<bool>,
    /// Whether the item can be renamed remotely.
    pub can_rename: Evidence<bool>,
    /// Whether the item can be moved within its drive.
    pub can_move_item_within_drive: Evidence<bool>,
    /// Whether the item can be sent to the provider's trash.
    pub can_trash: Evidence<bool>,
}

/// Correlates a logical item with identities scoped to one account and sync root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileIdentity {
    /// Owning account scope.
    pub account_id: AccountId,
    /// Local synchronization root scope.
    pub sync_root_id: SyncRootId,
    /// Inventory identity, absent before correlation.
    pub item_id: Option<ItemId>,
    /// Remote provider identity, absent when not known.
    pub remote_id: Option<RemoteId>,
    /// Auxiliary local filesystem identity evidence.
    pub local_identity: Evidence<LocalIdentity>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_keeps_unknown_states_distinct_from_known_values() {
        assert_ne!(Evidence::Known(false), Evidence::NotCollected);
        assert_ne!(Evidence::Known(false), Evidence::Unavailable);
        assert_eq!(Evidence::Known(Vec::<u8>::new()), Evidence::Known(vec![]));
    }

    #[test]
    fn typed_ids_serialize_as_their_opaque_value() {
        let id = SyncRootId("root-1".into());
        assert_eq!(serde_json::to_string(&id).unwrap(), "\"root-1\"");
        assert_eq!(
            serde_json::from_str::<SyncRootId>("\"root-1\"").unwrap(),
            id
        );
    }
}

mod operation;
pub(crate) mod path_serde;
pub(crate) use operation::{ExecutionEffect, Operation, OperationAction, OperationPreconditions};
