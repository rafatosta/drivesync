//! Typed inventory requests and durable domain state; no I/O or synchronization decisions.
use super::InventoryManager;
use crate::models::{
    AccountId, File, LocalFile, LocalFileId, LocalRelativePath, RemoteFile, RemoteFileRef, SyncId,
    Timestamp,
};
use serde::{Deserialize, Serialize};
pub type InventoryResult<T> = Result<T, InventoryFailure>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Logical identity and observed state; neither presence nor status executes an operation.
#[derive(Hash, PartialOrd, Ord)]
pub struct SyncItemId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, PartialOrd, Ord)]
pub struct OperationId(pub i64);
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, PartialOrd, Ord)]
pub struct ActivityId(pub i64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pagination {
    pub limit: u32,
    pub after: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page<T> {
    pub entries: Vec<T>,
    pub next_cursor: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemKind {
    File,
    Directory,
    Symlink,
    Other,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresenceState {
    Unknown,
    Present,
    AbsentConfirmed,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncStatus {
    PendingReconciliation,
    PendingUpload,
    PendingDownload,
    Synced,
    Conflict,
    Error,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationSource {
    LocalScan,
    LocalEvent,
    RemoteListing,
    RemoteChange,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Caller-supplied evidence context, never a proof inferred from a missing row.
///
/// Inventory accepts observations independently per side. Local observations
/// use positions from one durable synchronization-wide counter. Numeric remote
/// provider versions take priority; otherwise remote observations require a
/// comparable captured causal position. A lower marker is recorded as an
/// obsolete receipt and does not change item metadata, presence or revision.
/// Wall-clock timestamps never authorize freshness.
/// Exact event redelivery is resolved by its durable receipt before either
/// freshness or optimistic revision checks.
pub struct ObservationProof {
    pub source: ObservationSource,
    pub observed_at: Timestamp,
    pub scan_or_generation: Option<String>,
    pub complete_scope: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Explicit patch intent; absence and Keep are not commands to clear data.
pub enum FieldUpdate<T> {
    Keep,
    Set(T),
    Clear,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Logical identity and observed state; neither presence nor status executes an operation.
pub struct SyncItem {
    pub id: SyncItemId,
    pub kind: ItemKind,
    pub local_file_id: Option<LocalFileId>,
    pub remote_ref: Option<RemoteFileRef>,
    pub local: Option<LocalFile>,
    pub remote: Option<RemoteFile>,
    /// Most recent local collection, including failures and uncollected fields.
    pub local_latest: Option<LocalFile>,
    /// Most recent remote collection, distinct from retained known values.
    pub remote_latest: Option<RemoteFile>,
    pub baseline: Option<SyncBaseline>,
    pub local_presence: PresenceState,
    pub remote_presence: PresenceState,
    pub status: SyncStatus,
    pub conflict_reason: Option<String>,
    pub revision: u64,
    pub last_checked_at: Option<Timestamp>,
    pub last_synced_at: Option<Timestamp>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Only the last verified synchronized facts, distinct from current observations.
pub struct SyncBaseline {
    pub local: BaselineSide,
    pub remote: BaselineSide,
    pub confirmed_at: Timestamp,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaselineSide {
    pub presence: PresenceState,
    pub name: Option<String>,
    pub local_name_bytes: Option<Vec<u8>>,
    pub parent_identity: Option<BaselineParent>,
    pub version: Option<String>,
    pub checksum: Option<String>,
    pub size: Option<u64>,
    pub modified_at: Option<Timestamp>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncInfo {
    pub sync_id: SyncId,
    pub account_id: AccountId,
    pub remote_root_id: String,
    pub generation: u64,
    pub initial_download_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationDirection {
    Upload,
    Download,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationKind {
    Create,
    UpdateContent,
    Rename,
    Move,
    Trash,
    Delete,
    Restore,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationState {
    Pending,
    Running,
    RetryWait,
    Blocked,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Durable work request and state, without a transfer worker.
pub struct SyncOperation {
    pub id: OperationId,
    pub item_id: SyncItemId,
    pub direction: OperationDirection,
    pub kind: OperationKind,
    pub state: OperationState,
    pub base_item_revision: u64,
    pub expected_remote_version: Option<String>,
    pub attempt_count: u32,
    pub next_attempt_at: Option<Timestamp>,
    pub last_error_kind: Option<String>,
    pub created_at: Timestamp,
    pub started_at: Option<Timestamp>,
    pub completed_at: Option<Timestamp>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivityType {
    Upload,
    Download,
    Rename,
    Move,
    Create,
    Trash,
    Delete,
    Restore,
    ConflictDetected,
    ConflictResolved,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivityOrigin {
    Local,
    Remote,
    System,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivityOutcome {
    Completed,
    Failed,
    Cancelled,
    Blocked,
    Informational,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncActivity {
    pub id: ActivityId,
    pub item_id: SyncItemId,
    pub operation_id: Option<OperationId>,
    pub event_type: ActivityType,
    pub origin: ActivityOrigin,
    pub outcome: ActivityOutcome,
    pub occurred_at: Timestamp,
    pub description: Option<String>,
    pub error_kind: Option<String>,
    pub item_name: Option<String>,
    pub item_remote_id: Option<String>,
    pub item_local_path: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenInventoryRequest {
    pub local_root: std::path::PathBuf,
    pub expected: SyncIdentity,
    pub create_if_missing: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncIdentity {
    pub sync_id: SyncId,
    pub account_id: AccountId,
    pub remote_root_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InventoryRecoveryOutcome {
    Normal,
    Restored,
    Rebuilt,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InventoryAvailability {
    Ready,
    Recovering,
    Unavailable,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryStatusRequest {}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Cached operational state; Ready is not a guarantee that the next SQL call succeeds.
pub struct InventoryStatusResult {
    pub availability: InventoryAvailability,
    pub recovery: InventoryRecoveryOutcome,
    pub last_error: Option<InventoryError>,
}

pub struct OpenInventoryResult {
    pub inventory: InventoryManager,
    pub info: SyncInfo,
    pub created: bool,
    pub recovery: InventoryRecoveryOutcome,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetSyncInfoRequest {}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetSyncInfoResult {
    pub info: SyncInfo,
    /// Exact Unix bytes of the configured absolute root, for producer scope validation.
    pub local_root: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkInitialDownloadCompleteRequest {
    pub expected_generation: u64,
    pub expected_complete: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkInitialDownloadCompleteResult {
    pub info: SyncInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetItemRequest {
    pub selector: ItemSelector,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemSelector {
    Id(SyncItemId),
    RemoteRef(RemoteFileRef),
    LocalId(LocalFileId),
    LocalRelativePath(LocalRelativePath),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetItemResult {
    pub item: Option<SyncItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListItemsRequest {
    pub filter: ItemFilter,
    pub pagination: Pagination,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ItemFilter {
    pub statuses: Vec<SyncStatus>,
    pub local_presence: Option<PresenceState>,
    pub remote_presence: Option<PresenceState>,
    pub remote_parent: Option<RemoteFileRef>,
    pub local_parent_item_id: Option<SyncItemId>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListItemsResult {
    pub page: Page<SyncItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordLocalObservation {
    pub selector: ObservationTarget,
    pub observation: LocalObservation,
    pub expected_revision: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalObservation {
    pub observed: File,
    pub presence: PresenceState,
    pub proof: ObservationProof,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordRemoteObservation {
    pub selector: ObservationTarget,
    pub observation: RemoteObservation,
    pub expected_revision: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteObservation {
    pub observed: File,
    pub presence: PresenceState,
    pub proof: ObservationProof,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationTarget {
    Existing(SyncItemId),
    ByRemoteRef(RemoteFileRef),
    ByLocalId(LocalFileId),
    NewItem,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordObservationResult {
    pub item: SyncItem,
    pub created: bool,
    pub disposition: ObservationDisposition,
    /// Exact receipt replay; durable revalidation or analysis work may still be pending.
    pub duplicate: bool,
}

/// Freshness decision persisted with every accepted or rejected event receipt.
///
/// `Applied` requires a greater durable causal position on that side, or a greater
/// numeric provider version for remote observations. `Obsolete` requires a smaller
/// comparable marker. Without comparable evidence the observation is `Incomparable`;
/// its payload and revision are preserved and the responsible detector is asked to
/// revalidate the affected scope. Wall-clock timestamps never decide freshness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationDisposition {
    Applied,
    Obsolete,
    Incomparable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkItemRequest {
    pub item_id: SyncItemId,
    pub changes: ItemLinksChange,
    pub expected_revision: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemLinksChange {
    pub local_file_id: FieldUpdate<LocalFileId>,
    pub remote_ref: FieldUpdate<RemoteFileRef>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkItemResult {
    pub item: SyncItem,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateItemStatusRequest {
    pub item_id: SyncItemId,
    pub change: ItemStatusChange,
    pub expected_revision: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemStatusChange {
    pub status: SyncStatus,
    pub conflict_reason: FieldUpdate<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateItemStatusResult {
    pub item: SyncItem,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmSyncRequest {
    pub item_id: SyncItemId,
    pub expected_item_revision: u64,
    pub confirmed: ConfirmedState,
    pub completed_operation: Option<OperationId>,
    /// Claim attempt being confirmed; required exactly when completing an operation.
    pub expected_operation_attempt: Option<u32>,
    pub activity: Option<NewActivity>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmedState {
    pub baseline: SyncBaseline,
    pub status: SyncStatus,
    pub verified_at: Timestamp,
    /// Durable causal position reserved immediately before local verification.
    pub local_causal_order: Option<u64>,
    /// Durable causal position reserved immediately before remote verification.
    pub remote_causal_order: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmSyncResult {
    pub item: SyncItem,
    pub completed_operation: Option<SyncOperation>,
    pub activity: Option<SyncActivity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnqueueOperationRequest {
    pub item_id: SyncItemId,
    pub spec: NewSyncOperation,
    pub expected_item_revision: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewSyncOperation {
    pub direction: OperationDirection,
    pub kind: OperationKind,
    pub expected_remote_version: Option<String>,
    pub created_at: Timestamp,
    pub deduplication_key: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnqueueOperationResult {
    pub operation: SyncOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListOperationsRequest {
    pub filter: OperationFilter,
    pub pagination: Pagination,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct OperationFilter {
    pub item_id: Option<SyncItemId>,
    pub states: Vec<OperationState>,
    pub direction: Option<OperationDirection>,
    pub eligible_before: Option<Timestamp>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListOperationsResult {
    pub page: Page<SyncOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimOperationRequest {
    pub operation_id: OperationId,
    pub expected_state: OperationState,
    pub claimed_at: Timestamp,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimOperationResult {
    pub operation: SyncOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateOperationRequest {
    pub operation_id: OperationId,
    /// Rejects a stale worker after the same operation has been claimed again.
    pub expected_attempt_count: u32,
    pub expected_state: OperationState,
    pub transition: OperationTransition,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationTransition {
    Retry {
        next_attempt_at: Timestamp,
        error_kind: String,
    },
    Block {
        error_kind: String,
    },
    Fail {
        failed_at: Timestamp,
        error_kind: String,
    },
    Requeue {
        reason: String,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateOperationResult {
    pub operation: SyncOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewActivity {
    pub item_id: SyncItemId,
    pub operation_id: Option<OperationId>,
    pub event_type: ActivityType,
    pub origin: ActivityOrigin,
    pub outcome: ActivityOutcome,
    pub occurred_at: Timestamp,
    pub description: Option<String>,
    pub error_kind: Option<String>,
    pub item_name: Option<String>,
    pub item_remote_id: Option<String>,
    pub item_local_path: Option<Vec<u8>>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordActivityRequest {
    pub activity: NewActivity,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordActivityResult {
    pub activity: SyncActivity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListActivitiesRequest {
    pub filter: ActivityFilter,
    pub pagination: Pagination,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ActivityFilter {
    pub item_id: Option<SyncItemId>,
    pub operation_id: Option<OperationId>,
    pub event_types: Vec<ActivityType>,
    pub outcomes: Vec<ActivityOutcome>,
    pub from: Option<Timestamp>,
    pub until: Option<Timestamp>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListActivitiesResult {
    pub page: Page<SyncActivity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Sanitized inventory failures without paths, SQL or provider payloads.
pub enum InventoryError {
    InvalidArgument,
    InvalidIdentity,
    NotFound,
    AlreadyExists,
    DuplicateAssociation,
    VersionMismatch,
    InvalidTransition,
    UnconfirmedAbsence,
    MissingObservation,
    MissingActivity,
    InvalidCursor,
    CorruptDatabase,
    UnsupportedSchema,
    PermissionDenied,
    StorageUnavailable,
    StorageFull,
    TransactionFailed,
    SerializationFailed,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryFailure {
    pub kind: InventoryError,
    pub message: String,
    pub retryable: bool,
}

/// Historical parent identity qualified by the corresponding side.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BaselineParent {
    Local(SyncItemId),
    Remote(RemoteFileRef),
}

impl InventoryFailure {
    pub(super) fn new(kind: InventoryError) -> Self {
        let retryable = matches!(
            kind,
            InventoryError::StorageUnavailable | InventoryError::StorageFull
        );
        Self {
            kind,
            message: "inventory operation failed".into(),
            retryable,
        }
    }
}
impl std::fmt::Display for InventoryFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.kind)
    }
}
impl std::error::Error for InventoryFailure {}
impl Default for Pagination {
    fn default() -> Self {
        Self {
            limit: 100,
            after: None,
        }
    }
}

impl Default for ItemLinksChange {
    fn default() -> Self {
        Self {
            local_file_id: FieldUpdate::Keep,
            remote_ref: FieldUpdate::Keep,
        }
    }
}
