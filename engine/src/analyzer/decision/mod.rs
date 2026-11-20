//! Derives semantic outcomes from the context without persisting or executing them.
mod models;
pub use models::*;

use crate::analyzer::{AnalysisContext, AnalysisError};

/// Pure decision boundary over one complete context artifact.
pub trait DecisionService: Send + Sync {
    /// Produces one decision entry per affected item or scope target.
    ///
    /// Each entry has exactly one semantic outcome. ActionSelected must carry
    /// at least one intent, and NeedsEvidence at least one requirement.
    /// Targets must be unique and correspond to the input context.
    ///
    /// This stage may classify conflicts and identify required evidence. It does
    /// not access Inventory, select physical destinations, persist decisions, or
    /// invoke operations.
    /// It returns an `Observation` target when no inventory item matches, one
    /// `Item` target for each affected row, or a `Synchronization` target when a
    /// whole-synchronization observation has no current rows to decide.
    fn decide(&self, context: AnalysisContext) -> Result<AnalysisDecision, AnalysisError>;
}
