//! Terminal per-target results returned to the Analyzer facade.

use crate::analyzer::{AnalysisDisposition, DecisionTarget};

/// Durable outcome for one item or scope decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalysisItemResult {
    /// Target whose decision was handled.
    pub target: DecisionTarget,
    /// Persisted semantic outcome; selected work may remain pending execution.
    pub disposition: AnalysisDisposition,
}
