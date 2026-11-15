//! Internal decision vocabulary. No type here authorizes physical execution.
use crate::inventory::{ItemKind, SyncItem, SyncItemId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::analyzer) struct ReconciliationRequest {
    pub item: SyncItem,
    pub evidence: ReconciliationEvidence,
}

/// Comparisons are supplied by the coordinator, bound to the input revision.
/// Local content hashes and logical parent mapping are absent from LocalFile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::analyzer) struct ReconciliationEvidence {
    pub local_observation: ObservationConfidence,
    pub remote_observation: ObservationConfidence,
    pub local_content: Change,
    pub remote_content: Change,
    pub content_comparison: Comparison,
    pub local_parent: Option<SyncItemId>,
    pub parent_comparison: Comparison,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::analyzer) enum ObservationConfidence {
    Complete,
    Partial,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::analyzer) enum Change {
    Unchanged,
    Changed,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::analyzer) enum Comparison {
    Equal,
    Different,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::analyzer) enum Side {
    Local,
    Remote,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::analyzer) enum Dimension {
    Presence,
    Content,
    Name,
    Location,
}
/// Semantic intent only. Create is blocked on destination mapping; propagation
/// still requires command construction, capabilities and execution preconditions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::analyzer) enum Decision {
    Create { source: Side, kind: ItemKind },
    Trash { target: Side },
    Propagate { dimension: Dimension, source: Side },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::analyzer) enum DivergenceReason {
    MissingBaseline,
    IncompatibleStates,
    ConcurrentChanges,
    DeleteVsChange,
    InconsistentEvidence,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::analyzer) struct Divergence {
    pub dimension: Dimension,
    pub reason: DivergenceReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::analyzer) enum Requirement {
    Observe(Side),
    Compare(Dimension),
    VerifyIdentity,
    VerifyBaseline,
    VerifyContent,
    VerifySurvivingState,
    AuthorizeDeletion,
    ResolveDestination,
    SupportedKind,
}
/// Empty decisions do not mean synchronized when requirements/divergences remain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::analyzer) struct ReconciliationAssessment {
    pub item_id: SyncItemId,
    pub item_revision: u64,
    pub decisions: Vec<Decision>,
    pub divergences: Vec<Divergence>,
    pub requirements: Vec<Requirement>,
}
impl ReconciliationAssessment {
    pub(super) fn require(&mut self, requirement: Requirement) {
        if !self.requirements.contains(&requirement) {
            self.requirements.push(requirement);
        }
    }
}
/// Sanitized validation errors, without filenames, payloads or credentials.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::analyzer) enum ReconciliationError {
    InvalidRequest,
    InconsistentInput,
    InvalidItemIdentity,
    InvalidEvidence,
}
