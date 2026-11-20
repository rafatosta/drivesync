//! Materializes semantic decisions without reclassifying synchronization intent.
mod models;
pub use models::*;

use crate::{
    analyzer::{AnalysisDecision, AnalysisError, AnalysisFuture, AnalysisResult, DecisionTarget},
    models::File,
    operations::{ExecutionId, Operator},
};
use std::sync::Arc;

pub type ApplyFuture<'a> = AnalysisFuture<'a>;

pub trait ApplyService: Send + Sync {
    /// Validate all targets and intents before any durable or physical effect.
    /// Read the durable delivery receipt before checking stale item revisions:
    /// exact completed redelivery is Duplicate; a changed File with the same ID
    /// is invalid. Resume incomplete receipts without repeating verified effects.
    ///
    /// Persist accepted observations even when the outcome is NeedsEvidence.
    /// Never update state with an obsolete payload. NoAction, Conflict, Blocked,
    /// NeedsEvidence and Obsolete cannot dispatch a mutation. EstablishBaseline
    /// requires verified equivalent states and atomic durable confirmation.
    ///
    /// Only ActionSelected can dispatch operations. Plan exclusively from its
    /// intents with exact source/target identity, parent, scope and preconditions.
    /// Permanent Delete and unauthorized Trash are rejected; never fall back to
    /// deletion or merge. Use Operator's existing verification/operation API.
    /// Reserve an immutable ExecutionIntent before dispatch, and validate guards
    /// again atomically before claiming it. Keep ExecutionId across retries.
    ///
    /// On a verified step persist the returned physical identity, not a guessed
    /// ID. An unverified/uncertain effect requires recovery, never blind retry.
    /// A known no-effect transient failure may be durably queued for retry.
    /// Permanent failure is blocked; retry limits must be finite.
    /// After the first verified step of a multi-intent decision, preserve that
    /// step and request reanalysis before executing remaining dependent intents.
    /// No rollback of confirmed physical effects. Independent targets continue.
    /// Baseline confirmation requires all relevant dimensions verified, not
    /// merely a successful Operator response. A stale guard requests reanalysis.
    ///
    /// Ok authorizes ack ONLY after durable target outcomes, pending work and
    /// delivery completion are committed. Err means redelivery is necessary.
    /// For Completed preserve exactly one result per target in input order.
    /// Whole delivery Skipped(Obsolete) requires all targets proven obsolete.
    fn apply<'a>(&'a self, decision: AnalysisDecision) -> ApplyFuture<'a>;
}

/// Persistence port specified by Apply, not by the existing Inventory schema.
/// Operations are atomic at each boundary and use guarded writes; adapters
/// must not infer decisions, create commands or perform physical effects.
/// Receipt operations are scoped to DeliveryKey and immutable original File.
pub trait ApplyInventory: Send + Sync {
    fn delivery(&self, key: &DeliveryKey, file: &File) -> Result<DeliveryState, AnalysisError>;
    fn snapshot(&self, key: &DeliveryKey, target: &DecisionTarget) -> Result<ApplySnapshot, AnalysisError>;
    /// Atomically reserve the delivery and retain the original observation.
    /// Existing receipt must be reused; obsolete observations never replace facts.
    fn begin(&self, key: &DeliveryKey, file: &File) -> Result<(), AnalysisError>;
    /// Atomically bind command and execution ID under revision/scope guards.
    /// Repeated identical reservation returns the same receipt.
    fn reserve(&self, request: &ReserveOperation) -> Result<OperationReceipt, AnalysisError>;
    /// Atomic claim rejects competing execution and stale guards. Must not
    /// permit a second claim after a dispatched/uncertain effect.
    fn claim(&self, request: &ReserveOperation) -> Result<(), AnalysisError>;
    fn checkpoint(&self, key: &DeliveryKey, receipt: &OperationReceipt) -> Result<(), AnalysisError>;
    /// Read durable Operator checkpoint facts, including effects after a crash.
    fn recover(&self, key: &DeliveryKey, id: &ExecutionId) -> Result<RecoveryEvidence, AnalysisError>;
    /// Persist outcome and observed state or pending work under the target guard.
    /// Does not itself confirm a baseline.
    fn record(&self, request: &RecordTarget) -> Result<(), AnalysisError>;
    /// Compare-and-commit verified baseline, target outcome and operation results.
    fn confirm(&self, key: &DeliveryKey, proof: &BaselineConfirmation, record: &RecordTarget) -> Result<(), AnalysisError>;
    /// Atomically check that every target is durably handled, then finalize receipt.
    fn finish(&self, key: &DeliveryKey, result: &AnalysisResult) -> Result<(), AnalysisError>;
}

/// Concrete test entry point. No planning/execution algorithms yet.
pub struct ApplyEngine<I> {
    inventory: I,
    operator: Arc<dyn Operator>,
}

impl<I: ApplyInventory> ApplyEngine<I> {
    pub fn new(inventory: I, operator: Arc<dyn Operator>) -> Self {
        Self { inventory, operator }
    }
}

impl<I: ApplyInventory> ApplyService for ApplyEngine<I> {
    fn apply<'a>(&'a self, decision: AnalysisDecision) -> ApplyFuture<'a> {
        Box::pin(async move {
            let _ = (&self.inventory, &self.operator, decision);
            todo!("implement Apply after its behavioral contract is approved")
        })
    }
}
