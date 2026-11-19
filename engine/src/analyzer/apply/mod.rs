//! Plans, persists, and optionally executes a completed semantic decision.
mod models;
pub use models::*;

use crate::analyzer::{AnalysisDecision, AnalysisFuture};

/// Boxed future returned while the apply stage handles a decision artifact.
pub type ApplyFuture<'a> = AnalysisFuture<'a>;

/// Applies one decision artifact through planning, persistence, and optional execution.
pub trait ApplyService: Send + Sync {
    /// Returns only after each decision has a durable terminal outcome.
    ///
    /// An error means the detector delivery must be retried. A completed result
    /// authorizes acknowledgement; it does not by itself assert that a physical
    /// operation ran unless the item disposition says so. For `Completed`, return
    /// exactly one result for each decision target, preserving its target identity.
    fn apply<'a>(&'a self, decision: AnalysisDecision) -> ApplyFuture<'a>;
}
