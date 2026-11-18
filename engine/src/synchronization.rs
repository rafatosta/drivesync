//! Account-scoped synchronization snapshots shared by orchestration and consumers.
//!
//! Producers publish complete snapshots. Consumers receive the latest snapshot
//! and coalesced change notifications without starting work or reading storage.
use crate::{
    analyzer::{AnalysisPhase, AnalyzerErrorKind, CreationStage},
    detectors::{
        DeliveryState, DetectorError, DetectorSourceError, DetectorState, LocalActivity,
        LocalDetectorError, LocalDetectorState, LocalScanCoverage, QueueError, RemoteActivity,
        RemoteCoverage, RemoteError, RemoteState,
    },
    inventory::{InventoryError, ItemKind, SyncItemId},
    models::{FileEventAction, FileLocation, LocalRelativePath, RemoteFileId, RemoteFileRef},
    operations::{ExecutionPhase, OperatorError},
    Timestamp,
};
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

/// Broad lifecycle of the synchronization pipeline for one account.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SynchronizationState {
    /// No preparation or synchronization activity has started in this process.
    #[default]
    Idle,
    /// A scan, analysis, event batch or file operation is in progress.
    Running,
    /// The pipeline is healthy and waiting for another observation or retry time.
    Waiting,
    /// The last scan or event batch was incomplete and requires another pass.
    Partial,
    /// The pipeline was explicitly stopped or its active task was cancelled.
    Stopped,
    /// The pipeline cannot continue until an error is handled.
    Failed,
}

/// Coarse phase suitable for routing presentation while preserving detailed work data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SynchronizationActivity {
    /// No work is active.
    #[default]
    Idle,
    /// Sources and account-scoped preparation are being initialized.
    Starting,
    /// The local sync root is being enumerated.
    Scanning,
    /// Inventory evidence is being evaluated into durable decisions.
    Analyzing,
    /// Detector observations are being ingested into the inventory.
    ProcessingEvents,
    /// A prerequisite or operation is being checked.
    Checking,
    /// A transfer to remote storage is in progress.
    Uploading,
    /// A transfer from remote storage is in progress.
    Downloading,
    /// A local or remote operation is being applied.
    Applying,
    /// An operation result is being checked against its expected state.
    Verifying,
    /// The pipeline is waiting for an observation, source recovery or retry.
    Waiting,
    /// The pipeline has been explicitly stopped.
    Stopped,
    /// The pipeline cannot proceed because of an unresolved failure.
    Failed,
}

/// User-meaningful operation currently associated with a snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SynchronizationAction {
    /// Observe changes under the local synchronization root.
    ObserveLocal,
    /// Observe changes from the configured remote source.
    ObserveRemote,
    /// Analyze one inventory item and persist its current decision.
    Analyze,
    /// Reconcile observations and inventory state.
    Reconcile,
    /// Send file content to the remote source.
    Upload,
    /// Receive file content from the remote source.
    Download,
    /// Create a file entry.
    CreateFile,
    /// Create a directory entry.
    CreateDirectory,
    /// Replace content on an existing item.
    UpdateContent,
    /// Rename an item.
    Rename,
    /// Move an item to another parent.
    Move,
    /// Move an item to trash.
    Trash,
    /// Restore a trashed item.
    Restore,
    /// Verify the resulting state of an operation.
    Verify,
}

/// Scope-safe location data; local paths are relative to the configured sync root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SynchronizationLocation {
    /// Configured local sync root.
    LocalRoot,
    /// Path below the configured local sync root.
    Local(LocalRelativePath),
    /// Configured remote root.
    RemoteRoot(RemoteFileRef),
    /// Remote item or directory with optional known parent and display name.
    Remote {
        reference: RemoteFileRef,
        parent_id: Option<RemoteFileId>,
        name: Option<String>,
    },
}

/// File or directory identity and names known while work is in progress.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SynchronizationItem {
    /// Durable identity assigned by the inventory, when already known.
    pub id: Option<SyncItemId>,
    /// File, directory or symlink kind, when known.
    pub kind: Option<ItemKind>,
    /// Name in the local filesystem; may contain non-UTF-8 bytes.
    pub local_name: Option<std::ffi::OsString>,
    /// Name in the remote provider.
    pub remote_name: Option<String>,
    /// Path relative to the configured local sync root.
    pub local_path: Option<LocalRelativePath>,
    /// Opaque provider identity; consumers must not parse it.
    pub remote_reference: Option<RemoteFileRef>,
}

/// Detailed internal phase for the current item or event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SynchronizationWorkDetail {
    /// Source setup, scan coordination or another preparation step.
    Preparation,
    /// Current inventory analysis phase.
    Analysis(AnalysisPhase),
    /// Current operation planning or execution phase.
    Creation(CreationStage),
    /// Current transfer/application phase.
    Transfer(ExecutionPhase),
    /// The reason no active work is being performed.
    Waiting(SynchronizationWaitReason),
}

