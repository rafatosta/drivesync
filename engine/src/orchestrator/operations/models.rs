//! Normalized execution contracts. Credentials never enter these values.
pub(crate) use crate::inventory::models::{AppliedEffect, OperationAction};
use crate::{
    inventory::models::{InventoryWriteResult, LocalState, RemoteState},
    models::*,
};
use serde::{Deserialize, Serialize};

pub(crate) use crate::models::{ExecutionEffect, Operation, OperationPreconditions};

/// Fixed account and root boundaries for an executor.
#[derive(Debug, Clone)]
pub(crate) struct OperatorConfig {
    /// Explicit initial-association authorization for this session only.
    pub initial_association_authorized: bool,
    /// Explicit recoverable trash propagation authorization for this session only.
    pub trash_propagation_authorized: bool,
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

/// Conservative read-based reconciliation; Unknown never authorizes a repeat.
#[derive(Debug, Clone)]
pub(crate) struct RecoveryAssessment {
    pub outcome: ExecutionEffect,
    pub issue: Option<crate::inventory::models::RecoveryIssue>,
    pub effect: Option<AppliedEffect>,
}
impl RecoveryAssessment {
    pub(super) fn unknown(issue: crate::inventory::models::RecoveryIssue) -> Self {
        Self {
            outcome: ExecutionEffect::Unknown,
            issue: Some(issue),
            effect: None,
        }
    }
}
#[derive(Debug, Clone)]
pub(crate) struct RecoveryResult {
    pub operation_id: Id,
    pub item: crate::synchronization::SynchronizationItem,
    pub assessment: RecoveryAssessment,
    pub inventory_write: Option<InventoryWriteResult>,
}
