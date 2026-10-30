//! Evidence-normalized differences; callers retain the scoped B/L/R observations.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

/// A durable, non-authorizing snapshot of an item that needs review.
///
/// The record contains identifiers and verified observations only; it never
/// stores file contents, OAuth credentials, or permission to apply a decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConflictRecord {
    /// Stable key derived from the remote identity and observed path.
    pub conflict_id: String,
    pub remote_id: Option<String>,
    pub remote_parent_id: Option<String>,
    pub remote_name: Option<String>,
    pub relative_path: Option<PathBuf>,
    pub diagnosis: crate::ChangeKind,
    pub status: ConflictStatus,
    pub local_fingerprint: Option<String>,
    pub remote_version: Option<u64>,
    pub outcomes: Vec<ConflictOutcome>,
}

/// Whether the latest verified observation still requires user attention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ConflictStatus {
    Pending,
    NoLongerConflicting,
}

/// Verified effects or sanitized failures from supported conflict handling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ConflictOutcome {
    /// The remote version was verified and retained as a separate local copy.
    CopyPreserved {
        copy_path: PathBuf,
        remote_version: u64,
    },
    /// A retry stopped without claiming success or changing the conflict status.
    RetryBlocked { reason: ConflictFailure },
    /// A decision based on outdated evidence was cancelled without any effect.
    /// A new comparison is required before another proposal can be applied.
    ResolutionAborted {
        reason: super::StaleResolutionReason,
    },
}

/// Safe error categories suitable for durable audit without raw provider text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ConflictFailure {
    LocalConflict,
    LocalWriting,
    LocalTemporary,
    ObservationUnknown,
    Integrity,
    LocalIo,
    RemoteChanged,
    RemoteUnavailable,
    StateUnavailable,
}

/// Append-only lifecycle event for a conflict item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConflictHistoryEvent {
    /// Monotonic sequence within the scoped SQLite journal.
    pub sequence: u64,
    pub record: ConflictRecord,
    pub kind: ConflictHistoryKind,
}

/// Audited state transition; repeated identical observations are deduplicated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ConflictHistoryKind {
    PendingObserved,
    NoLongerConflicting,
    CopyPreserved,
    RetryBlocked {
        reason: ConflictFailure,
    },
    ResolutionAborted {
        reason: super::StaleResolutionReason,
    },
}
