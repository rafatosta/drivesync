//! Normalized execution contracts. Credentials never enter these values.
pub(crate) use crate::inventory::models::{AppliedEffect, OperationAction};
use crate::{
    inventory::models::{InventoryWriteResult, LocalState, RemoteState},
    models::*,
};
use serde::{Deserialize, Serialize};

/// One ordered operation in a proposed plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Operation {
    /// One-based order within the plan.
    pub step: u64,
    /// Requested normalized operation.
    pub action: OperationAction,
    /// Logical item identity.
    pub item_ref: ItemId,
    /// Earlier operation steps that must complete first.
    pub depends_on: Vec<u64>,
    /// Provider item identity when known.
    pub remote_id: Option<RemoteId>,
    /// Current or target local relative path, when applicable.
    pub local_path: Option<Path>,
    /// Target name for a creation or rename.
    pub name: Option<String>,
    /// Target remote parent for a move or creation.
    pub parent_id: Option<RemoteId>,
    /// Target local parent for a move or creation.
    pub parent: Option<Path>,
    /// Preconditions that the executor must recheck before applying the operation.
    pub preconditions: Option<OperationPreconditions>,
}

/// Snapshot stability requirements for a planned operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct OperationPreconditions {
    /// Remote version observed during analysis.
    pub observed_remote_version: Option<u64>,
    /// Remote version recorded in the baseline.
    pub baseline_remote_version: Option<u64>,
    /// Whether the local source must remain unchanged until execution.
    pub local_unchanged: Evidence<bool>,
    /// Whether the remote source must remain unchanged until execution.
    pub remote_unchanged: Evidence<bool>,
    /// Whether the destination was confirmed free during analysis.
    pub destination_free: Evidence<bool>,
}

/// Fixed account and root boundaries for an executor.
#[derive(Debug, Clone)]
pub(crate) struct OperatorConfig {
    /// Explicit initial-association authorization for this session only.
    pub initial_association_authorized: bool,
    pub account_id: AccountId,
    pub sync_root_id: SyncRootId,
    pub provider_id: ProviderId,
    pub local_root: Path,
    pub remote_root_id: RemoteId,
}
/// Authorized command and the exact evidence whose stability must be checked.
#[derive(Debug, Clone)]
pub(crate) struct ExecuteOperationRequest {
    pub operation: Operation,
    pub execution_id: Id,
    pub expected_revision: Option<u64>,
    pub identity: FileIdentity,
    pub local: LocalState,
    pub remote: RemoteState,
    /// Receipts from successful dependencies in the same plan.
    pub completed_steps: Vec<u64>,
}
/// Actual execution outcome, including partial persistence failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct OperationResult {
    pub operation_id: Id,
    pub item_id: ItemId,
    pub action: OperationAction,
    pub status: OperationResultStatus,
    pub effect: Option<AppliedEffect>,
    pub completed_at: Option<DateTime>,
    pub error: Option<OperationFailure>,
    pub inventory_write: Option<InventoryWriteResult>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum OperationResultStatus {
    Succeeded,
    Failed,
    Unconfirmed,
    Skipped,
}
/// Sanitized failure without transport text, file contents, or sessions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct OperationFailure {
    pub code: String,
    pub retryable: bool,
    pub message: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExecutionEffect {
    NotApplied,
    Applied,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperatorErrorKind {
    UnsupportedProvider,
    UnsupportedOperation,
    InvalidRequest,
    PermissionDenied,
    Unavailable,
    PreconditionFailed,
    Cancelled,
    Internal,
}
/// Technical failure preserves whether repeating a command is safe.
#[derive(Debug, Clone)]
pub(crate) struct OperatorError {
    pub code: OperatorErrorKind,
    pub retryable: bool,
    pub effect: ExecutionEffect,
}
impl OperatorError {
    pub(super) fn new(code: OperatorErrorKind) -> Self {
        Self {
            code,
            retryable: false,
            effect: ExecutionEffect::NotApplied,
        }
    }
    pub(super) fn uncertain(mut self) -> Self {
        self.effect = ExecutionEffect::Unknown;
        self.retryable = false;
        self
    }
    pub(super) fn failure(&self) -> OperationFailure {
        OperationFailure {
            code: format!("{:?}", self.code),
            retryable: self.retryable,
            message: format!("Operation could not be confirmed: {:?}.", self.code),
        }
    }
}
