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
    /// Preserve the original File and the inventory snapshots unchanged.
    /// Distinguish matched, unmatched, ambiguous and whole-sync observations.
    /// A missing row never proves absence; incomplete observations remain unknown.
    /// Only evidence measured at the snapshot revision may be associated with it.
    /// A stale measurement is discarded rather than attached to a newer revision.
    /// Reject input outside the inventory synchronization/account; do not return
    /// candidates belonging to another account. Return Err on inventory failure.
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

/// Facts returned by a read-only inventory adapter. Exact matches are based on
/// qualified remote identity or persistent local identity; names and paths
/// alone cannot establish a link. Possible matches are not authoritative.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextInventorySnapshot {
    pub identity: SyncIdentity,
    pub observation: ObservationContext,
    pub candidates: Vec<ContextCandidate>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextCandidate {
    pub item: SyncItem,
    pub exact_identity: bool,
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
