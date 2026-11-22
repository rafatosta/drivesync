//! Typed Inventory queries, snapshots, baselines, and writes.

use crate::models::{
    AccountId, Checksum, DateTime, Evidence, FileIdentity, FileKind, FileState, Id,
    LocalCapabilities, LocalIdentity, Path, RemoteCapabilities, RemoteId, SyncRootId,
};
use serde::{Deserialize, Serialize};

/// Lookup keys are always constrained by account and synchronization root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InventoryLookup {
    /// Account isolation boundary.
    pub account_id: AccountId,
    /// Synchronization root isolation boundary.
    pub sync_root_id: SyncRootId,
    /// Logical item key, when already assigned.
    pub item_id: Option<crate::models::ItemId>,
    /// Provider identity, when already correlated.
    pub remote_id: Option<RemoteId>,
    /// Auxiliary local identity evidence; only Known is used as a lookup key.
    pub local_identity: Evidence<LocalIdentity>,
    /// Relative local path used as an auxiliary key.
    pub local_path: Option<Path>,
}

/// Outcome of a read that distinguishes conclusive absence from an unavailable store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum InventoryLookupStatus {
    /// Exactly one snapshot matched all supplied keys.
    Found,
    /// No supplied key matched a record in the requested scope.
    NotFound,
    /// Supplied keys matched multiple records or contradicted one another.
    Ambiguous,
    /// The database or requested account scope was unavailable.
    Unavailable,
    /// The lookup could not be completed because the request or stored record was invalid.
    Error,
}

/// Snapshot returned only for a unique successful lookup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct InventoryLookupResult {
    /// Explicit lookup outcome.
    pub status: InventoryLookupStatus,
    /// Persisted facts when status is Found.
    pub snapshot: Option<InventorySnapshot>,
    /// Sanitized diagnostic for ambiguity, invalid input, or store failures.
    pub reason: Option<String>,
}

impl InventoryLookupResult {
    pub(super) fn new(
        status: InventoryLookupStatus,
        snapshot: Option<InventorySnapshot>,
        reason: Option<String>,
    ) -> Self {
        Self {
            status,
            snapshot,
            reason,
        }
    }

    pub(super) fn unavailable(reason: &str) -> Self {
        Self::new(
            InventoryLookupStatus::Unavailable,
            None,
            Some(reason.to_owned()),
        )
    }
}

/// Persistent facts needed by Context and Operations for one correlated item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct InventorySnapshot {
    /// Account, sync root, and logical/provider/local identities.
    pub identity: FileIdentity,
    /// Last state confirmed by an operation.
    pub baseline: Baseline,
    /// Operations that are pending, in progress, or retained for recovery/audit.
    pub pending_operations: Vec<PendingOperation>,
    /// Conflict currently relevant to this item, if any.
    pub prior_conflict: Option<ConflictState>,
    /// Monotonic optimistic-concurrency revision for this record.
    pub revision: u64,
}

/// Last confirmed synchronization state; it is never updated by observation alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Baseline {
    /// Whether a confirmed baseline exists and is available.
    pub state: BaselineState,
    /// Last confirmed local replica state.
    pub local: Option<LocalState>,
    /// Last confirmed remote replica state.
    pub remote: Option<RemoteState>,
    /// Time at which the baseline was confirmed.
    pub confirmed_at: Option<DateTime>,
    /// Physical representation of the local replica, when applicable.
    pub projection: Option<ProjectionKind>,
}

impl Baseline {
    pub(crate) fn never_synced() -> Self {
        Self {
            state: BaselineState::NeverSynced,
            local: None,
            remote: None,
            confirmed_at: None,
            projection: None,
        }
    }

    /// Returns an explicit unavailable baseline for inconclusive Inventory results.
    pub(crate) fn unavailable() -> Self {
        Self {
            state: BaselineState::Unavailable,
            local: None,
            remote: None,
            confirmed_at: None,
            projection: None,
        }
    }
}

/// Baseline availability states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum BaselineState {
    /// The local and remote states were confirmed as synchronized.
    Synced,
    /// A conclusive lookup found no prior synchronization.
    NeverSynced,
    /// A prior baseline exists but cannot currently be read.
    Unavailable,
}

