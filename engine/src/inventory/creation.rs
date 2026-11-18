//! Analyzer-owned execution claims and verified replica provenance.
use super::*;
use crate::{models::Timestamp, operations::*};

/// Durable binding stored on the analysis operation, never inferred by names.
#[derive(Debug, Clone)]
pub struct CreationBinding {
    pub operation: SyncOperation,
    pub intent: ExecutionIntent,
}
pub struct GetCreationRequest {
    pub item_id: SyncItemId,
}
pub struct BeginCreationRequest {
    pub analysis_id: AnalysisId,
    pub expected_item_revision: u64,
    pub intent: ExecutionIntent,
    pub started_at: Timestamp,
}
pub struct RecordCreatedReplicaRequest {
    pub binding: CreationBinding,
    pub result: ItemResult,
}
pub struct RecordTrashedLocalRequest {
    pub binding: CreationBinding,
    pub result: ItemResult,
    pub trashed_at: Timestamp,
}
pub struct FailCreationRequest {
    pub binding: CreationBinding,
    pub error: OperatorError,
    pub at: Timestamp,
}

/// Bounded recovery discovery; excludes completed and permanently blocked work.
pub struct DueCreationsRequest {
    pub at: Timestamp,
    pub limit: u32,
}

/// Completes an applied step while leaving the previous baseline unchanged.
/// Used only when another planned dimension still needs reconciliation.
pub struct CompleteExecutionStepRequest {
    pub binding: CreationBinding,
    pub at: Timestamp,
}
