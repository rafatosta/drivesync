//! Private coordination scope and outcomes. No alternate synchronization file model.
use super::{analyzer::DecisionStatus, operations::models::OperationResult};
use crate::{
    models::{ProviderId, SyncRootId},
    synchronization::StartSynchronizationRequest,
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OrchestratorConfig {
    pub trash_propagation_authorized: bool,
    pub initial_association_authorized: bool,
    pub roots: StartSynchronizationRequest,
    pub sync_root_id: SyncRootId,
    pub provider_id: ProviderId,
}
/// Retained only for an individual processing call, not a second persistent queue.
#[derive(Debug)]
pub(super) struct ProcessingOutcome {
    pub decision: DecisionStatus,
    pub results: Vec<OperationResult>,
}
