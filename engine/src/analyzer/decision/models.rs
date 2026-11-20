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
    /// Both states already agree with a confirmed baseline, or both are absent.
    NoAction,
    /// All relevant dimensions converge; Apply must verify and establish/advance
    /// the baseline. This is not itself a confirmation or a physical operation.
    EstablishBaseline,
    /// The observation is provably older than the matched subject.
    Obsolete,
    /// A known unsupported condition prevents synchronization.
    Blocked { reason: DecisionBlock },
    /// Semantic intents, without physical destinations or executable commands.
    /// Must contain at least one intent.
    ActionSelected { intents: NonEmpty<AnalysisIntent> },
    /// A conflict blocks execution until resolved.
    Conflict { requirements: Vec<AnalysisRequirement> },
    /// Terminal diagnosis after Context exhausted relevant collection; not a request.
    InsufficientEvidence { missing: NonEmpty<AnalysisRequirement> },
}

impl DecisionOutcome {
    /// Classifies the semantic outcome without implying durable handling.
    /// Apply may use this classification only after handling the decision.
    pub fn disposition(&self) -> AnalysisDisposition {
        match self {
            Self::NoAction => AnalysisDisposition::NoAction,
            Self::EstablishBaseline => AnalysisDisposition::BaselineEstablished,
            Self::Obsolete => AnalysisDisposition::Obsolete,
            Self::Blocked { .. } => AnalysisDisposition::Blocked,
            Self::ActionSelected { .. } => AnalysisDisposition::ActionSelected,
            Self::Conflict { .. } => AnalysisDisposition::Conflict,
            Self::InsufficientEvidence { .. } => AnalysisDisposition::Blocked,
        }
    }
}

/// A known condition which cannot be resolved by repeating an observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionBlock {
    UnsupportedKind,
    IdentityUncertain,
    ResourceUnavailable,
    ParentUnavailable,
    AuthorizationRequired,
    DestinationOccupied,
}

/// Non-empty ordered semantic payload. An empty list cannot be constructed
/// through the public API; callers cannot mutate the contained vector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NonEmpty<T>(Vec<T>);

impl<T> NonEmpty<T> {
    pub fn new(first: T, rest: Vec<T>) -> Self {
        let mut values = vec![first];
        values.extend(rest);
        Self(values)
    }

    pub fn from_vec(values: Vec<T>) -> Option<Self> {
        if values.is_empty() { None } else { Some(Self(values)) }
    }

    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<T> {
        self.0
    }
}