/// Local replica facts suitable for a confirmed baseline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LocalState {
    /// Whether the item is present and observable locally.
    pub state: FileState,
    /// Current path relative to the local root.
    #[serde(with = "path_serde::option")]
    pub path: Option<Path>,
    /// Current name evidence.
    pub name: Evidence<String>,
    /// Parent path relative to the local root.
    #[serde(with = "path_serde::evidence")]
    pub parent: Evidence<Path>,
    /// Filesystem entry kind.
    pub kind: Evidence<FileKind>,
    /// Auxiliary filesystem identity.
    pub identity: Evidence<LocalIdentity>,
    /// Size in bytes when applicable.
    pub size: Evidence<u64>,
    /// Local content modification time.
    pub modified_at: Evidence<DateTime>,
    /// Local metadata/state change time, not creation time.
    pub changed_at: Evidence<DateTime>,
    /// Content digest evidence.
    pub checksum: Evidence<Checksum>,
    /// Local operation capabilities.
    pub capabilities: Evidence<LocalCapabilities>,
}

/// Remote replica facts suitable for a confirmed baseline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RemoteState {
    /// Whether the item is present and observable remotely.
    pub state: FileState,
    /// Provider item identity, when known.
    pub id: Option<RemoteId>,
    /// Current name evidence.
    pub name: Evidence<String>,
    /// Parent provider identities.
    pub parents: Evidence<Vec<RemoteId>>,
    /// Provider MIME type.
    pub mime_type: Evidence<String>,
    /// Size in bytes when applicable.
    pub size: Evidence<u64>,
    /// Content digest evidence.
    pub checksum: Evidence<Checksum>,
    /// Provider item version.
    pub version: Evidence<u64>,
    /// Remote content modification time.
    pub modified_time: Evidence<DateTime>,
    /// Whether the provider reports the item in its trash.
    pub trashed: Evidence<bool>,
    /// Remote operation capabilities.
    pub capabilities: Evidence<RemoteCapabilities>,
}

/// Physical representation of a local replica.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ProjectionKind {
    /// A normal local file or directory.
    Native,
    /// A local shortcut representing a Google Workspace item.
    GoogleWorkspaceShortcut,
    /// A local shortcut representing an item that exists only remotely.
    RemoteOnlyShortcut,
}

/// One provider or filesystem operation retained in the item snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PendingOperation {
    /// Stable identity of this operation execution.
    pub id: Id,
    /// Logical item associated with the operation.
    pub item_id: crate::models::ItemId,
    /// Requested operation category.
    pub action: OperationAction,
    /// Persisted execution state.
    pub state: OperationState,
    /// Remote identity obtained during partial execution, when known.
    pub remote_id: Option<RemoteId>,
    /// Local and remote effects observed during execution, when available.
    pub applied_effect: Option<AppliedEffect>,
    /// Idempotency key supplied by the executor, when supported.
    pub idempotency_key: Option<String>,
    /// Time at which the operation was first recorded.
    pub created_at: DateTime,
    /// Time at which the operation state was last changed.
    pub updated_at: DateTime,
}

/// Operation categories shared by pending Inventory records and the executor contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum OperationAction {
    /// Upload local content to the provider.
    Upload,
    /// Download remote content to the local filesystem.
    Download,
    /// Update remote content.
    UpdateRemoteContent,
    /// Update local content.
    UpdateLocalContent,
    /// Rename a remote item.
    RenameRemote,
    /// Rename a local item.
    RenameLocal,
    /// Move a remote item.
    MoveRemote,
    /// Move a local item.
    MoveLocal,
    /// Send a remote item to the provider's trash.
    TrashRemote,
    /// Remove a local replica according to the configured safe policy.
    RemoveLocal,
    /// Create a remote directory.
    CreateRemoteDirectory,
    /// Create a local directory.
    CreateLocalDirectory,
    /// Create a local shortcut for a provider-native item.
    CreateLocalShortcut,
}

