//! Optimistic execution checkpoints committed before any staged publication or upload continuation.
use super::*;
use std::sync::Mutex;

pub(super) trait ExecutionJournal: Send + Sync {
    fn record(&self) -> RecoveryRecord;
    fn checkpoint(&self, checkpoint: RecoveryCheckpoint) -> Result<(), OperatorError>;
}
pub(super) struct Journal {
    inventory: Arc<Inventory>,
    request: ExecuteOperationRequest,
    state: Mutex<(u64, RecoveryRecord)>,
}
impl Journal {
    pub(super) fn new(
        inventory: Arc<Inventory>,
        request: ExecuteOperationRequest,
        revision: u64,
        record: RecoveryRecord,
    ) -> Self {
        Self {
            inventory,
            request,
            state: Mutex::new((revision, record)),
        }
    }
    pub(super) fn revision(&self) -> Option<u64> {
        self.state.lock().ok().map(|state| state.0)
    }
}
impl ExecutionJournal for Journal {
    fn record(&self) -> RecoveryRecord {
        self.state.lock().unwrap().1.clone()
    }
    fn checkpoint(&self, checkpoint: RecoveryCheckpoint) -> Result<(), OperatorError> {
        let phase = if matches!(&checkpoint, RecoveryCheckpoint::Transfer(_)) {
            Phase::CheckpointTransfer
        } else {
            Phase::Checkpoint
        };
        let mut span = Span::start(phase, State::Waiting(Wait::Inventory));
        let inventory_context = span.activity_context();
        let result = (|| {
            let mut state = self.state.lock().map_err(|_| filesystem::stale())?;
            let receipt = self.inventory.write_with_context(
                InventoryWrite {
                    operation_id: self.request.execution_id.clone(),
                    item_id: self.request.operation.item_ref.clone(),
                    identity: self.request.identity.clone(),
                    expected_revision: Some(state.0),
                    effect: PersistedEffect::RecoveryCheckpoint(checkpoint.clone()),
                    recorded_at: now(),
                },
                inventory_context.as_ref(),
            );
            span.receipt(receipt.status);
            if receipt.status != WriteStatus::Written {
                return Err(filesystem::stale().uncertain());
            }
            state.0 = receipt.revision.ok_or_else(filesystem::stale)?;
            match checkpoint {
                RecoveryCheckpoint::LocalTrash(reference) => state.1.local_trash = Some(reference),
                RecoveryCheckpoint::ConflictArchive(id) => state.1.conflict_archive = Some(id),
                RecoveryCheckpoint::LocalStage(identity) => state.1.stage_identity = Some(identity),
                RecoveryCheckpoint::Transfer(transfer) => state.1.transfer = Some(transfer),
                RecoveryCheckpoint::RemoteAncestry(ancestors) => {
                    state.1.remote_ancestry = Some(ancestors)
                }
            }
            Ok(())
        })();
        span.result(&result);
        result
    }
}
