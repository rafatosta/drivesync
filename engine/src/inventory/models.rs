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
    /// Last confirmed location, independent of historical synchronization baseline.
    #[serde(with = "path_serde::option")]
    pub local_path: Option<Path>,
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
impl RemoteState {
    /// Current binary content still matches the baseline; later metadata-only versions are allowed.
    /// This is evidence for an already bound slot, never permission to associate new items.
    pub(crate) fn unchanged_binary(&self, previous: &Self) -> bool {
        self.id.is_some()
            && self.id == previous.id
            && self.state == FileState::Present
            && previous.state == FileState::Present
            && self.trashed == Evidence::Known(false)
            && self.name == previous.name
            && self.parents == previous.parents
            && self.mime_type == previous.mime_type
            && matches!(&self.mime_type,Evidence::Known(mime) if !mime.starts_with("application/vnd.google-apps."))
            && matches!((&self.version,&previous.version),(Evidence::Known(now),Evidence::Known(before)) if now >= before)
            && matches!((&self.size,&previous.size),(Evidence::Known(now),Evidence::Known(before)) if now == before)
            && matches!((&self.checksum,&previous.checksum),(Evidence::Known(now),Evidence::Known(before)) if now == before && now.algorithm == "sha256" && !now.value.is_empty())
    }
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
    /// Immutable dispatch inputs and provider/FS references required after interruption.
    pub recovery: Option<Box<RecoveryRecord>>,
    /// Last conservative reconciliation outcome, including blocked uncertainty.
    pub recovery_diagnostic: Option<RecoveryDiagnostic>,
    /// Time at which the operation was first recorded.
    pub created_at: DateTime,
    /// Time at which the operation state was last changed.
    pub updated_at: DateTime,
}

pub(crate) use crate::models::OperationAction;

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
    /// Store a revalidated location without recording a synchronized baseline.
    ItemRegistered {
        #[serde(with = "path_serde::option")]
        local_path: Option<Path>,
    },
    /// Atomically store verified convergence and its directory association dependency.
    ConvergenceConfirmed {
        baseline: Baseline,
        effect: Box<AppliedEffect>,
        parent: Option<ParentAssociation>,
    },
    /// Record a newly planned or started operation.
    Pending(PendingOperation),
    /// Persist a transfer or staged-object checkpoint before publication/continuation.
    RecoveryCheckpoint(RecoveryCheckpoint),
    /// Record a read-based recovery assessment without replaying the command.
    RecoveryObserved {
        outcome: crate::models::ExecutionEffect,
        issue: Option<RecoveryIssue>,
        effect: Option<Box<AppliedEffect>>,
    },
    /// Record an observed operation effect that may require later confirmation.
    OperationApplied(AppliedEffect),
    /// Confirm a verified individual effect without asserting whole-item convergence.
    OperationConfirmed,
    /// Replace the baseline after the caller has confirmed the resulting states.
    BaselineConfirmed {
        baseline: Baseline,
        parent: Option<ParentAssociation>,
    },
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

use crate::models::path_serde;

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
        let parent = ParentAssociation {
            item_id: crate::models::ItemId("parent".into()),
            revision: 1,
            remote_id: RemoteId("remote".into()),
            local_path: state.path.clone().unwrap(),
        };
        assert_eq!(
            serde_json::from_slice::<ParentAssociation>(&serde_json::to_vec(&parent).unwrap())
                .unwrap(),
            parent
        );
    }
}

/// Account/root enumeration is all-or-error; an unreadable row never becomes absence.
#[derive(Debug, Clone)]
pub(crate) struct InventoryScopeQuery {
    pub account_id: AccountId,
    pub sync_root_id: SyncRootId,
}

/// Optimistic dependency checked within the same transaction as a child baseline write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ParentAssociation {
    pub item_id: crate::models::ItemId,
    pub revision: u64,
    pub remote_id: RemoteId,
    #[serde(with = "path_serde::required")]
    pub local_path: Path,
}

/// Stable, credential-free inputs to one dispatched operation, scoped to its configured roots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RecoveryRecord {
    pub account_id: AccountId,
    pub provider_id: crate::models::ProviderId,
    pub sync_root_id: SyncRootId,
    #[serde(with = "path_serde::required")]
    pub local_root: Path,
    pub remote_root_id: RemoteId,
    pub root_identity: LocalIdentity,
    pub parent_identity: Option<LocalIdentity>,
    /// Pinned destination directory for local movement recovery.
    pub destination_parent_identity: Option<LocalIdentity>,
    pub command: crate::models::Operation,
    pub execution_id: Id,
    pub identity: FileIdentity,
    pub expected_revision: Option<u64>,
    pub local: LocalState,
    pub remote: RemoteState,
    pub completed_steps: Vec<u64>,
    /// Server-reserved identity, committed before a create or upload dispatch.
    pub reserved_remote_id: Option<RemoteId>,
    #[serde(with = "path_serde::option")]
    pub stage_path: Option<Path>,
    /// Identity committed before publishing a tagged staged entry.
    pub stage_identity: Option<LocalIdentity>,
    pub transfer: Option<TransferReference>,
    /// Ancestor facts committed before a tree-location mutation.
    pub remote_ancestry: Option<Vec<RemoteState>>,
}
/// Sensitive provider transfer handle. No OAuth credential is stored; Debug redacts the URI.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TransferReference {
    pub session_uri: String,
    pub total_bytes: u64,
    pub confirmed_offset: u64,
}
impl std::fmt::Debug for TransferReference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransferReference")
            .field("total_bytes", &self.total_bytes)
            .field("confirmed_offset", &self.confirmed_offset)
            .finish_non_exhaustive()
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum RecoveryCheckpoint {
    LocalStage(LocalIdentity),
    Transfer(TransferReference),
    RemoteAncestry(Vec<RemoteState>),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum RecoveryIssue {
    MissingCommand,
    UnsupportedAction,
    ScopeMismatch,
    RootChanged,
    ParentChanged,
    SourceChanged,
    DestinationChanged,
    RemoteInconclusive,
    TransferIncomplete,
    TransferExpired,
    NotDispatched,
    InventoryConflict,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RecoveryDiagnostic {
    pub outcome: crate::models::ExecutionEffect,
    pub issue: Option<RecoveryIssue>,
    pub checked_at: DateTime,
}
