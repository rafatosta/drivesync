//! Owned command data; no I/O, provider protocol or Analyzer/Inventory dependencies.
use super::{CancellationSignal, ExecutionContext, ProgressReporter};
use crate::{
    models::{
        AccountId, FileData, LocalFileId, LocalRelativePath, ProviderId, RemoteFileId,
        RemoteFileRef, SyncId,
    },
    Engine,
};
use serde::{Deserialize, Serialize};
use std::{ffi::OsString, path::PathBuf, sync::Arc};

/// A typed failure, never a synthetic successful execution result.
pub type OperatorResult<T> = Result<T, OperatorFailure>;

/// The factory resolves the provider from this persisted account through Engine.
/// No credential, public concrete executor or user-chosen provider is injected.
#[derive(Clone)]
pub struct OperatorConfig {
    pub connector: Engine,
    pub scope: OperatorScope,
    /// Blocking Inventory-owned persistence, called only on a blocking worker.
    pub journal: Arc<dyn super::ExecutionJournal>,
}

/// Authority boundary for every local and remote reference in a command.
/// Paths remain untrusted until an executor validates them against this scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorScope {
    pub sync_id: SyncId,
    pub account_id: AccountId,
    pub local_root: PathBuf,
    pub remote_root_id: RemoteFileId,
}

/// Identifies a resource, not an association inferred from a name.
/// Local identity is optional evidence; missing identity never proves equivalence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceRef {
    Local {
        relative_path: LocalRelativePath,
        local_id: Option<LocalFileId>,
    },
    Remote {
        provider_id: ProviderId,
        reference: RemoteFileRef,
    },
}

/// An explicitly identified parent, always interpreted inside OperatorScope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DirectoryRef {
    /// The configured local root; an empty LocalRelativePath is never fabricated.
    LocalRoot,
    Local {
        relative_path: LocalRelativePath,
        local_id: Option<LocalFileId>,
    },
    Remote {
        provider_id: ProviderId,
        reference: RemoteFileRef,
    },
}

/// Preserve arbitrary Unix filename bytes; remote names are Unicode strings.
/// The executor must reject the wrong name representation for its destination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemName {
    Local(OsString),
    Remote(String),
}

/// Content is an explicit resource or bounded in-memory input. File inputs allow
/// future streaming without exposing provider byte streams through this contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentSource {
    LocalFile {
        relative_path: LocalRelativePath,
        local_id: Option<LocalFileId>,
    },
    RemoteFile {
        resource: ResourceRef,
    },
    Bytes {
        data: Vec<u8>,
    },
}

/// Expected resource versions; absence never grants implicit overwrite authority.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preconditions {
    pub expected_version: Option<String>,
    pub expected_revision: Option<String>,
}

