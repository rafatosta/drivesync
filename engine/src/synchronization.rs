//! Stable, credential-free synchronization lifecycle and status boundary for applications.
use crate::models::{AccountId, DateTime, ItemId, Path, RemoteId};
use tokio::sync::watch;

/// One account's configured local and remote synchronization roots.
/// Starting activates read-only observation producers and the existing coordination contracts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartSynchronizationRequest {
    pub account_id: AccountId,
    pub local_root: Path,
    pub remote_root_id: RemoteId,
}
/// Account-facing lifecycle, independent of any provider or GUI toolkit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynchronizationState {
    Idle,
    Running,
    Waiting,
    Stopped,
    Failed,
}
/// Actual current work, including the functional initial local scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynchronizationActivity {
    None,
    Observing,
    Scanning,
    Analyzing,
    /// Checking a durable operation against current evidence after an interruption.
    Recovering,
    Uploading,
    Downloading,
    Applying,
    /// Applying a name-only change to an existing replica.
    Renaming,
    /// Applying a parent change to an existing replica.
    Moving,
    /// Creating a directory replica.
    CreatingDirectory,
    /// Creating or updating a derived Workspace shortcut on the computer.
    ProjectingWorkspace,
    /// Moving an associated replica to recoverable trash.
    TrashingLocal,
    TrashingRemote,
}
/// Display-safe identity; paths retain platform-native bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SynchronizationItem {
    pub item_id: Option<ItemId>,
    pub local_path: Option<Path>,
    /// Observed name when a local path is unavailable.
    pub display_name: Option<String>,
    pub remote_id: Option<RemoteId>,
}
/// Completed, durably confirmed steps within the current item's plan, not transfer bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynchronizationProgress {
    pub completed: u64,
    pub total: u64,
}
/// Stable blocker categories; callers should localize these rather than backend diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynchronizationIssueKind {
    Deferred,
    DeferredFor(SynchronizationWait),
    Conflict,
    AdoptionRequired,
    Unsupported(Option<SynchronizationProjectionIssue>),
    InvalidInput,
    OperationFailed,
    PreconditionFailed,
    RecoveryRequired,
    WorkerFailed,
    TrashRefused,
    TrashRecoveryRequired,
}
/// Evidence the worker needs before it can continue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynchronizationWait {
    LocalObservation,
    RemoteObservation,
    FreshObservations,
    CompleteObservations,
    /// A confirmed item is awaiting enough current evidence to verify its state.
    ItemVerification {
        source: SynchronizationVerificationSource,
        confirmed_action: Option<SynchronizationConfirmedAction>,
    },
}

/// Side whose current evidence is insufficient for verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynchronizationVerificationSource {
    Local,
    Remote,
    Both,
    Unspecified,
}

/// The last confirmed operation when it was a file transfer or folder creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynchronizationConfirmedAction {
    Download,
    Upload,
    CreateLocalDirectory,
    CreateRemoteDirectory,
}

/// Why a file could not be represented or verified for synchronization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynchronizationProjectionIssue {
    SpecialLocalFile,
    InvalidLocalName,
    InvalidRemoteName,
    DriveShortcut,
    UnsupportedDriveType,
    NameCollision,
    OccupiedWorkspacePath,
    MissingRemoteContentEvidence,
}

