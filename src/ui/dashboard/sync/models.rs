//! Presentation values contain no synchronization implementation types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatusView {
    Initializing,
    Scanning,
    Synchronizing,
    Synced,
    Attention,
    Offline,
    Failed,
    Stopped,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProgressView {
    Indeterminate,
    Bytes { transferred: u64, total: u64 },
    Steps { completed: u64, total: u64 },
}
impl ProgressView {
    pub fn measured(&self) -> Option<(u64, u64)> {
        let (value, total) = match *self {
            Self::Indeterminate => return None,
            Self::Bytes { transferred, total } => (transferred, total),
            Self::Steps { completed, total } => (completed, total),
        };
        (total > 0 && value <= total).then_some((value, total))
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActivityKind {
    Download,
    Upload,
    Verify,
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
pub struct IssueView {
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub scope: String,
    pub status: StatusView,
    pub description: String,
    pub activity: Option<ActivityView>,
    pub attention: Vec<AttentionItemView>,
}