/// Persisted execution state of an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum OperationState {
    /// Recorded but not started.
    Pending,
    /// Execution started.
    Running,
    /// Effect may have been applied but is not confirmed.
    AppliedUnconfirmed,
    /// Execution effect has been confirmed.
    Confirmed,
    /// Execution failed.
    Failed,
}

/// Persistent conflict facts associated with one logical item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ConflictState {
    /// Logical item associated with the conflict.
    pub item_id: crate::models::ItemId,
    /// Stable conflict category or diagnostic code.
    pub reason: String,
    /// Whether a user or policy has resolved the conflict.
    pub resolved: bool,
    /// Explicit resolution choice, represented as an opaque policy value.
    pub resolution: Option<ConflictResolution>,
    /// Time at which the conflict state last changed.
    pub updated_at: DateTime,
}

/// Explicit conflict-resolution choice supplied by a later policy/UI contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ConflictResolution {
    /// Opaque stable choice understood by the component that requested resolution.
    pub choice: String,
}

/// Observed effects of a completed operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AppliedEffect {
    /// Local state observed after execution, when applicable.
    pub local: Option<LocalState>,
    /// Remote state observed after execution, when applicable.
    pub remote: Option<RemoteState>,
    /// Remote identity created or updated by the operation.
    pub remote_id: Option<RemoteId>,
    /// Whether sufficient evidence confirmed the effect.
    pub verified: bool,
}

/// Typed change requested by Operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum PersistedEffect {
    /// Record a newly planned or started operation.
    Pending(PendingOperation),
    /// Record an observed operation effect that may require later confirmation.
    OperationApplied(AppliedEffect),
    /// Confirm a verified individual effect without asserting whole-item convergence.
    OperationConfirmed,
    /// Replace the baseline after the caller has confirmed the resulting states.
    BaselineConfirmed(Baseline),
    /// Record a failed operation.
    OperationFailed,
    /// Set or clear the conflict associated with this item.
    ConflictUpdated(Option<ConflictState>),
}

/// Atomic write request, including the account and root scope used by the database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct InventoryWrite {
    /// Operation execution identity.
    pub operation_id: Id,
    /// Logical item key required by every synchronization write.
    pub item_id: crate::models::ItemId,
    /// Complete identity and account/sync-root scope to persist.
    pub identity: FileIdentity,
    /// Expected current revision; new records require Some(0), which asserts a conclusive
    /// NotFound lookup. None opts out of comparison only for an existing record.
    pub expected_revision: Option<u64>,
    /// Typed state transition to apply.
    pub effect: PersistedEffect,
    /// Time at which this write was requested.
    pub recorded_at: DateTime,
}

/// Persistent write outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum WriteStatus {
    /// The transaction committed and the new revision is returned.
    Written,
    /// The expected revision did not match the stored revision.
    RevisionConflict,
    /// Request identity or state transition was invalid.
    Rejected,
    /// The database was unavailable or a transaction could not be committed.
    Unavailable,
}

/// Receipt returned after a write attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct InventoryWriteResult {
    /// Typed write outcome.
    pub status: WriteStatus,
    /// New revision after a successful commit, or the current revision on conflict.
    pub revision: Option<u64>,
    /// Time the Inventory handled the write.
    pub recorded_at: DateTime,
}

impl InventoryWriteResult {
    pub(super) fn new(status: WriteStatus, revision: Option<u64>, recorded_at: DateTime) -> Self {
        Self {
            status,
            revision,
            recorded_at,
        }
    }
}

/// Sanitized failures opening an Inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InventoryError {
    /// The configured directory or account scope is invalid.
    InvalidConfiguration,
    /// The database cannot be opened or validated.
    Unavailable,
}

impl std::fmt::Display for InventoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Inventory could not be opened: {self:?}")
    }
}

impl std::error::Error for InventoryError {}

/// Local path serde that preserves Unix path bytes in persisted JSON.
mod path_serde {
    use crate::models::{Evidence, Path};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::path::PathBuf;

    #[cfg(unix)]
    fn to_bytes(path: &Path) -> Vec<u8> {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().to_vec()
    }

