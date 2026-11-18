//! Durable analysis facts and semantic work intent, independent of Analyzer algorithms.
use super::{
    ItemKind, OperationDirection, OperationId, OperationKind, Page, Pagination, SyncItem,
    SyncItemId,
};
use crate::models::{SyncId, Timestamp};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AnalysisId(pub i64);
/// Explicit caller measurements, never inferred from size, timestamps or missing rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceRelation {
    Equal,
    Different,
    Unknown,
}
/// Revision-bound evidence retained alongside the decision that consumed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisEvidence {
    pub baseline_confirmed: bool,
    pub local_scope_complete: bool,
    pub remote_scope_complete: bool,
    pub local_content: EvidenceRelation,
    pub remote_content: EvidenceRelation,
    pub current_content: EvidenceRelation,
    pub local_parent: Option<SyncItemId>,
    pub current_parents: EvidenceRelation,
}
/// These are pending analysis results, never confirmation of physical effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnalysisDisposition {
    NoChanges,
    PendingOperations,
    PendingEvidence,
    PendingUserDecision,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnalysisOrigin {
    Reconciliation,
    Conflicts,
    Planning,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnalysisDimension {
    Presence,
    Content,
    Name,
    Location,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnalysisFindingStatus {
    Compatible,
    Divergent,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnalysisReason {
    Equivalent,
    UnilateralChange,
    ConcurrentChanges,
    DeleteVsChange,
    NewItem,
    MissingBaseline,
    InsufficientEvidence,
    UnsupportedKind,
    IdentityAmbiguous,
    IncompatibleStates,
    InconsistentEvidence,
    DestinationOccupied,
    UserRequested,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisFinding {
    pub origin: AnalysisOrigin,
    pub dimension: AnalysisDimension,
    pub status: AnalysisFindingStatus,
    pub reason: AnalysisReason,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnalysisRequirement {
    ObserveLocal,
    ObserveRemote,
    VerifyIdentity,
    VerifyBaseline,
    VerifyContent,
    VerifyName,
    VerifyParent,
    VerifySurvivingState,
    AuthorizeDeletion,
    ResolveDestination,
    VerifyKind,
    VerifyCapabilities,
    VerifyPreconditions,
}
/// An incomplete semantic intent. It carries no destination, execution command,
/// or permission to run; each accepted intent must pass planning and policy checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisIntent {
    pub direction: OperationDirection,
    pub kind: OperationKind,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewAnalysisAssessment {
    pub plan: Option<super::AnalysisPlan>,
    pub disposition: AnalysisDisposition,
    pub findings: Vec<AnalysisFinding>,
    pub requirements: Vec<AnalysisRequirement>,
    pub intents: Vec<AnalysisIntent>,
}
/// Atomic preconditions include the full current item; an equal ID/revision alone
/// cannot authorize recording an assessment of caller-modified snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordAnalysisRequest {
    pub expected_sync_id: SyncId,
    pub expected_item: SyncItem,
    pub evidence: AnalysisEvidence,
    pub assessment: NewAnalysisAssessment,
    pub recorded_at: Timestamp,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisDecision {
    pub intent: AnalysisIntent,
    pub operation_id: OperationId,
}
/// Append-only evaluation facts. `stale` is computed against the current item.
/// Blocked operations are linked explicitly; no executable plan is persisted yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedAnalysis {
    pub plan: Option<super::AnalysisPlan>,
    pub id: AnalysisId,
    pub item_id: SyncItemId,
    pub item_kind: ItemKind,
    pub analyzed_item_revision: u64,
    pub resulting_item_revision: u64,
    pub input_fingerprint: Vec<u8>,
    pub recorded_at: Timestamp,
    pub evidence: AnalysisEvidence,
    pub disposition: AnalysisDisposition,
    pub findings: Vec<AnalysisFinding>,
    pub requirements: Vec<AnalysisRequirement>,
    pub decisions: Vec<AnalysisDecision>,
    pub stale: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordAnalysisResult {
    pub analysis: RecordedAnalysis,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetAnalysisRequest {
    pub selector: AnalysisSelector,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnalysisSelector {
    Id(AnalysisId),
    LatestForItem(SyncItemId),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetAnalysisResult {
    pub analysis: Option<RecordedAnalysis>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AnalysisFilter {
    pub item_id: Option<SyncItemId>,
    pub dispositions: Vec<AnalysisDisposition>,
    /// Excludes evaluations invalidated by a newer item revision.
    pub current_only: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListAnalysesRequest {
    pub filter: AnalysisFilter,
    pub pagination: Pagination,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListAnalysesResult {
    pub page: Page<RecordedAnalysis>,
}
