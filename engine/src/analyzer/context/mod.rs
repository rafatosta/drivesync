//! Builds the inventory-backed context consumed by the Analyzer decision stage.
mod models;
pub use models::*;

use crate::{
    analyzer::AnalysisError,
    inventory::{AnalysisEvidence, SyncIdentity, SyncItem},
    models::File,
};
use std::{future::Future, pin::Pin};

/// Boxed future returned while the context stage reads current state and evidence.
pub type ContextFuture<'a> =
    Pin<Box<dyn Future<Output = Result<AnalysisContext, AnalysisError>> + Send + 'a>>;

/// Reads the current inventory state relevant to one detector file.
pub trait ContextService: Send + Sync {
    /// Converts a detector file into the file plus its related inventory snapshots.
    ///
    /// Preserve File and inventory snapshots independently; never merge them.
    /// Validate File before reading. Invalid File, wrong synchronization/account,
    /// or a foreign candidate returns Err; failed reads propagate.
    /// Match qualified remote identity or persistent local identity only.
    /// A path/name candidate without matching identity is Ambiguous, not Matched.
    /// A matching identity plus a different destination candidate is Ambiguous.
    /// Unrelated candidates are excluded; a successful empty lookup is Unmatched.
    /// WholeSync is Scope, including when empty. Deduplicate identical rows by ID;
    /// reject contradictory snapshots of the same ID.
    /// Incoming presence derives only from File, not missing rows or event labels.
    /// RemovedFromScope alone does not prove deletion. Do not infer opposite-side
    /// absence or content equality; preserve inventory presence separately.
    /// Evidence must match the item revision; otherwise use unknown evidence.
    /// Compare numeric remote versions first when both are comparable, otherwise
    /// durable causal orders on the same side; never use wall-clock timestamps.
    /// Missing comparable markers or whole-sync scope means Incomparable.
    /// No writes, filesystem/provider access, reobservation requests, conflict
    /// classification or operation selection are permitted.
    fn build<'a>(&'a self, file: File) -> ContextFuture<'a>;
}

/// Read-only input port for the Context stage. Implementations must use the
/// Inventory API only; no physical observation or mutation is allowed.
/// All candidates must originate from the same synchronization as identity.
pub trait ContextInventory: Send + Sync {
    fn read(&self, file: &File) -> Result<ContextInventorySnapshot, AnalysisError>;
}

/// Raw candidate snapshots returned by the read-only adapter. It may find
/// candidates by explicit identity or plausible destination/name/path lookup,
/// but must not decide the association. No physical observation is performed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextInventorySnapshot {
    pub identity: SyncIdentity,
    pub candidates: Vec<ContextCandidate>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextCandidate {
    pub item: SyncItem,
    /// Raw durable order markers from Inventory, not a freshness verdict.
    pub local_causal_order: Option<u64>,
    pub remote_causal_order: Option<u64>,
    /// Missing or stale measurements must become unknown in the output.
    pub evidence: Option<RevisionEvidence>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RevisionEvidence {
    pub revision: u64,
    pub evidence: AnalysisEvidence,
}

/// Concrete entry point for behavior-first tests. The inventory adapter and
/// context assembly are deliberately not implemented in this TDD step.
pub struct ContextBuilder<I> {
    inventory: I,
}

impl<I: ContextInventory> ContextBuilder<I> {
    pub fn new(inventory: I) -> Self {
        Self { inventory }
    }
}

impl<I: ContextInventory> ContextService for ContextBuilder<I> {
    fn build<'a>(&'a self, file: File) -> ContextFuture<'a> {
        Box::pin(async move {
            let _ = (&self.inventory, file);
            todo!("implement Context after its behavioral contract is approved")
        })
    }
}
