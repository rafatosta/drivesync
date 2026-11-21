//! Private asynchronous executor and provider factory boundaries.
use super::{filesystem::Filesystem, models::*};
use std::{future::Future, pin::Pin};
pub(crate) type OperationFuture<'a> = Pin<Box<dyn Future<Output = OperationResult> + Send + 'a>>;
pub(super) type EffectFuture<'a> =
    Pin<Box<dyn Future<Output = Result<AppliedEffect, OperatorError>> + Send + 'a>>;
/// Executes one authorized operation and persists its real result.
pub(crate) trait Operator: Send + Sync {
    fn execute<'a>(&'a self, request: ExecuteOperationRequest) -> OperationFuture<'a>;
}
/// Dispatches exclusively by the provider of the configured account.
pub(crate) trait OperatorFactory: Send + Sync {
    fn create(&self, config: OperatorConfig) -> Result<Box<dyn Operator>, OperatorError>;
}
/// Provider specialization; local filesystem execution remains owned by Operations.
pub(super) trait RemoteExecutor: Send + Sync {
    fn execute<'a>(
        &'a self,
        config: &'a OperatorConfig,
        request: &'a ExecuteOperationRequest,
        filesystem: &'a Filesystem,
    ) -> EffectFuture<'a>;
}
