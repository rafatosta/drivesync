//! Evidence-normalized differences; callers retain the scoped B/L/R observations.
use serde::{Deserialize, Serialize};

/// Only durable completed work establishes B; invalid state is not a fresh start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BaseEvidence {
    Confirmed,
    Absent,
    Invalid,
}
/// Content comparison uses verified bytes, never timestamps or matching names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentComparison {
    Equal,
    Different,
    Unknown,
}
/// A name or parent change relative to B; matching targets may converge.
/// Targets are observations, not validated filesystem paths or write permissions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttributeChange {
    Unchanged,
    Changed(String),
    Unknown,
}
/// Independently established causes of unavailability do not prove deletion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationFailure {
    Offline,
    AccessDenied,
    NotListed,
    ReadFailed,
}
/// Differences from B supplied by an evidence adapter in one account/root scope.
/// `Present` requires continuous identity; a substituted or ambiguous identity
/// must use `Replaced` or `Unknown`, even when the bytes match B.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SideDifference {
    Present {
        content: ContentComparison,
        name: AttributeChange,
        parent: AttributeChange,
    },
    /// Observed absence is weaker than confirmed deletion or user intent.
    Absent,
    Deleted,
    Replaced,
    Unavailable(ObservationFailure),
    Unsupported,
    Unknown,
}
impl SideDifference {
    /// Verified continuous identity and no change relative to B.
    pub fn unchanged() -> Self {
        Self::Present {
            content: ContentComparison::Equal,
            name: AttributeChange::Unchanged,
            parent: AttributeChange::Unchanged,
        }
    }
}
/// Evidence of differences from B and a separate comparison of current L/R bytes.
/// Adapters must reject regressive revisions and incomplete scope/identity proofs.
/// This value contains no executable capability and does not establish a baseline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DifferenceInput {
    pub base: BaseEvidence,
    pub local: SideDifference,
    pub remote: SideDifference,
    pub current_content: ContentComparison,
}
/// Semantic diagnosis independent of available executors and policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DifferenceKind {
    Unchanged,
    LocalChanged,
    RemoteChanged,
    Equivalent,
    Conciliable,
    Conflict,
    Incompatible,
    Unknown,
}
/// Candidate handling only: even Synchronize requires policy and revalidation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Disposition {
    Synchronize,
    Reconcile,
    Preserve,
    Block,
    Unknown,
}
/// Stable reasons distinguish proven conflicts, missing evidence and policy limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DifferenceReason {
    NoChange,
    LocalOnly,
    RemoteOnly,
    EqualContent,
    IndependentChanges,
    DivergentContent,
    DivergentLocation,
    DeleteModify,
    BothDeleted,
    DeletionRequiresPolicy,
    ObservedAbsence,
    MissingBase,
    InvalidBase,
    IdentityChanged,
    Unsupported,
    Unavailable(ObservationFailure),
    IncompleteEvidence,
    ContentNotCompared,
}
/// A deterministic observational result, never confirmation of preservation or I/O.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DifferenceDecision {
    pub kind: DifferenceKind,
    pub disposition: Disposition,
    pub reason: DifferenceReason,
}