    #[cfg(not(unix))]
    fn to_bytes(path: &Path) -> Result<Vec<u8>, &'static str> {
        path.to_str()
            .map(|value| value.as_bytes().to_vec())
            .ok_or("Path cannot be represented as UTF-8 on this platform.")
    }

    #[cfg(unix)]
    fn from_bytes(bytes: Vec<u8>) -> PathBuf {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;
        PathBuf::from(OsString::from_vec(bytes))
    }

    #[cfg(not(unix))]
    fn from_bytes(bytes: Vec<u8>) -> Result<PathBuf, &'static str> {
        String::from_utf8(bytes)
            .map(PathBuf::from)
            .map_err(|_| "Persisted path is invalid UTF-8 on this platform.")
    }

    pub(super) mod option {
        use super::*;

        pub fn serialize<S>(path: &Option<Path>, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            #[cfg(unix)]
            let bytes = path.as_ref().map(to_bytes);
            #[cfg(not(unix))]
            let bytes = path
                .as_ref()
                .map(to_bytes)
                .transpose()
                .map_err(serde::ser::Error::custom)?;
            bytes.serialize(serializer)
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Path>, D::Error>
        where
            D: Deserializer<'de>,
        {
            let bytes = Option::<Vec<u8>>::deserialize(deserializer)?;
            #[cfg(unix)]
            let path = bytes.map(from_bytes);
            #[cfg(not(unix))]
            let path = bytes
                .map(from_bytes)
                .transpose()
                .map_err(serde::de::Error::custom)?;
            Ok(path)
        }
    }

    pub(super) mod evidence {
        use super::*;

        pub fn serialize<S>(evidence: &Evidence<Path>, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let encoded = match evidence {
                Evidence::Known(path) => {
                    #[cfg(unix)]
                    {
                        Evidence::Known(to_bytes(path))
                    }
                    #[cfg(not(unix))]
                    {
                        Evidence::Known(to_bytes(path).map_err(serde::ser::Error::custom)?)
                    }
                }
                Evidence::NotCollected => Evidence::NotCollected,
                Evidence::NotApplicable => Evidence::NotApplicable,
                Evidence::Unsupported => Evidence::Unsupported,
                Evidence::Unavailable => Evidence::Unavailable,
                Evidence::PermissionDenied => Evidence::PermissionDenied,
            };
            encoded.serialize(serializer)
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<Evidence<Path>, D::Error>
        where
            D: Deserializer<'de>,
        {
            let encoded = Evidence::<Vec<u8>>::deserialize(deserializer)?;
            match encoded {
                Evidence::Known(bytes) => {
                    #[cfg(unix)]
                    {
                        Ok(Evidence::Known(from_bytes(bytes)))
                    }
                    #[cfg(not(unix))]
                    {
                        Ok(Evidence::Known(
                            from_bytes(bytes).map_err(serde::de::Error::custom)?,
                        ))
                    }
                }
                Evidence::NotCollected => Ok(Evidence::NotCollected),
                Evidence::NotApplicable => Ok(Evidence::NotApplicable),
                Evidence::Unsupported => Ok(Evidence::Unsupported),
                Evidence::Unavailable => Ok(Evidence::Unavailable),
                Evidence::PermissionDenied => Ok(Evidence::PermissionDenied),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn baseline_serialization_preserves_non_utf8_paths() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let state = LocalState {
            state: FileState::Present,
            path: Some(Path::from(OsString::from_vec(vec![b'a', 0xff]))),
            name: Evidence::Known("a".into()),
            parent: Evidence::Known(Path::from(OsString::from_vec(vec![0xfe]))),
            kind: Evidence::Known(FileKind::Regular),
            identity: Evidence::NotCollected,
            size: Evidence::Known(1),
            modified_at: Evidence::NotCollected,
            changed_at: Evidence::NotCollected,
            checksum: Evidence::NotCollected,
            capabilities: Evidence::NotCollected,
        };
        let encoded = serde_json::to_vec(&state).unwrap();
        let decoded: LocalState = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, state);
    }
}

/// Account/root enumeration is all-or-error; an unreadable row never becomes absence.
#[derive(Debug, Clone)]
pub(crate) struct InventoryScopeQuery {
    pub account_id: AccountId,
    pub sync_root_id: SyncRootId,
}
