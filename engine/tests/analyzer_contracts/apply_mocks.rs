//! Scripted port fakes. They record requests and return raw configured facts;
//! they do not implement the Apply algorithm or classify outcomes.
use super::apply_fixtures;
use drivesync_engine::{
    analyzer::{AnalysisDecision, AnalysisError, AnalysisErrorKind, AnalysisResult, DecisionTarget},
    analyzer::apply::*,
    models::File,
    operations::*,
};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Call {
    Delivery, Snapshot, Begin(File), Reserve(ReserveOperation), Claim(ReserveOperation),
    Checkpoint(OperationReceipt), Recover(ExecutionId), Record(RecordTarget),
    Confirm(BaselineConfirmation, RecordTarget), Finish(AnalysisResult),
    VerifyCreation(OperationCommand), VerifySource(SourceEvidence), Execute(ExecutionId, OperationCommand),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Fault { Delivery, Snapshot, Begin, Reserve, Claim, Checkpoint, Record, Confirm, Finish }

pub(super) struct State {
    pub calls: Vec<Call>,
    pub snapshot: ApplySnapshot,
    pub delivery: DeliveryState,
    pub fault: Option<Fault>,
    pub effect: Result<ItemResult, OperatorFailure>,
    pub integrity: IntegrityStatus,
    pub recovery: RecoveryEvidence,
    pub verification_error: Option<OperatorFailure>,
    pub reserved: Option<OperationReceipt>,
    pub source_mismatch: bool,
    pub fail_after_execute: bool,
}
pub(super) type Shared = Arc<Mutex<State>>;

pub(super) fn setup(d: &AnalysisDecision) -> Shared {
    Arc::new(Mutex::new(State {
        calls: vec![], snapshot: apply_fixtures::snapshot(d), delivery: DeliveryState::New,
        fault: None, effect: Ok(apply_fixtures::applied_result(false, true)),
        integrity: IntegrityStatus::Verified, recovery: RecoveryEvidence::Unknown,
        verification_error: None, reserved: None, source_mismatch: false, fail_after_execute: false,
    }))
}
pub(super) fn failure(kind: OperatorError, effect: ExecutionEffect) -> OperatorFailure {
    OperatorFailure { kind, effect, resource: None }
}
fn error() -> AnalysisError { AnalysisError { kind: AnalysisErrorKind::Inventory } }
fn record_call(state: &Shared, call: Call, fault: Fault) -> Result<(), AnalysisError> {
    let mut s = state.lock().unwrap();
    s.calls.push(call);
    if s.fault == Some(fault) { Err(error()) } else { Ok(()) }
}
#[derive(Clone)]
pub(super) struct Store(pub Shared);
impl ApplyInventory for Store {
    fn delivery(&self, _: &DeliveryKey, _: &File) -> Result<DeliveryState, AnalysisError> {
        record_call(&self.0, Call::Delivery, Fault::Delivery)?;
        Ok(self.0.lock().unwrap().delivery.clone())
    }
    fn snapshot(&self, _: &DeliveryKey, target: &DecisionTarget) -> Result<ApplySnapshot, AnalysisError> {
        record_call(&self.0, Call::Snapshot, Fault::Snapshot)?;
        let mut snapshot = self.0.lock().unwrap().snapshot.clone();
        if let DecisionTarget::Item { id, .. } = target {
            if let Some(item) = &mut snapshot.current_item {
                if &item.id != id { *item = super::decision_fixtures::known_subject(&id.0).item; }
            }
        }
        Ok(snapshot)
    }
    fn begin(&self, _: &DeliveryKey, file: &File) -> Result<(), AnalysisError> {
        record_call(&self.0, Call::Begin(file.clone()), Fault::Begin)
    }
    fn reserve(&self, request: &ReserveOperation) -> Result<OperationReceipt, AnalysisError> {
        record_call(&self.0, Call::Reserve(request.clone()), Fault::Reserve)?;
        let mut s = self.0.lock().unwrap();
        if let Some(old) = &s.reserved {
            if old.intent != request.intent { return Err(error()); }
            return Ok(old.clone());
        }
        let receipt = OperationReceipt { intent: request.intent.clone(), state: OperationState::Reserved, attempts: 0 };
        s.reserved = Some(receipt.clone());
        Ok(receipt)
    }
    fn claim(&self, request: &ReserveOperation) -> Result<(), AnalysisError> {
        record_call(&self.0, Call::Claim(request.clone()), Fault::Claim)
    }
    fn checkpoint(&self, _: &DeliveryKey, receipt: &OperationReceipt) -> Result<(), AnalysisError> {
        record_call(&self.0, Call::Checkpoint(receipt.clone()), Fault::Checkpoint)
    }
    fn recover(&self, _: &DeliveryKey, id: &ExecutionId) -> Result<RecoveryEvidence, AnalysisError> {
        self.0.lock().unwrap().calls.push(Call::Recover(id.clone()));
        Ok(self.0.lock().unwrap().recovery.clone())
    }
    fn record(&self, request: &RecordTarget) -> Result<(), AnalysisError> {
        record_call(&self.0, Call::Record(request.clone()), Fault::Record)
    }
    fn confirm(&self, _: &DeliveryKey, proof: &BaselineConfirmation, record: &RecordTarget) -> Result<(), AnalysisError> {
        record_call(&self.0, Call::Confirm(proof.clone(), record.clone()), Fault::Confirm)
    }
    fn finish(&self, _: &DeliveryKey, result: &AnalysisResult) -> Result<(), AnalysisError> {
        record_call(&self.0, Call::Finish(result.clone()), Fault::Finish)
    }
}

pub(super) struct FakeOperator(pub Shared);
impl FakeOperator {
    fn execute(&self, id: ExecutionId, command: OperationCommand) -> OperatorResult<ItemResult> {
        let mut s = self.0.lock().unwrap();
        s.calls.push(Call::Execute(id, command));
        if s.fail_after_execute { s.fault = Some(Fault::Checkpoint); }
        s.effect.clone()
    }
    fn transfer(&self, id: ExecutionId, command: OperationCommand) -> OperatorResult<TransferResult> {
        let item = self.execute(id, command)?;
        Ok(TransferResult { item, bytes_transferred: 23, integrity: self.0.lock().unwrap().integrity })
    }
}
impl Operator for FakeOperator {
    fn new(_: OperatorConfig) -> OperatorResult<Self> { panic!("use scripted fake constructor") }
    fn verify_creation(&self, request: VerifyCreationRequest) -> OperationFuture<'_, VerifyCreationResult> {
        Box::pin(async move {
            let mut s = self.0.lock().unwrap();
            s.calls.push(Call::VerifyCreation(request.command.clone()));
            if let Some(err) = &s.verification_error { return Err(err.clone()); }
            Ok(VerifyCreationResult { command: request.command, source: None })
        })
    }
    fn verify_source(&self, request: VerifySourceRequest) -> OperationFuture<'_, VerifiedSource> {
        Box::pin(async move {
            let mut s = self.0.lock().unwrap();
            s.calls.push(Call::VerifySource(request.source.clone()));
            if let Some(err) = &s.verification_error { return Err(err.clone()); }
            let (mut metadata, sha256) = match request.source {
                SourceEvidence::Local { metadata, sha256 } => (drivesync_engine::models::FileData::Local(*metadata), sha256),
                SourceEvidence::Remote { metadata, .. } => (drivesync_engine::models::FileData::Remote(*metadata), None),
            };
            if s.source_mismatch {
                if let drivesync_engine::models::FileData::Local(m) = &mut metadata { m.storage.size_bytes = drivesync_engine::models::MetadataValue::Known(999); }
            }
            Ok(VerifiedSource { metadata, sha256 })
        })
    }
    fn create_file(&self, r: CreateFileRequest) -> OperationFuture<'_, ItemResult> {
        Box::pin(async move { self.execute(r.execution.id, OperationCommand::CreateFile { parent: r.parent, name: r.name, content: r.content }) })
    }
    fn create_directory(&self, r: CreateDirectoryRequest) -> OperationFuture<'_, ItemResult> {
        Box::pin(async move { self.execute(r.execution.id, OperationCommand::CreateDirectory { parent: r.parent, name: r.name }) })
    }
    fn upload(&self, r: UploadRequest) -> OperationFuture<'_, TransferResult> {
        Box::pin(async move { self.transfer(r.execution.id, OperationCommand::Upload { source: r.source, destination: r.destination, name: r.name }) })
    }
    fn download(&self, r: DownloadRequest) -> OperationFuture<'_, TransferResult> {
        Box::pin(async move { self.transfer(r.execution.id, OperationCommand::Download { source: r.source, destination: r.destination, name: r.name }) })
    }
    fn update_content(&self, r: UpdateContentRequest) -> OperationFuture<'_, TransferResult> {
        Box::pin(async move { self.transfer(r.execution.id, OperationCommand::UpdateContent { target: r.target, source: r.source, preconditions: r.preconditions }) })
    }
    fn rename(&self, r: RenameRequest) -> OperationFuture<'_, ItemResult> {
        Box::pin(async move { self.execute(r.execution.id, OperationCommand::Rename { target: r.target, new_name: r.new_name, preconditions: r.preconditions }) })
    }
    fn move_item(&self, r: MoveRequest) -> OperationFuture<'_, ItemResult> {
        Box::pin(async move { self.execute(r.execution.id, OperationCommand::Move { target: r.target, destination: r.destination, preconditions: r.preconditions }) })
    }
    fn trash(&self, r: TrashRequest) -> OperationFuture<'_, ItemResult> {
        Box::pin(async move { self.execute(r.execution.id, OperationCommand::Trash { target: r.target, preconditions: r.preconditions }) })
    }
    fn restore(&self, r: RestoreRequest) -> OperationFuture<'_, ItemResult> {
        Box::pin(async move { self.execute(r.execution.id, OperationCommand::Restore { target: r.target, destination: r.destination, preconditions: r.preconditions }) })
    }
    fn delete(&self, _: DeleteRequest) -> OperationFuture<'_, DeleteResult> {
        panic!("Apply must never call permanent delete")
    }
}
pub(super) fn run(d: AnalysisDecision, state: &Shared) -> Result<AnalysisResult, AnalysisError> {
    let service = ApplyEngine::new(Store(state.clone()), Arc::new(FakeOperator(state.clone())));
    tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(service.apply(d))
}
pub(super) fn calls(state: &Shared) -> Vec<Call> { state.lock().unwrap().calls.clone() }
