//! Presentation values contain no synchronization implementation types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatusView {
    Initializing,
    WaitingForEvidence,
    Finalizing,
    Checking,
    VerificationIncomplete,
    Synchronizing,
    Recovering,
    Synced,
    Attention,
    Offline,
    Failed,
    Stopped,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProgressView {
    None,
    Indeterminate,
    Bytes { transferred: u64, total: u64 },
    Steps { completed: u64, total: u64 },
}
impl ProgressView {
    pub fn measured(&self) -> Option<(u64, u64)> {
        let (value, total) = match *self {
            Self::None | Self::Indeterminate => return None,
            Self::Bytes { transferred, total } => (transferred, total),
            Self::Steps { completed, total } => (completed, total),
        };
        (total > 0 && value <= total).then_some((value, total))
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActivityKind {
    CreateWorkspaceShortcut,
    UpdateWorkspaceShortcut,
    Download,
    DownloadDiscoveredRemote,
    UpdateLocalContent,
    UpdateRemoteContent,
    Rename {
        previous_name: String,
    },
    Move {
        source_parent: String,
        destination_parent: String,
    },
    Upload,
    UploadDiscoveredLocal,
    Verify,
    Recover,
    CreateDirectory,
    Trash,
    Apply,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Direction {
    RemoteToLocal,
    LocalToRemote,
    None,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    Running,
    Confirmed,
    Failed,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityView {
    pub scope: String,
    /// None when the public source does not publish an operation identity.
    pub operation_id: Option<String>,
    pub item_id: String,
    pub name: String,
    pub kind: ActivityKind,
    pub direction: Direction,
    pub phase: Phase,
    pub progress: ProgressView,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttentionAction {
    OpenConflictList,
    OpenIssue(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttentionItemView {
    pub scope: String,
    pub id: String,
    pub title: String,
    pub summary: String,
    pub action: AttentionAction,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
    Error,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssueView {
    pub severity: Severity,
    pub title: String,
    pub description: String,
    pub guidance: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConflictView {
    pub id: String,
    pub revision: u64,
    pub name: String,
    pub local: String,
    pub remote: String,
    pub differences: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConflictResolverState {
    Loading,
    Reviewing,
    Confirming,
    Submitting,
    Applying,
    Resolved,
    Stale,
    Failed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConflictChoice {
    KeepLocal,
    KeepRemote,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConflictChoiceRequest {
    pub id: String,
    pub revision: u64,
    pub choice: ConflictChoice,
}
/// File identity shown independently of the synchronization message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusItem {
    pub name: String,
    pub path: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// Absent when the detailed status cannot be represented by the four overview states.
    pub overview: Option<crate::ui::components::sync_overview::SyncOverviewState>,
    pub scope: String,
    pub status: StatusView,
    pub description: String,
    pub item: Option<StatusItem>,
    pub active: bool,
    pub activity: Option<ActivityView>,
    pub attention: Vec<AttentionItemView>,
}
