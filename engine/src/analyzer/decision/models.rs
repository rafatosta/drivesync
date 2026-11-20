//! Semantic decision artifacts passed unchanged to the apply stage.

use crate::{
    analyzer::{AnalysisContext, AnalysisDisposition},
    inventory::{AnalysisFinding, AnalysisIntent, AnalysisRequirement, SyncItemId},
    models::FileId,
};

/// Decision artifact for the detector file and all subjects resolved in context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalysisDecision {
    /// Original observation and inventory snapshots used to make these decisions.
    pub context: AnalysisContext,
    /// Per-item or scope-level semantic outcomes.
    pub items: Vec<AnalysisItemDecision>,
}

/// Identity boundary for one entry in `AnalysisDecision::items`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecisionTarget {
    /// The incoming observation has no current inventory item yet.
    Observation(FileId),
    /// A decision is bound to the exact inventory item revision read in context.
    Item { id: SyncItemId, revision: u64 },
    /// The input represents a synchronization-wide scope rather than one item.
    Synchronization,
}

/// Semantic result for one affected target; contains no executable command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalysisItemDecision {
    /// Stable target associated with this decision.
    pub target: DecisionTarget,
    /// Findings that explain compatible, divergent, or unknown dimensions.
    pub findings: Vec<AnalysisFinding>,
    /// Mutually exclusive semantic outcome; not a persisted result.
    pub outcome: DecisionOutcome,
}

/// Semantic outcome before Apply persists or executes a decision.
///
/// Only actionable decisions can carry intents.
///
/// ```compile_fail
/// use drivesync_engine::analyzer::DecisionOutcome;
/// let _ = DecisionOutcome::Conflict {
///     requirements: vec![],
///     intents: vec![],
/// };
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecisionOutcome {
    /// No synchronization operation is necessary.
    NoAction,
    /// Semantic intents, without physical destinations or executable commands.
    /// Must contain at least one intent.
    ActionSelected { intents: Vec<AnalysisIntent> },
    /// A conflict blocks execution until resolved.
    Conflict { requirements: Vec<AnalysisRequirement> },
    /// Additional evidence is required before a safe decision can be made.
    /// Must contain at least one requirement.
    NeedsEvidence { requirements: Vec<AnalysisRequirement> },
}

impl DecisionOutcome {
    /// Classifies the semantic outcome without implying durable handling.
    /// Apply may use this classification only after handling the decision.
    pub fn disposition(&self) -> AnalysisDisposition {
        match self {
            Self::NoAction => AnalysisDisposition::NoAction,
            Self::ActionSelected { .. } => AnalysisDisposition::ActionSelected,
            Self::Conflict { .. } => AnalysisDisposition::Conflict,
            Self::NeedsEvidence { .. } => AnalysisDisposition::NeedsEvidence,
        }
    }
}
