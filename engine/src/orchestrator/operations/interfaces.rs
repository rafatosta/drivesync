//! Private asynchronous executor and provider factory boundaries.
use super::{filesystem::Filesystem, journal::ExecutionJournal, models::*};
use crate::models::RemoteId;
use std::{future::Future, pin::Pin};
pub(crate) type OperationFuture<'a> = Pin<Box<dyn Future<Output = OperationResult> + Send + 'a>>;
pub(super) type EffectFuture<'a> =
    Pin<Box<dyn Future<Output = Result<AppliedEffect, OperatorError>> + Send + 'a>>;
/// Executes one authorized operation and persists its real result.
pub(crate) type RecoveryFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<RecoveryResult>, OperatorError>> + Send + 'a>>;
pub(crate) trait Operator: Send + Sync {
    fn record_conflict<'a>(
        &'a self,
        _file: &'a crate::detectors::context::File,
        _reason: String,
    ) -> Pin<Box<dyn Future<Output = Result<(), OperatorError>> + Send + 'a>> {
        Box::pin(async { Err(OperatorError::new(OperatorErrorKind::UnsupportedOperation)) })
    }
    fn recover(&self) -> RecoveryFuture<'_>;
    fn execute<'a>(&'a self, request: ExecuteOperationRequest) -> OperationFuture<'a>;
}
/// Dispatches exclusively by the provider of the configured account.
pub(crate) trait OperatorFactory: Send + Sync {
    fn create(&self, config: OperatorConfig) -> Result<Box<dyn Operator>, OperatorError>;
}
/// Provider specialization; local filesystem execution remains owned by Operations.
pub(super) trait RemoteExecutor: Send + Sync {
    fn reconcile<'a>(
        &'a self,
        _: &'a OperatorConfig,
        _: &'a crate::inventory::models::RecoveryRecord,
        _: &'a Filesystem,
    ) -> Pin<Box<dyn Future<Output = RecoveryAssessment> + Send + 'a>> {
        Box::pin(async {
            RecoveryAssessment::unknown(crate::inventory::models::RecoveryIssue::UnsupportedAction)
        })
    }
    /// Continue only a persisted upload session after current progress and source validation.
    fn resume_upload<'a>(
        &'a self,
        _: &'a OperatorConfig,
        _: &'a crate::inventory::models::RecoveryRecord,
        _: &'a Filesystem,
        _: &'a dyn ExecutionJournal,
    ) -> Pin<Box<dyn Future<Output = RecoveryAssessment> + Send + 'a>> {
        Box::pin(async {
            RecoveryAssessment::unknown(crate::inventory::models::RecoveryIssue::TransferIncomplete)
        })
    }
    /// Reserve a recoverable remote create identity before committing dispatch intent.
    fn reserve<'a>(
        &'a self,
        _: &'a OperatorConfig,
        request: &'a ExecuteOperationRequest,
    ) -> Pin<Box<dyn Future<Output = Result<Option<RemoteId>, OperatorError>> + Send + 'a>> {
        Box::pin(async move {
            if matches!(
                request.operation.action,
                OperationAction::Upload | OperationAction::CreateRemoteDirectory
            ) {
                Err(OperatorError::new(OperatorErrorKind::UnsupportedOperation))
            } else {
                Ok(None)
            }
        })
    }
    fn execute<'a>(
        &'a self,
        config: &'a OperatorConfig,
        request: &'a ExecuteOperationRequest,
        filesystem: &'a Filesystem,
        journal: Option<&'a dyn ExecutionJournal>,
    ) -> EffectFuture<'a>;
}
