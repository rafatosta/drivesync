//! Contracts for provider-selected operations consumed by the Analyzer.
//!
//! This module declares data and interfaces only. There is no concrete Operator,
//! factory, provider registry, filesystem helper or execution implementation.
//! In particular, an operation cannot report success without an executor.
//!
//! Future composition follows the remote detector boundary: resolve the provider
//! from the persisted account through Engine, then select its specialist. Only
//! specialists access Providers; they may also perform scoped local work for
//! downloads. Names and matching content never establish resource association.
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
mod interfaces;
mod models;

pub use interfaces::*;
pub use models::*;
