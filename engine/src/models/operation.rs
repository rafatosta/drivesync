//! Normalized command values shared with durable execution storage; no execution or analysis.
use super::*;
/// Operation categories shared by pending Inventory records and the executor contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum OperationAction {
    /// Persist a new unilateral identity without asserting convergence.
    RegisterIdentity,
    /// Confirm an explicitly authorized initial association without transfer.
    Adopt,
    /// Record revalidated convergence for an already associated item.
    ConfirmBaseline,
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

/// One ordered operation in a proposed plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Operation {
    /// One-based order within the plan.
    pub step: u64,
    /// Requested normalized operation.
    pub action: OperationAction,
    /// Logical item identity.
    pub item_ref: ItemId,
    /// Earlier operation steps that must complete first.
    pub depends_on: Vec<u64>,
    /// Provider item identity when known.
    pub remote_id: Option<RemoteId>,
    /// Current or target local relative path, when applicable.
    #[serde(with = "super::path_serde::option")]
    pub local_path: Option<Path>,
    /// Target name for a creation or rename.
    pub name: Option<String>,
    /// Target remote parent for a move or creation.
    pub parent_id: Option<RemoteId>,
    /// Target local parent for a move or creation.
    #[serde(with = "super::path_serde::option")]
    pub parent: Option<Path>,
    /// Preconditions that the executor must recheck before applying the operation.
    pub preconditions: Option<OperationPreconditions>,
}

/// Snapshot stability requirements for a planned operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct OperationPreconditions {
    /// Remote version observed during analysis.
    pub observed_remote_version: Option<u64>,
    /// Remote version recorded in the baseline.
    pub baseline_remote_version: Option<u64>,
    /// Whether the local source must remain unchanged until execution.
    pub local_unchanged: Evidence<bool>,
    /// Whether the remote source must remain unchanged until execution.
    pub remote_unchanged: Evidence<bool>,
    /// Whether the destination was confirmed free during analysis.
    pub destination_free: Evidence<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ExecutionEffect {
    NotApplied,
    Applied,
    Unknown,
}
