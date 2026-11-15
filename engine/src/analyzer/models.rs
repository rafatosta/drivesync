//! Public analysis-only coordination contracts. Execution and resolution are deferred.
use crate::{
    inventory::{
        AnalysisEvidence, InventoryError, InventoryManager, RecordedAnalysis, SyncIdentity,
        SyncItem, SyncItemId,
    },
    models::SyncId,
};
use std::sync::Arc;

pub type AnalyzerResult<T> = Result<T, AnalyzerError>;
/// The Inventory handle is scoped and shared; no SQL, Provider or Operator leaks.
pub struct AnalyzerDependencies {
    pub inventory: Arc<InventoryManager>,
}
pub struct AnalyzerConfig {
    pub identity: SyncIdentity,
    pub dependencies: AnalyzerDependencies,
}
/// One exact inventory snapshot and measurements bound to that revision.
/// New evidence or observation requires a new analysis; stale effects are never replayed.
#[derive(Debug, Clone)]
pub struct AnalyzeRequest {
    pub directive: Option<crate::operations::RestoreSelection>,
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
        write!(f, "analyzer operation failed: {:?}", self.kind)
    }
}
impl std::error::Error for AnalyzerError {}
