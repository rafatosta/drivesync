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
    /// Handle each `DecisionOutcome` explicitly. Only `ActionSelected` may
    /// proceed to operation planning, subject to execution preconditions.
    /// NonEmpty prevents empty actionable/evidence payloads. Validate targets,
    /// deduplication and preconditions before acknowledgement.
    /// EstablishBaseline requires verification and durable confirmation, not an
    /// Operator transfer. Obsolete must be proven and durably handled; a whole
    /// delivery may be Skipped(Obsolete) only if every target is obsolete.
    /// Blocked records the known unsupported condition without operation planning.
    /// Authorization is a requirement to persist, not an automatic deletion.
    ///
    /// An error means the detector delivery must be retried. A completed result
    /// authorizes acknowledgement; it does not by itself assert that a physical
    /// operation ran unless the item disposition says so. For `Completed`, return
    /// exactly one result for each decision target, preserving its target identity.
    fn apply<'a>(&'a self, decision: AnalysisDecision) -> ApplyFuture<'a>;
}
