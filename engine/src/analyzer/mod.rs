//! Public boundary between the orchestrator and the per-file analyzer.
//!
//! The orchestrator submits one validated detector `File` and awaits its result.
//! Progress is reported independently so the orchestrator can publish which file
//! is active and whether each submitted file was analyzed, skipped, or failed.
//!
//! The internal artifact flow is `File -> AnalysisContext -> AnalysisDecision -> AnalysisResult`.
//! Context reads state, Decision classifies intent, and Apply plans/persists/optionally
//! executes. The decision artifact retains the context needed by Apply.
//!
//! | Boundary | Receives | Returns |
//! | --- | --- | --- |
//! | `Analyzer::analyze` | One `File` and feedback sink | `AnalysisResult` |
//! | `ContextService::build` | One `File` | `AnalysisContext` |
//! | `DecisionService::decide` | `AnalysisContext` | `AnalysisDecision` |
//! | `ApplyService::apply` | `AnalysisDecision` | `AnalysisResult` |

pub mod apply;
pub mod context;
pub mod decision;

pub use apply::{AnalysisItemResult, ApplyService};
pub use context::{
    AnalysisContext, AnalysisItemContext, ContextAssociation, ContextService, ObservationContext,
    ObservationFreshness,
};
pub use decision::{
    AnalysisDecision, AnalysisItemDecision, DecisionOutcome, DecisionService, DecisionTarget,
};

use crate::models::File;
use std::{future::Future, pin::Pin};

/// Boxed future returned by the analyzer boundary.
pub type AnalysisFuture<'a> =
    Pin<Box<dyn Future<Output = Result<AnalysisResult, AnalysisError>> + Send + 'a>>;

/// One-file analyzer contract consumed by the orchestrator.
pub trait Analyzer: Send + Sync {
    /// Analyzes one detector file and reports lifecycle updates to `feedback`.
    ///
    /// The call completes only after this file has reached a terminal result.
    /// The orchestrator publishes `Started` before calling this method;
    /// implementations report phase updates and return the authoritative result.
    /// Returning `Ok` authorizes the orchestrator to acknowledge this detector file:
    /// completed outcomes must be durably handled, and skipped outcomes must be
    /// proven duplicate or obsolete. Return `Err` when redelivery is required.
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
    /// Detector file currently being handled. Consumers should derive only the
    /// display-safe identity and location needed for progress reporting.
    pub file: File,
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
    /// The file's current state was checked and no operation was needed.
    NoAction,
    /// The selected synchronization operation was durably recorded.
    ActionSelected,
    /// A conflict outcome was durably recorded for later resolution.
    Conflict,
    /// A request for more evidence was durably recorded before an operation.
    NeedsEvidence,
}

/// Why an accepted file did not enter analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisSkipReason {
    /// The file was already handled and the delivery is a duplicate.
    Duplicate,
    /// The observation is older than the state already known to the analyzer.
    Obsolete,
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

impl std::fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let description = match self.kind {
            AnalysisErrorKind::Inventory => "inventory access or persistence failed",
            AnalysisErrorKind::Evidence => "required analysis evidence is unavailable",
            AnalysisErrorKind::Operation => "a selected operation failed",
            AnalysisErrorKind::Internal => "an internal analyzer failure occurred",
        };
        f.write_str(description)
    }
}

impl std::error::Error for AnalysisError {}

/// Terminal result for one submitted detector file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnalysisResult {
    /// Every target produced by the decision stage reached a durable outcome.
    Completed(Vec<AnalysisItemResult>),
    /// The analyzer intentionally ignored this delivery.
    Skipped(AnalysisSkipReason),
}

impl AnalysisUpdate {
    /// Builds an update tied to the identity of the submitted detector file.
    pub fn new(file: &File, status: AnalysisStatus) -> Self {
        Self {
            file: file.clone(),
            status,
        }
    }
}