/// Current blocker without credentials, raw server text, or observation payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynchronizationIssue {
    pub kind: SynchronizationIssueKind,
    pub retryable: bool,
}
/// Public identity of the operation currently dispatched to the executor.
/// This does not imply byte telemetry or a lossless sequence of operation events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynchronizationOperation {
    pub id: String,
    pub item_id: ItemId,
    pub display_name: Option<String>,
    /// This download follows a verified registration of a newly observed remote identity.
    pub newly_observed_remote: bool,
    /// This upload follows a verified registration of a newly observed local identity.
    pub newly_observed_local: bool,
    /// The operation replaces the content of an existing local replica.
    /// Downloading covers transfer and verification. Applying denotes a verified,
    /// persisted update while observations are refreshed.
    pub updates_local_content: bool,
    /// Replaces an existing Drive replica. Applying denotes a verified, persisted update.
    pub updates_remote_content: bool,
    /// Name change on the destination replica. Applying denotes a verified, persisted rename.
    pub rename: Option<SynchronizationRename>,
    /// Parent change on the destination replica. Applying denotes a verified, persisted move.
    pub movement: Option<SynchronizationMove>,
    /// Derived Workspace shortcut action. Applying denotes a verified, persisted operation.
    pub workspace_shortcut: Option<WorkspaceShortcutActivity>,
    /// Directory creation destination. Applying denotes a verified, persisted creation.
    pub directory_creation: Option<SynchronizationDirectoryDestination>,
    /// Trash destination. Applying denotes a verified, persisted trash operation.
    pub trash: Option<SynchronizationTrashDestination>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceShortcutActivity {
    Create,
    Update,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynchronizationTrashDestination {
    Local,
    Remote,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynchronizationDirectoryDestination {
    Local,
    Remote,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynchronizationMove {
    pub origin: SynchronizationMoveOrigin,
    /// Source directory relative to the synchronized root.
    pub source_parent: std::path::PathBuf,
    /// Destination directory relative to the synchronized root.
    pub destination_parent: std::path::PathBuf,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynchronizationMoveOrigin {
    Local,
    Remote,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynchronizationRename {
    pub origin: SynchronizationRenameOrigin,
    pub previous_name: String,
    pub new_name: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynchronizationRenameOrigin {
    Local,
    Remote,
}
/// Latest account-scoped synchronization state. Idle does not assert whole-tree convergence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynchronizationStatus {
    pub account_id: AccountId,
    pub state: SynchronizationState,
    pub activity: SynchronizationActivity,
    pub current_item: Option<SynchronizationItem>,
    pub current_operation: Option<SynchronizationOperation>,
    pub progress: Option<SynchronizationProgress>,
    pub issue: Option<SynchronizationIssue>,
    pub updated_at: DateTime,
}
impl SynchronizationStatus {
    pub(crate) fn new(account_id: AccountId, state: SynchronizationState) -> Self {
        Self {
            account_id,
            state,
            activity: SynchronizationActivity::None,
            current_item: None,
            current_operation: None,
            progress: None,
            issue: None,
            updated_at: crate::orchestrator::timestamp(),
        }
    }
}
/// Coalescing latest-state subscription. Awaiting changes does not require a caller Tokio runtime.
/// Dropping a subscription stops listening without stopping the account's worker.
pub struct SynchronizationSubscription {
    pub(crate) receiver: watch::Receiver<SynchronizationStatus>,
}
impl SynchronizationSubscription {
    /// Reads the latest state and marks it observed by this subscriber.
    pub fn current(&mut self) -> SynchronizationStatus {
        self.receiver.borrow_and_update().clone()
    }
    /// Waits for a newer snapshot; None means the worker's publication channel has closed.
    pub async fn changed(&mut self) -> Option<SynchronizationStatus> {
        self.receiver.changed().await.ok()?;
        Some(self.current())
    }
}
/// Lifecycle failures without provider or storage implementation details.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynchronizationErrorKind {
    InvalidConfiguration,
    AccountUnavailable,
    UnsupportedProvider,
    InventoryUnavailable,
    LocalRootUnavailable,
    LocalRootIdentityChanged,
    AlreadyRunning,
    RootInUse,
    NotStarted,
    Stopped,
    InvalidObservation,
}

/// Convergence proof for complete captured source rounds at a specific effect epoch.
/// This is not a promise that external files cannot change after the captured horizon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynchronizationAudit {
    /// Exact configured scope whose captured horizon was audited.
    pub roots: StartSynchronizationRequest,
    pub initial_complete: bool,
    pub coverage_complete: bool,
    pub epoch: u64,
    pub local_entries: usize,
    pub remote_entries: usize,
    pub inventory_items: usize,
    pub converged_items: usize,
    pub unconfirmed_operations: usize,
    pub unresolved_conflicts: usize,
}
