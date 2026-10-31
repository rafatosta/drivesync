//! Recoverable same-identity conflict decisions and independent rename/content effects.
pub(crate) mod execution;
pub(crate) mod guard;
mod local;
mod models;
pub(crate) mod planning;
pub(crate) mod progress;
pub use progress::{ResolutionProgress, ResolutionState};
pub(crate) mod provider;
pub use execution::ResolutionReport;
pub(crate) use models::*;
pub use models::{ResolutionChoice, ResolutionRequest};
#[cfg(test)]
mod tests;
