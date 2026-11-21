//! Typed validation, comparison, decision, and operation-plan contracts.

pub(crate) use super::super::operations::models::{Operation, OperationPreconditions};
use crate::inventory::models::BaselineState;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Result of checking whether a File is safe and coherent to analyze.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Validation {
    /// Aggregate validation outcome.
    pub status: ValidationStatus,
    /// Individual rules and their outcomes.
    pub checks: Vec<ValidationCheck>,
}

/// Aggregate validation result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ValidationStatus {
    /// All required validation checks passed.
    Valid,
    /// The input violates a structural or scope contract.
    InvalidInput,
    /// Available identities or prior state establish a conflict.
    Conflict,
    /// More evidence or a later retry is required.
    Defer,
}

/// One stable validation result and its explanation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ValidationCheck {
    /// Result of this validation rule.
    pub status: ValidationCheckStatus,
    /// Stable machine-readable rule code.
    pub code: String,
    /// Human-readable explanation.
    pub detail: String,
}

/// Outcome of one validation rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ValidationCheckStatus {
    /// The rule passed.
    Pass,
    /// The input is inconsistent or malformed.
    InvalidInput,
    /// The evidence establishes incompatible state.
    Conflict,
    /// The rule could not be decided from current evidence.
    Defer,
}

/// Attribute-level comparison against the last confirmed baseline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Changeset {
    /// Whether the baseline and current evidence can be compared conclusively.
    pub status: ComparisonStatus,
    /// Baseline state used for the comparison.
    pub baseline: BaselineState,
    /// Local and remote changes for each supported attribute.
    pub attributes: BTreeMap<AttributeName, AttributePairChange>,
    /// Concise evidence supporting the comparison.
    pub evidence: Vec<String>,
    /// Comparisons that need more evidence.
    pub warnings: Vec<ValidationCheck>,
}

/// Whether a changeset is complete enough for planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ComparisonStatus {
    /// Required values were compared against a confirmed baseline.
    Comparable,
    /// At least one required comparison is unavailable or ambiguous.
    Blocked,
}

/// Supported dimensions of synchronization state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum AttributeName {
    /// Presence within the managed scope.
    Existence,
    /// Local kind or remote MIME type.
    Kind,
    /// Content identity based on comparable checksums or a conclusive size change.
    Content,
    /// Item name.
    Name,
    /// Parent location.
    Parent,
    /// Provider trash state.
    Trashed,
}

/// Local and remote change evidence for one attribute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AttributePairChange {
    /// Local side compared with its baseline value.
    pub local: AttributeEvidence,
    /// Remote side compared with its baseline value.
    pub remote: AttributeEvidence,
}

/// Result and rationale for one side of an attribute comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AttributeEvidence {
    /// Comparison outcome.
    pub status: ChangeStatus,
    /// Evidence used or the reason the comparison is incomplete.
    pub basis: String,
}

/// Outcome of an attribute comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ChangeStatus {
    /// The current value differs from the baseline.
    Changed,
    /// The current value equals the baseline.
    Unchanged,
    /// Available evidence cannot establish equality or difference.
    Indeterminate,
    /// The attribute does not apply to this side.
    NotApplicable,
}

/// Overall Analyzer result; a ready decision is a plan, not a completed operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Decision {
    /// Resulting plan or blocking state.
    pub status: DecisionStatus,
    /// Explanation of the result.
    pub reason: String,
    /// Input consistency results.
    pub validation: Validation,
    /// Attribute comparisons used by the decision.
    pub changeset: Changeset,
    /// Planned operations, empty whenever execution is blocked or deferred.
    pub operations: Vec<Operation>,
    /// Static execution feasibility based on the supplied File.
    pub execution: ExecutionAssessment,
}

/// Possible Analyzer outcomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum DecisionStatus {
    /// A complete, currently executable plan exists.
    Ready,
    /// Both sides match the baseline and need no operation.
    NoOp,
    /// Explicitly authorized initial association can be adopted.
    Adopt,
    /// Incompatible concurrent state is established.
    Conflict,
    /// Evidence or preconditions are insufficient.
    Defer,
    /// Input violates a contract.
    InvalidInput,
    /// The requested change is outside supported Analyzer behavior.
    Unsupported,
}

/// Static assessment of whether the plan can be attempted in the observed environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ExecutionAssessment {
    /// Aggregate feasibility.
    pub status: ExecutionStatus,
    /// Conditions checked against current evidence.
    pub checks: Vec<ExecutionCheck>,
    /// Failed conditions summarized for callers.
    pub issues: Vec<String>,
}

/// Aggregate execution feasibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ExecutionStatus {
    /// Every required capability and environment fact is known and available.
    Executable,
    /// Required environment or evidence is currently unavailable.
    Blocked,
    /// A required operation is explicitly unsupported or denied.
    NotExecutable,
}

/// One static operation feasibility condition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ExecutionCheck {
    /// Whether the condition is satisfied.
    pub ok: bool,
    /// Stable machine-readable condition code.
    pub code: String,
    /// Human-readable explanation.
    pub detail: String,
}
