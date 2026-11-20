//! Apply outputs and persistence-port artifacts; no storage or execution logic.
use crate::{
    analyzer::{AnalysisContext, AnalysisItemDecision, AnalysisResult, DecisionTarget, NonEmpty},
    inventory::{AnalysisRequirement, SyncItem, SyncItemId},
    models::{FileId, SyncId},
    operations::{DirectoryRef, ExecutionId, ExecutionIntent, ExecutionScope, ItemResult, OperatorFailure, SourceEvidence, VerifiedSource},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalysisItemResult {
    pub target: DecisionTarget,
    pub outcome: ApplyOutcome,
}

/// Acknowledgement describes durable handling, not necessarily synchronization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplyOutcome {
    NoAction,
    BaselineEstablished,
    ConflictRecorded,
    EvidenceRequested { requirements: NonEmpty<AnalysisRequirement> },
    Blocked { reason: ApplyBlock },
    Obsolete,
    PendingExecution { operations: NonEmpty<ExecutionId> },
    Applied { operations: NonEmpty<AppliedOperation> },
    PendingRecovery { operations: NonEmpty<ExecutionId> },
    PendingReanalysis { completed: Vec<AppliedOperation>, reason: ReanalysisReason },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppliedOperation {
    pub id: ExecutionId,
    pub result: ItemResult,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyBlock {
    UnsupportedKind,
    UnsupportedOperation,
    PermissionDenied,
    RetryExhausted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReanalysisReason {
    StaleRevision,
    DestinationOccupied,
    SourceChanged,
    PriorStepCompleted,
    NewObservation,
}

/// Retry identity is qualified by the configured synchronization and delivery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveryKey {
    pub sync_id: SyncId,
    pub file_id: FileId,
}

/// Scope and raw historical facts needed to resolve a physical parent. These
/// facts do not select commands or declare a destination physically free.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplySnapshot {
    pub scope: ExecutionScope,
    pub current_item: Option<SyncItem>,
    pub related_parents: Vec<ParentBinding>,
    /// Existing durable authorization evidence for the selected target/revision.
    pub deletion_authorized: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParentBinding {
    pub item_id: SyncItemId,
    pub local: Option<DirectoryRef>,
    pub remote: Option<DirectoryRef>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeliveryState {
    New,
    Completed(AnalysisResult),
    InProgress(Vec<TargetReceipt>),
    /// Same delivery identity bound to a different immutable File.
    IdentityMismatch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetReceipt {
    pub target: DecisionTarget,
    pub outcome: Option<ApplyOutcome>,
    pub operations: Vec<OperationReceipt>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationReceipt {
    pub intent: ExecutionIntent,
    pub state: OperationState,
    pub attempts: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OperationState {
    Reserved,
    Dispatched,
    RetryPending(OperatorFailure),
    RecoveryRequired,
    Verified(ItemResult),
}

/// A guarded persistence request authored by Apply, not a storage-side decision.
/// The exact context snapshot remains separate from newly observed facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordTarget {
    pub delivery: DeliveryKey,
    pub context: AnalysisContext,
    pub decision: AnalysisItemDecision,
    pub outcome: ApplyOutcome,
}

/// Complete post-effect evidence. Verification does not itself persist a baseline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaselineConfirmation {
    pub target: DecisionTarget,
    pub local: VerifiedSource,
    pub remote: VerifiedSource,
}

/// All execution and verification data are bound to a previously reserved intent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReserveOperation {
    pub delivery: DeliveryKey,
    pub target: DecisionTarget,
    pub intent: ExecutionIntent,
}

/// A recovered effect is a checkpoint fact, not permission to execute again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecoveryEvidence {
    NotApplied,
    Applied(ItemResult),
    Unknown,
}

/// Read-only source guard for the surviving sides, derived by Apply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerificationSources {
    pub local: SourceEvidence,
    pub remote: SourceEvidence,
}