/// Current work target and its available source or destination context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SynchronizationWork {
    /// User-meaningful category of the current work.
    pub action: SynchronizationAction,
    /// File or directory being handled, when known.
    pub item: Option<SynchronizationItem>,
    /// Source, destination or containing location, when known.
    pub location: Option<SynchronizationLocation>,
    /// More specific phase or wait reason.
    pub detail: SynchronizationWorkDetail,
    /// Observation currently being ingested, when work was triggered by an event.
    pub event: Option<SynchronizationEvent>,
}

/// One observed change currently being ingested; it is evidence, not a sync command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SynchronizationEvent {
    /// Filesystem or remote change that prompted the ingestion.
    pub action: FileEventAction,
    /// Previous location for moves and renames, when known.
    pub previous: Option<FileLocation>,
    /// Current location, when the item still has one.
    pub current: Option<FileLocation>,
    /// Time the source recorded the observation.
    pub observed_at: Timestamp,
}

/// Why the worker is waiting instead of performing work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SynchronizationWaitReason {
    /// No new observations are available.
    ForChanges,
    /// A retry is scheduled for a later time.
    RetryBackoff,
    /// A local or remote observation source is recovering.
    SourceRecovery,
    /// Another local scan is required to restore complete coverage.
    IncompleteScan,
    /// Another event batch is required to finish processing observations.
    IncompleteEventBatch,
}

/// Unit and scope of a measured counter; totals remain absent when unknown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SynchronizationProgressUnit {
    /// Bytes transferred for the current operation.
    Bytes,
    /// Files visited or handled in the current scope.
    Files,
    /// Directories visited or handled in the current scope.
    Directories,
    /// Events processed in the current batch.
    Events,
    /// Items analyzed in the current pass.
    Items,
}

/// Measured progress for the current work item or bounded batch, never global completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SynchronizationProgress {
    /// Amount completed within the current work scope.
    pub completed: u64,
    /// Scope total when the producer can determine it.
    pub total: Option<u64>,
    /// Unit used by `completed` and `total`.
    pub unit: SynchronizationProgressUnit,
    /// Operation phase for byte-level progress, when applicable.
    pub phase: Option<ExecutionPhase>,
}

/// Counts collected during preparation and the current event or analysis pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SynchronizationCounters {
    /// Files observed during the current scan.
    pub files_observed: u64,
    /// Directories observed during the current scan.
    pub directories_observed: u64,
    /// Inventory records updated during the current scan.
    pub items_recorded: u64,
    /// Issues recorded during the current scan.
    pub scan_issues: u64,
    /// Events ingested in the current bounded batch.
    pub events_processed: u64,
    /// Event count in the current bounded batch.
    pub events_total: Option<u64>,
    /// Items analyzed in the current analysis pass.
    pub analysis_items_processed: u64,
    /// Item count in the current analysis pass.
    pub analysis_items_total: Option<u64>,
}

/// Current observation-source health and bounded queue counts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SynchronizationSources {
    /// Combined lifecycle reported by the detector.
    pub detector_state: Option<DetectorState>,
    /// Whether captured events are paused, active or degraded.
    pub delivery: Option<DeliveryState>,
    /// Events waiting to be leased for ingestion.
    pub pending_events: u64,
    /// Events currently leased to the ingestion worker.
    pub leased_events: u64,
    /// Events durably confirmed as ingested.
    pub confirmed_events: u64,
    /// Events durably captured by the source.
    pub detected_events: u64,
    /// Last queue error, if any.
    pub queue_error: Option<QueueError>,
    /// Cached local watcher state.
    pub local: SynchronizationLocalSource,
    /// Cached remote polling state.
    pub remote: SynchronizationRemoteSource,
}

/// Cached local watcher state sampled from the detector without filesystem I/O.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SynchronizationLocalSource {
    /// Current local watcher lifecycle.
    pub state: Option<LocalDetectorState>,
    /// Current local scan or watch activity.
    pub activity: Option<LocalActivity>,
    /// Entries inspected in the current local observation round.
    pub observed_in_current_round: u64,
    /// Events emitted in the current local observation round.
    pub emitted_in_current_round: u64,
    /// Time of the last local observation.
    pub last_observation_at: Option<Timestamp>,
    /// Coverage achieved by the most recent local scan.
    pub coverage: Option<LocalScanCoverage>,
    /// Most recent local watcher error.
    pub last_error: Option<LocalDetectorError>,
    /// Error that prevents the detector from using this source.
    pub source_error: Option<DetectorSourceError>,
}

