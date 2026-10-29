//! Incremental remote observations; never installs or deletes user files.
//! A terminal feed cursor and its projected inventory form one checkpoint.
mod collection;
mod equivalence;
mod models;
mod reconciliation;
pub(crate) use collection::validate;
pub(crate) use collection::{collect, previous_paths, reconcile, Provider};
pub(crate) use equivalence::prepare;
pub(crate) use models::{Checkpoint, Notice};
pub(crate) use reconciliation::classify;
pub use reconciliation::{ChangeKind, ReconciliationEntry};
#[cfg(test)]
mod tests;
