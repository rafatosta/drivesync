//! Public boundary between the orchestrator and the per-file analyzer.
//!
//! The orchestrator submits one validated detector `File` and awaits its result.
//! Progress is reported independently so the orchestrator can publish which file
//! is active and whether each submitted file was analyzed, skipped, or failed.

use crate::models::{File, FileId, SyncId};
use std::{future::Future, pin::Pin};

/// Boxed future returned by the analyzer boundary.
pub type AnalysisFuture<'a> =
    Pin<Box<dyn Future<Output = Result<AnalysisResult, AnalysisError>> + Send + 'a>>;

/// One-file analyzer contract consumed by the orchestrator.
pub trait Analyzer: Send + Sync {
    /// Analyzes one detector file and reports lifecycle updates to `feedback`.
    ///
    /// The call completes only after this file has reached a terminal result.
    /// Implementations must report `Started` before phase updates and exactly one
    /// terminal feedback state (`Completed`, `Skipped`, or `Failed`).
    fn analyze<'a>(&'a self, file: File, feedback: &'a dyn AnalysisFeedback) -> AnalysisFuture<'a>;
}

/// Receives per-file lifecycle updates while an analysis call is pending.
pub trait AnalysisFeedback: Send + Sync {
    /// Publishes a lifecycle update for the file currently being analyzed.
    fn report(&self, update: AnalysisUpdate);
}

/// Progress or terminal status for one detector file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalysisUpdate {
    /// Detector identity of the submitted file.
    pub file_id: FileId,
    /// Synchronization to which the submitted file belongs.
    pub sync_id: SyncId,
    /// Current lifecycle status.
    pub status: AnalysisStatus,
}

/// Lifecycle state reported for an individual file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnalysisStatus {
    /// The analyzer has accepted the file and started processing it.
    Started,
    /// The analyzer is working through a named stage.
    InProgress(AnalysisStage),
    /// The analyzer finished handling the file.
    Completed(AnalysisDisposition),
    /// The file was deliberately not analyzed, with a machine-readable reason.
    Skipped(AnalysisSkipReason),
    /// Analysis failed; the call also returns the corresponding error.
    Failed(AnalysisErrorKind),
}

/// Coarse stages suitable for user-visible synchronization feedback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisStage {
    /// Read current and historical state from the inventory.
    Inspecting,
    /// Determine the intended synchronization changes.
    Reconciling,
    /// Check whether concurrent changes require conflict handling.
    CheckingConflicts,
    /// Prepare the changes to be applied or recorded.
    Preparing,
    /// Apply the analyzer's selected changes.
    Applying,
    /// Confirm the resulting state.
    Verifying,
}

/// High-level outcome of successfully handling one file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisDisposition {
    /// The file was handled and no operation was needed.
    NoAction,
    /// The file was handled and a synchronization operation was selected.
    ActionSelected,
    /// The file was handled but requires a conflict decision.
    Conflict,
    /// The file was handled but more evidence is needed before an operation.
    NeedsEvidence,
}

/// Why an accepted file did not enter analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisSkipReason {
    /// The file was already handled and the delivery is a duplicate.
    Duplicate,
    /// The observation is older than the state already known to the analyzer.
    Obsolete,
    /// The observation cannot be analyzed with the currently available state.
    InsufficientContext,
}

/// Sanitized analyzer failure category suitable for status reporting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisErrorKind {
    /// Inventory access or persistence failed.
    Inventory,
    /// Required provider or filesystem evidence could not be read.
    Evidence,
    /// Applying or verifying a selected operation failed.
    Operation,
    /// An unexpected internal failure occurred.
    Internal,
}

/// Analyzer error returned to the orchestrator after terminal feedback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalysisError {
    /// Stable category for orchestration and user-visible diagnostics.
    pub kind: AnalysisErrorKind,
}

/// Terminal result for one submitted detector file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnalysisResult {
    /// Analysis reached a successful outcome.
    Completed(AnalysisDisposition),
    /// The analyzer intentionally ignored this delivery.
    Skipped(AnalysisSkipReason),
}

impl AnalysisUpdate {
    /// Builds an update tied to the identity of the submitted detector file.
    pub fn new(file: &File, status: AnalysisStatus) -> Self {
        Self {
            file_id: file.id.clone(),
            sync_id: file.sync_id.clone(),
            status,
        }
    }
}
