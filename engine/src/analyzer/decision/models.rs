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
    /// High-level result selected from the available evidence.
    pub disposition: AnalysisDisposition,
    /// Findings that explain compatible, divergent, or unknown dimensions.
    pub findings: Vec<AnalysisFinding>,
    /// Additional evidence or authorization still required.
    pub requirements: Vec<AnalysisRequirement>,
    /// Incomplete semantic intents; no physical destination or operation command.
    pub intents: Vec<AnalysisIntent>,
}