/// Independent optional per-call ports. No cancellation or progress engine is
/// implemented here, and neither port establishes completion or verification.
#[derive(Clone, Default)]
pub struct ExecutionOptions {
    pub cancellation: Option<Arc<dyn CancellationSignal>>,
    pub progress: Option<Arc<dyn ProgressReporter>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionPhase {
    Preparing,
    Transferring,
    Applying,
    Verifying,
    Finalizing,
}

/// Counters contain observed bytes, not estimates of synchronization completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionProgress {
    pub phase: ExecutionPhase,
    pub bytes_transferred: Option<u64>,
    pub total_bytes: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressDelivery {
    Accepted,
    Dropped,
}

#[derive(Clone)]
pub struct CreateFileRequest {
    pub execution: ExecutionContext,
    pub parent: DirectoryRef,
    pub name: ItemName,
    pub content: Option<ContentSource>,
    pub options: ExecutionOptions,
}
#[derive(Clone)]
pub struct CreateDirectoryRequest {
    pub execution: ExecutionContext,
    pub parent: DirectoryRef,
    pub name: ItemName,
    pub options: ExecutionOptions,
}
/// Source must be local and destination remote in the executor's account.
#[derive(Clone)]
pub struct UploadRequest {
    pub execution: ExecutionContext,
    pub source: ResourceRef,
    pub destination: DirectoryRef,
    pub name: ItemName,
    pub options: ExecutionOptions,
}
/// Source must be remote and destination local inside the configured scope.
#[derive(Clone)]
pub struct DownloadRequest {
    pub execution: ExecutionContext,
    pub source: ResourceRef,
    pub destination: DirectoryRef,
    pub name: ItemName,
    pub options: ExecutionOptions,
}
#[derive(Clone)]
pub struct UpdateContentRequest {
    pub execution: ExecutionContext,
    pub target: ResourceRef,
    pub source: ContentSource,
    pub preconditions: Preconditions,
    pub options: ExecutionOptions,
}
#[derive(Clone)]
pub struct RenameRequest {
    pub execution: ExecutionContext,
    pub target: ResourceRef,
    pub new_name: ItemName,
    pub preconditions: Preconditions,
    pub options: ExecutionOptions,
}
#[derive(Clone)]
pub struct MoveRequest {
    pub execution: ExecutionContext,
    pub target: ResourceRef,
    pub destination: DirectoryRef,
    pub preconditions: Preconditions,
    pub options: ExecutionOptions,
}
/// Kept callable by the contract, but permanent deletion is prohibited everywhere.
#[derive(Clone)]
pub struct DeleteRequest {
    pub execution: ExecutionContext,
    pub target: ResourceRef,
    pub preconditions: Preconditions,
    pub options: ExecutionOptions,
}
#[derive(Clone)]
pub struct TrashRequest {
    pub execution: ExecutionContext,
    pub target: ResourceRef,
    pub preconditions: Preconditions,
    pub options: ExecutionOptions,
}
#[derive(Clone)]
pub struct RestoreRequest {
    pub execution: ExecutionContext,
    pub target: ResourceRef,
    pub destination: Option<DirectoryRef>,
    pub preconditions: Preconditions,
    pub options: ExecutionOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
    Verified,
    Unverified,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntegrityStatus {
    Verified,
    NotAvailable,
}

/// Reports the identity actually observed after publication, including a changed
/// local physical identity after staging replacement. Never copies an old inode
/// identity to manufacture preservation or confirms a logical association.
/// Identity and observed metadata from Models; never fabricates a baseline,
/// SyncItem, association or fields not actually returned by the execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemResult {
    pub resource: ResourceRef,
    pub metadata: Option<FileData>,
    pub verification: VerificationStatus,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferResult {
    pub item: ItemResult,
    pub bytes_transferred: u64,
    pub integrity: IntegrityStatus,
}
/// Retained result shape, not permission to perform permanent deletion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteResult {
    pub target: ResourceRef,
    pub verification: VerificationStatus,
}

/// A cancelled or failed call must distinguish no effect from uncertain effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionEffect {
    NotApplied,
    Applied,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorError {
    InvalidConfiguration,
    InvalidRequest,
    UnsupportedProvider,
    UnsupportedOperation,
    PermanentDeletionProhibited,
    WrongScope,
    NotFoundOrInaccessible,
    PermissionDenied,
    DestinationOccupied,
    PreconditionFailed,
    IntegrityMismatch,
    InsufficientSpace,
    Io,
    Network,
    Cancelled,
    IndeterminateEffect,
    CheckpointConflict,
    CheckpointUnavailable,
    Internal,
}

/// Sanitized category and affected identity for later verification. No raw HTTP,
/// credentials or provider message text; Unknown never authorizes a blind retry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorFailure {
    pub kind: OperatorError,
    pub effect: ExecutionEffect,
    pub resource: Option<ResourceRef>,
}

/// Durable, unexecuted commands. Runtime options are supplied only upon future
/// execution. Permanent deletion deliberately has no representable variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationCommand {
    CreateFile {
        parent: DirectoryRef,
        name: ItemName,
        content: Option<ContentSource>,
    },
    CreateDirectory {
        parent: DirectoryRef,
        name: ItemName,
    },
    Upload {
        source: ResourceRef,
        destination: DirectoryRef,
        name: ItemName,
    },
    Download {
        source: ResourceRef,
        destination: DirectoryRef,
        name: ItemName,
    },
    UpdateContent {
        target: ResourceRef,
        source: ContentSource,
        preconditions: Preconditions,
    },
    Rename {
        target: ResourceRef,
        new_name: ItemName,
        preconditions: Preconditions,
    },
    Move {
        target: ResourceRef,
        destination: DirectoryRef,
        preconditions: Preconditions,
    },
    Trash {
        target: ResourceRef,
        preconditions: Preconditions,
    },
    Restore {
        target: ResourceRef,
        destination: Option<DirectoryRef>,
        preconditions: Preconditions,
    },
}

/// Explicit user request, bound by AnalyzeRequest to the exact item snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreSelection {
    pub decision_id: String,
    pub target: ResourceRef,
}
