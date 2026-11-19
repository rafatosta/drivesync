//! Builds the inventory-backed context consumed by the Analyzer decision stage.
mod models;
pub use models::*;

use crate::{analyzer::AnalysisError, models::File};
use std::{future::Future, pin::Pin};

/// Boxed future returned while the context stage reads current state and evidence.
pub type ContextFuture<'a> =
    Pin<Box<dyn Future<Output = Result<AnalysisContext, AnalysisError>> + Send + 'a>>;

/// Reads the current inventory state relevant to one detector file.
pub trait ContextService: Send + Sync {
    /// Converts a detector file into the file plus its related inventory snapshots.
    ///
    /// `items` is empty for an observation not yet represented in the inventory.
    /// A whole-synchronization observation may return several item contexts.
    /// This stage reads state and gathers evidence; it does not write inventory
    /// decisions or infer identity from matching names.
    fn build<'a>(&'a self, file: File) -> ContextFuture<'a>;
}
