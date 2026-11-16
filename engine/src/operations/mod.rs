//! Provider-selected file operations, durable recovery and verified effects.
//!
//! `DefaultOperatorFactory` resolves the persisted account and constructs a
//! `GoogleOperator`. New blob upload/download execute on owned blocking workers,
//! using Providers, private staging and Inventory checkpoints. Metadata changes,
//! replacement, trash and restore verify identity and reject unavailable capabilities;
//! permanent deletion refuses locally.
//!
//! Names and matching bytes never associate preexisting resources. No execution
//! releases Analyzer queue entries, confirms a baseline or drives a GUI.
//!
//! ```no_run
//! use std::sync::Arc;
//! use drivesync_engine::operations::{Operator, RenameRequest, OperatorResult};
//! async fn rename(operator: Arc<dyn Operator>, request: RenameRequest)
//!     -> OperatorResult<()> {
//!     let result = operator.rename(request).await?;
//!     // The caller must inspect verification before confirming any baseline.
//!     let _ = result;
//!     Ok(())
//! }
//! ```
mod filesystem;
mod google;
pub use google::{DefaultOperatorFactory, GoogleOperator};
mod interfaces;
mod models;
mod recovery;

pub use interfaces::*;
pub use models::*;
pub use recovery::*;
