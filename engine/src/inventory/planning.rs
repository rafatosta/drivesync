//! Durable plan data, explicit user identity choices and revision-bound observations.
use super::*;
use crate::{
    models::Timestamp,
    operations::{DirectoryRef, OperationCommand, ResourceRef},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Fingerprints physical/logical evidence, not diagnostic status or analysis-only
/// revision increments. A later changed observation invalidates dependent plans.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemGuard {
    pub item_id: SyncItemId,
    pub fingerprint: Vec<u8>,
}
impl ItemGuard {
    pub fn for_item(item: &SyncItem) -> InventoryResult<Self> {
        let evidence = (
            &item.kind,
            &item.local_file_id,
            &item.remote_ref,
            &item.local,
            &item.remote,
            &item.local_latest,
            &item.remote_latest,
            &item.baseline,
            &item.local_presence,
            &item.remote_presence,
        );
        let bytes = serde_json::to_vec(&evidence)
            .map_err(|_| InventoryFailure::new(InventoryError::SerializationFailed))?;
        Ok(Self {
            item_id: item.id.clone(),
            fingerprint: Sha256::digest(bytes).to_vec(),
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanBlock {
    Evidence,
    Identity,
    Destination,
    Parent,
    Collision,
    Conflict,
    Capabilities,
    UserAuthorization,
    PriorStep,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlannedMethod {
    CreateFile,
    CreateDirectory,
    Upload,
    Download,
    UpdateContent,
    Rename,
    Move,
    Trash,
    Restore,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedStep {
    pub method: PlannedMethod,
    pub command: Option<OperationCommand>,
    pub blocked_by: Vec<PlanBlock>,
    pub depends_on: Vec<SyncItemId>,
    pub depends_on_steps: Vec<u32>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManualReason {
    ConcurrentChanges,
    DeleteVsChange,
    DestinationOccupied,
    AssociationRequired,
    MissingBaseline,
    DeletionAuthorization,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualIssue {
    pub reason: ManualReason,
    pub resources: Vec<ResourceRef>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisPlan {
    pub restore_request: Option<crate::operations::RestoreSelection>,
    pub namespaces: Vec<NamespaceGuard>,
    pub steps: Vec<PlannedStep>,
    pub guards: Vec<ItemGuard>,
    pub manual: Vec<ManualIssue>,
}
/// Explicit choice only; no heuristic matching or inferred user consent.
#[derive(Debug, Clone)]
pub struct AssociateItemsRequest {
    pub expected_local: SyncItem,
    pub expected_remote: SyncItem,
    pub decision_id: String,
    pub decided_at: Timestamp,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemAssociation {
    pub canonical_item: SyncItemId,
    pub retired_item: SyncItemId,
    pub decision_id: String,
    pub decided_at: Timestamp,
}
#[derive(Debug, Clone)]
pub struct AssociateItemsResult {
    pub item: SyncItem,
    pub association: ItemAssociation,
}
#[derive(Debug, Clone)]
pub struct GetAssociationRequest {
    pub item_id: SyncItemId,
}
#[derive(Debug, Clone)]
pub struct GetAssociationResult {
    pub association: Option<ItemAssociation>,
}

/// A destination child-set guard detects insertions as well as modifications.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamespaceGuard {
    pub directory: DirectoryRef,
    pub fingerprint: Vec<u8>,
}
pub struct GetNamespaceRequest {
    pub directory: DirectoryRef,
}
pub struct GetNamespaceResult {
    pub guard: NamespaceGuard,
}
