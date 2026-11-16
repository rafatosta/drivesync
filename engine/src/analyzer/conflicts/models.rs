//! Identification-only contracts, private to Analyzer; no resolution/action types.
use crate::inventory::{SyncItem, SyncItemId};

/// One logical item's snapshots, plus revision-bound comparisons from the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::analyzer) struct ConflictRequest {
    pub item: SyncItem,
    pub evidence: ConflictEvidence,
}
/// Measurements must refer to the supplied revision and comparable content domains.
/// This pure evaluator cannot authenticate the caller's evidence or collect hashes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::analyzer) struct ConflictEvidence {
    pub baseline_confirmed: bool,
    pub local_scope_complete: bool,
    pub remote_scope_complete: bool,
    pub content: ContentEvidence,
    pub local_parent: Option<SyncItemId>,
    pub current_parents: EvidenceComparison,
}
/// Explicit comparisons; size, timestamps and remote versions are not hashes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::analyzer) struct ContentEvidence {
    pub local_to_baseline: EvidenceComparison,
    pub remote_to_baseline: EvidenceComparison,
    pub current_sides: EvidenceComparison,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::analyzer) enum EvidenceComparison {
    Equal,
    Different,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::analyzer) enum ConflictPresence {
    NoConflict,
    Conflict,
    Undetermined,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::analyzer) enum ConflictDimension {
    Presence,
    Content,
    Name,
    Location,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::analyzer) enum FindingStatus {
    Compatible,
    Divergent,
    Unknown,
}
/// A factual explanation code, not a proposed action or user choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::analyzer) enum FindingReason {
    Equivalent,
    UnilateralChange,
    ConcurrentChanges,
    DeleteVsChange,
    NewItem,
    MissingBaseline,
    InsufficientEvidence,
    UnsupportedKind,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::analyzer) struct ConflictFinding {
    pub dimension: ConflictDimension,
    pub status: FindingStatus,
    pub reason: FindingReason,
}
/// Missing evidence is separate from a proven need for a user decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::analyzer) enum EvidenceRequirement {
    ObserveLocal,
    ObserveRemote,
    VerifyBaseline,
    VerifyContent,
    VerifyParent,
    VerifyName,
    VerifyKind,
}
/// Findings retain unknown dimensions even when another dimension proves conflict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::analyzer) struct ConflictAssessment {
    pub item_id: SyncItemId,
    pub item_revision: u64,
    pub presence: ConflictPresence,
    pub findings: Vec<ConflictFinding>,
    pub requirements: Vec<EvidenceRequirement>,
}
impl ConflictAssessment {
    /// Only proven incompatibility requests a user decision. Unknown evidence
    /// needs further observation; NoConflict does not assert synchronization.
    pub(in crate::analyzer) fn requires_user_intervention(&self) -> bool {
        self.presence == ConflictPresence::Conflict
    }
    pub(super) fn require(&mut self, requirement: EvidenceRequirement) {
        if !self.requirements.contains(&requirement) {
            self.requirements.push(requirement);
        }
    }
}
/// Sanitized structural failures; actual conflicts are successful evaluations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::analyzer) enum ConflictError {
    InvalidRequest,
    InconsistentInput,
    WrongItemIdentity,
    InvalidEvidence,
}
