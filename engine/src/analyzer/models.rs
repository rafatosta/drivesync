//! Analysis coordination with optional verified operation execution.
use crate::{
    inventory::{
        AnalysisEvidence, InventoryError, InventoryManager, RecordedAnalysis, SyncIdentity,
        SyncItem, SyncItemId,
    },
    models::SyncId,
};
use std::sync::Arc;

pub type AnalyzerResult<T> = Result<T, AnalyzerError>;
/// Scoped persistence and optional account-scoped execution dependencies.
pub struct AnalyzerDependencies {
    pub inventory: Arc<InventoryManager>,
    pub execution: Option<AnalyzerExecution>,
}
pub struct AnalyzerConfig {
    pub provider: crate::models::ProviderId,
    pub identity: SyncIdentity,
    pub dependencies: AnalyzerDependencies,
}
/// One exact inventory snapshot and measurements bound to that revision.
/// New evidence or observation requires a new analysis; stale effects are never replayed.
#[derive(Debug, Clone)]
pub struct AnalyzeRequest {
    pub directive: Option<crate::operations::RestoreSelection>,
    /// Source events for this analysis pass; these are evidence, not deletion proof.
    pub observations: Vec<crate::models::File>,
    pub sync_id: SyncId,
    pub item: SyncItem,
    pub evidence: AnalysisEvidence,
    pub recorded_at: crate::models::Timestamp,
}
/// A durable evaluation receipt, not a statement that operations were executed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisOutcome {
    pub analysis: RecordedAnalysis,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisPhase {
    Validating,
    Reconciling,
    EvaluatingConflict,
    Planning,
    Recording,
    Executing,
    Verifying,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveAnalysis {
    pub item_id: SyncItemId,
    pub item_revision: u64,
    pub phase: AnalysisPhase,
}
pub struct AnalysisStatusRequest {
    pub item_id: SyncItemId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisStatusResult {
    pub active: Option<ActiveAnalysis>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalyzerErrorKind {
    InvalidConfiguration,
    InvalidRequest,
    WrongSyncScope,
    AlreadyRunning,
    StaleItem,
    InvalidEvidence,
    InventoryFailure,
    RuntimeUnavailable,
    Internal,
}
/// Sanitized diagnostics; dependent errors preserve only their typed category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzerError {
    pub kind: AnalyzerErrorKind,
    pub inventory_error: Option<InventoryError>,
}
impl std::fmt::Display for AnalyzerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.inventory_error {
            Some(cause) => write!(f, "analyzer operation failed: {:?} ({cause:?})", self.kind),
            None => write!(f, "analyzer operation failed: {:?}", self.kind),
        }
    }
}
impl std::error::Error for AnalyzerError {}

/// Optional execution policy for an account's live pipeline. Analysis-only
/// consumers omit it; the Analyzer, not Orchestrator, dispatches the command.
#[derive(Clone)]
pub struct AnalyzerExecution {
    pub operator: Arc<dyn crate::operations::Operator>,
    pub options: crate::operations::ExecutionOptions,
    pub observer: Arc<dyn CreationObserver>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreationStage {
    Checking,
    Blocked,
    Executing,
    Verifying,
    Completed,
    Failed,
}
#[derive(Debug, Clone)]
pub struct CreationEvent {
    pub item_id: SyncItemId,
    pub command: Option<crate::operations::OperationCommand>,
    pub operation_id: Option<crate::inventory::OperationId>,
    pub stage: CreationStage,
    pub error: Option<crate::operations::OperatorError>,
}
/// Fast in-memory diagnostics port; events never authorize or confirm an effect.
pub trait CreationObserver: Send + Sync {
    fn notify(&self, event: CreationEvent);
}
