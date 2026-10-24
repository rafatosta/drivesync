//! Incremental remote observations; never installs or deletes user files.
//! A terminal feed cursor and its projected inventory form one checkpoint.
mod collection;
mod models;
pub(crate) use collection::validate;
pub(crate) use collection::{collect, previous_paths, reconcile, Provider};
pub(crate) use models::{Checkpoint, Notice};
#[cfg(test)]
mod tests;