/// Cached remote poll state sampled from the detector without provider I/O.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SynchronizationRemoteSource {
    /// Current remote poller lifecycle.
    pub state: Option<RemoteState>,
    /// Current remote polling activity.
    pub activity: Option<RemoteActivity>,
    /// Coverage achieved by the latest remote poll.
    pub coverage: Option<RemoteCoverage>,
    /// Configured delay between remote polls.
    pub polling_interval: Option<std::time::Duration>,
    /// Entries inspected in the current remote observation round.
    pub observed_in_current_round: u64,
    /// Events emitted in the current remote observation round.
    pub emitted_in_current_round: u64,
    /// Time of the most recent remote observation.
    pub last_observed_at: Option<Timestamp>,
    /// Start time of the most recent remote poll.
    pub last_poll_started_at: Option<Timestamp>,
    /// Scheduled time of the next remote poll.
    pub next_poll_at: Option<Timestamp>,
    /// Duration of the most recent remote poll.
    pub last_poll_duration: Option<std::time::Duration>,
    /// Consecutive remote poll failures.
    pub consecutive_failures: u32,
    /// Recovery attempts made by the remote source.
    pub recovery_attempts: u64,
    /// Most recent remote polling error.
    pub last_error: Option<RemoteError>,
    /// Error that prevents the detector from using this source.
    pub source_error: Option<DetectorSourceError>,
}

/// Sanitized reason work stopped or an operation could not complete.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SynchronizationIssueKind {
    /// Detector-level failure not attributed to a specific source.
    Detector(DetectorError),
    /// Local watcher or filesystem source failure.
    LocalSource(LocalDetectorError),
    /// Remote poller or provider source failure.
    RemoteSource(RemoteError),
    /// Inventory read or write failure.
    Inventory(InventoryError),
    /// Durable observation queue failure.
    Queue(QueueError),
    /// Local scanner failure.
    Scanner(crate::scanner::ScannerErrorKind),
    /// Inventory analysis failure.
    Analyzer(AnalyzerErrorKind),
    /// File operation failure.
    Operation(OperatorError),
    /// A scan ended without complete coverage.
    IncompleteScan,
    /// Failure without a more specific sanitized category.
    Internal,
}

/// Current or most recent unresolved problem; no raw provider or OS error text is retained.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SynchronizationIssue {
    /// Sanitized typed failure category.
    pub kind: SynchronizationIssueKind,
    /// Whether the engine expects retry or source recovery to help.
    pub retryable: bool,
    /// Item implicated in the problem, when known.
    pub item: Option<SynchronizationItem>,
    /// Number of affected items or occurrences represented by this issue.
    pub count: u64,
}

/// Complete read-only snapshot for one account.
///
/// `progress` is scoped to `work`; it never means whole-account completion.
/// `issue` remains available alongside later work until a new preparation succeeds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SynchronizationStatus {
    /// Overall lifecycle state for this account's pipeline.
    pub state: SynchronizationState,
    /// Broad current phase for presentation.
    pub activity: SynchronizationActivity,
    /// Current operation and its available context.
    pub work: Option<SynchronizationWork>,
    /// Progress scoped to `work`, not whole-account completion.
    pub progress: Option<SynchronizationProgress>,
    /// Counts from current scan, event batch and analysis pass.
    pub counters: SynchronizationCounters,
    /// Cached health and activity of local and remote sources.
    pub sources: Option<SynchronizationSources>,
    /// Current or most recent unresolved issue.
    pub issue: Option<SynchronizationIssue>,
}

/// Coalescing subscription: slow readers receive the latest complete snapshot.
pub struct SynchronizationSubscription(watch::Receiver<SynchronizationStatus>);
impl SynchronizationSubscription {
    /// Cached initial/latest value; performs no I/O.
    pub fn snapshot(&self) -> SynchronizationStatus {
        self.0.borrow().clone()
    }
    /// Waits for a change, including changes published before this call.
    pub async fn changed(&mut self) -> bool {
        self.0.changed().await.is_ok()
    }
}

#[derive(Default)]
pub(crate) struct StatusRegistry(
    pub Mutex<std::collections::HashMap<crate::AccountId, Arc<Publisher>>>,
);

/// Serializes updates from concurrent stages into complete public snapshots.
pub(crate) struct Publisher {
    state: Mutex<SynchronizationStatus>,
    sender: watch::Sender<SynchronizationStatus>,
}
impl Publisher {
    pub(crate) fn new() -> Self {
        let initial = SynchronizationStatus::default();
        let (sender, _) = watch::channel(initial.clone());
        Self {
            state: Mutex::new(initial),
            sender,
        }
    }
    pub(crate) fn subscribe(&self) -> SynchronizationSubscription {
        SynchronizationSubscription(self.sender.subscribe())
    }
    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> SynchronizationStatus {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    #[cfg(test)]
    pub(crate) fn publish(&self, status: SynchronizationStatus) {
        let mut current = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if *current == status {
            return;
        }
        *current = status.clone();
        self.sender.send_replace(status);
    }
    pub(crate) fn update(&self, update: impl FnOnce(&mut SynchronizationStatus)) {
        let mut current = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let previous = current.clone();
        update(&mut current);
        if *current != previous {
            self.sender.send_replace(current.clone());
        }
    }
}
