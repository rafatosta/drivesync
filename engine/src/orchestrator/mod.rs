//! Account-scoped coordination of pure analysis, ordered execution, and status publication.
mod analyzer;
mod evidence;
mod models;
mod operations;
use crate::{
    accounts::Accounts,
    detectors::context::File,
    inventory::{models::WriteStatus, Inventory, InventoryConfig},
    models::{DateTime, Evidence, Id},
    providers::google::GoogleDriveClient,
    synchronization::*,
};
use analyzer::{DecisionStatus, ExecutionStatus};
pub(crate) use models::OrchestratorConfig;
use models::ProcessingOutcome;
use operations::{
    models::{
        ExecuteOperationRequest, OperationAction, OperationResultStatus, OperatorConfig,
        OperatorErrorKind,
    },
    Operator, OperatorFactory,
};
use std::{
    collections::BTreeMap,
    io::Read,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::{mpsc, watch, RwLock};
const QUEUE_CAPACITY: usize = 32;

/// Producer handle to a bounded per-account File queue. It exposes no Analyzer or Operator.
pub(crate) struct OrchestratorHandle {
    config: OrchestratorConfig,
    files: mpsc::Sender<Command>,
    stop: watch::Sender<bool>,
    finished: watch::Receiver<bool>,
    status: watch::Receiver<SynchronizationStatus>,
}
impl Drop for OrchestratorHandle {
    fn drop(&mut self) {
        self.stop.send_replace(true);
    }
}
impl OrchestratorHandle {
    pub(crate) fn config(&self) -> &OrchestratorConfig {
        &self.config
    }
    pub(crate) fn is_finished(&self) -> bool {
        *self.finished.borrow()
    }
    pub(crate) fn status(&self) -> SynchronizationStatus {
        self.status.borrow().clone()
    }
    pub(crate) fn subscribe(&self) -> SynchronizationSubscription {
        SynchronizationSubscription {
            receiver: self.status.clone(),
        }
    }
    /// Backpressure keeps observations bounded; a stopped worker rejects new work.
    pub(crate) async fn submit(&self, file: File) -> Result<(), SynchronizationErrorKind> {
        if *self.stop.borrow() || self.is_finished() {
            return Err(SynchronizationErrorKind::Stopped);
        }
        self.files
            .send(Command::File(Box::new(file)))
            .await
            .map_err(|_| SynchronizationErrorKind::Stopped)
    }
    pub(crate) async fn source_status(
        &self,
        source: Source,
        scanning: bool,
        issue: Option<SynchronizationIssue>,
    ) -> Result<(), SynchronizationErrorKind> {
        self.files
            .send(Command::Source(source, scanning, issue))
            .await
            .map_err(|_| SynchronizationErrorKind::Stopped)
    }
    /// Stops accepting work, drains only the currently executing step, and waits for termination.
    pub(crate) async fn stop(&self) -> SynchronizationStatus {
        self.stop.send_replace(true);
        let mut finished = self.finished.clone();
        while !*finished.borrow_and_update() {
            if finished.changed().await.is_err() {
                break;
            }
        }
        self.status()
    }
}
/// Opens the existing Inventory and operator services without collecting or interpreting evidence.
pub(crate) fn open(
    config: OrchestratorConfig,
    directory: PathBuf,
    accounts: Accounts,
    client: GoogleDriveClient,
    ownership: Arc<RwLock<()>>,
) -> Result<OrchestratorHandle, SynchronizationErrorKind> {
    let inventory = Arc::new(
        Inventory::open(InventoryConfig {
            directory,
            account_id: config.roots.account_id.clone(),
        })
        .map_err(|_| SynchronizationErrorKind::InventoryUnavailable)?,
    );
    let operator = operations::DefaultOperatorFactory::new(accounts, inventory, client)
        .create(OperatorConfig {
            initial_association_authorized: config.initial_association_authorized,
            account_id: config.roots.account_id.clone(),
            sync_root_id: config.sync_root_id.clone(),
            provider_id: config.provider_id.clone(),
            local_root: config.roots.local_root.clone(),
            remote_root_id: config.roots.remote_root_id.clone(),
        })
        .map_err(|error| match error.code {
            OperatorErrorKind::UnsupportedProvider => SynchronizationErrorKind::UnsupportedProvider,
            OperatorErrorKind::PermissionDenied => SynchronizationErrorKind::AccountUnavailable,
            _ => SynchronizationErrorKind::LocalRootUnavailable,
        })?;
    Ok(start(config, operator, ownership, QUEUE_CAPACITY))
}
fn start(
    config: OrchestratorConfig,
    operator: Box<dyn Operator>,
    ownership: Arc<RwLock<()>>,
    capacity: usize,
) -> OrchestratorHandle {
    let mut initial = SynchronizationStatus::new(
        config.roots.account_id.clone(),
        SynchronizationState::Waiting,
    );
    initial.activity = SynchronizationActivity::Observing;
    let (status, status_rx) = watch::channel(initial);
    let (stop, stop_rx) = watch::channel(false);
    let (finished, finished_rx) = watch::channel(false);
    let (files, files_rx) = mpsc::channel(capacity);
    let mut worker = Orchestrator {
        config: config.clone(),
        operator,
        status: status.clone(),
        stop: stop_rx,
        blockers: BTreeMap::new(),
        sources: BTreeMap::new(),
        recovery_failed: false,
    };
    let task = tokio::spawn(async move {
        worker.run(files_rx, ownership).await;
    });
    tokio::spawn(async move {
        if task.await.is_err() {
            let mut failure = status.borrow().clone();
            failure.state = SynchronizationState::Failed;
            failure.activity = SynchronizationActivity::None;
            failure.issue = Some(issue(SynchronizationIssueKind::WorkerFailed, false));
            failure.updated_at = timestamp();
            status.send_replace(failure);
        }
        finished.send_replace(true);
    });
    OrchestratorHandle {
        config,
        files,
        stop,
        finished: finished_rx,
        status: status_rx,
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Source {
    Local,
    Remote,
}
enum Command {
    File(Box<File>),
    Source(Source, bool, Option<SynchronizationIssue>),
}
struct Orchestrator {
    config: OrchestratorConfig,
    operator: Box<dyn Operator>,
    status: watch::Sender<SynchronizationStatus>,
    stop: watch::Receiver<bool>,
    blockers: BTreeMap<SynchronizationItem, (SynchronizationItem, SynchronizationIssue)>,
    sources: BTreeMap<Source, Option<SynchronizationIssue>>,
    recovery_failed: bool,
}
impl Orchestrator {
    async fn run(&mut self, mut files: mpsc::Receiver<Command>, ownership: Arc<RwLock<()>>) {
        {
            let _lease = ownership.clone().read_owned().await;
            match self.operator.recover().await {
                Ok(results) => {
                    let changed = !results.is_empty();
                    for result in results {
                        if result.assessment.outcome == operations::models::ExecutionEffect::Unknown
                            || !result
                                .inventory_write
                                .as_ref()
                                .is_some_and(|receipt| receipt.status == WriteStatus::Written)
                        {
                            self.block(
                                result.item,
                                issue(SynchronizationIssueKind::RecoveryRequired, false),
                            );
                        }
                    }
                    if changed {
                        self.settle();
                    }
                }
                Err(_) => {
                    self.recovery_failed = true;
                    self.settle();
                }
            }
        }
        loop {
            let file = tokio::select! {biased;
                _=self.stop.changed()=>break,
                file=files.recv()=>match file {Some(file)=>file,None=>break},
            };
            if *self.stop.borrow() {
                break;
            }
            // A complete plan holds the connection owner's lease against credential removal.
            let _lease = tokio::select! {biased;
                _=self.stop.changed()=>break,
                lease=ownership.clone().read_owned()=>lease,
            };
            if *self.stop.borrow() {
                break;
            }
            match file {
                Command::File(file) => {
                    self.process(*file).await;
                }
                Command::Source(source, scanning, issue) => {
                    self.sources.insert(source, issue);
                    if scanning
                        && !self.recovery_failed
                        && !self
                            .sources
                            .values()
                            .flatten()
                            .any(|issue| is_failure(issue.kind))
                    {
                        self.publish(
                            SynchronizationState::Running,
                            SynchronizationActivity::Scanning,
                            None,
                            None,
                            None,
                        );
                    } else {
                        self.settle();
                    }
                }
            }
        }
        files.close();
        self.publish(
            SynchronizationState::Stopped,
            SynchronizationActivity::None,
            None,
            None,
            None,
        );
    }
    fn publish(
        &self,
        state: SynchronizationState,
        activity: SynchronizationActivity,
        item: Option<SynchronizationItem>,
        progress: Option<SynchronizationProgress>,
        issue: Option<SynchronizationIssue>,
    ) {
        self.status.send_replace(SynchronizationStatus {
            account_id: self.config.roots.account_id.clone(),
            state,
            activity,
            current_item: item,
            progress,
            issue,
            updated_at: timestamp(),
        });
    }
    fn block(&mut self, item: SynchronizationItem, issue: SynchronizationIssue) {
        self.blockers
            .retain(|_, (blocked, _)| !same_item(blocked, &item));
        self.blockers.insert(key(&item), (item, issue));
    }
    fn settle(&self) {
        if self.recovery_failed {
            self.publish(
                SynchronizationState::Waiting,
                SynchronizationActivity::None,
                None,
                None,
                Some(issue(SynchronizationIssueKind::RecoveryRequired, false)),
            );
            return;
        }
        if let Some(issue) = self
            .sources
            .values()
            .flatten()
            .find(|issue| is_failure(issue.kind))
        {
            self.publish(
                SynchronizationState::Failed,
                SynchronizationActivity::Observing,
                None,
                None,
                Some(issue.clone()),
            );
            return;
        }

        if let Some((_, (item, issue))) = self
            .blockers
            .iter()
            .find(|(_, (_, issue))| is_failure(issue.kind))
            .or_else(|| self.blockers.first_key_value())
        {
            self.publish(
                if is_failure(issue.kind) {
                    SynchronizationState::Failed
                } else {
                    SynchronizationState::Waiting
                },
                SynchronizationActivity::None,
                Some(item.clone()),
                None,
                Some(issue.clone()),
            );
        } else if let Some(issue) = self.sources.values().flatten().next() {
            self.publish(
                if is_failure(issue.kind) {
                    SynchronizationState::Failed
                } else {
                    SynchronizationState::Waiting
                },
                SynchronizationActivity::Observing,
                None,
                None,
                Some(issue.clone()),
            );
        } else if !self.sources.is_empty() && self.sources.len() < 2 {
            self.publish(
                SynchronizationState::Waiting,
                SynchronizationActivity::Observing,
                None,
                None,
                None,
            );
        } else {
            self.publish(
                SynchronizationState::Idle,
                SynchronizationActivity::None,
                None,
                None,
                None,
            );
        }
    }
    async fn process(&mut self, mut file: File) -> ProcessingOutcome {
        let item = SynchronizationItem {
            item_id: file.identity.item_id.clone(),
            local_path: file.local.path.clone(),
            remote_id: file.identity.remote_id.clone(),
        };
        let mut outcome = ProcessingOutcome {
            decision: DecisionStatus::InvalidInput,
            results: Vec::new(),
        };
        if file.identity.account_id != self.config.roots.account_id
            || file.identity.sync_root_id != self.config.sync_root_id
            || file.environment.local_root.0 != self.config.roots.local_root
            || file.environment.remote_root_id != self.config.roots.remote_root_id
        {
            self.block(item, issue(SynchronizationIssueKind::InvalidInput, false));
            self.settle();
            return outcome;
        }
        self.publish(
            SynchronizationState::Running,
            SynchronizationActivity::Analyzing,
            Some(item.clone()),
            None,
            None,
        );
        if self.recovery_failed {
            outcome.decision = DecisionStatus::Defer;
            self.settle();
            return outcome;
        }
        file.initial_association_authorized &= self.config.initial_association_authorized;
        let decision = analyzer::analyze(&file);
        outcome.decision = decision.status;
        self.blockers
            .retain(|_, (blocked, _)| !same_item(blocked, &item));
        if decision.status != DecisionStatus::Ready
            && !(decision.status == DecisionStatus::Adopt
                && decision.execution.status == ExecutionStatus::Executable
                && !decision.operations.is_empty())
        {
            let blocker = match decision.status {
                DecisionStatus::NoOp => None,
                DecisionStatus::Adopt => Some(SynchronizationIssueKind::AdoptionRequired),
                DecisionStatus::Conflict => Some(SynchronizationIssueKind::Conflict),
                DecisionStatus::InvalidInput => Some(SynchronizationIssueKind::InvalidInput),
                DecisionStatus::Unsupported => Some(SynchronizationIssueKind::Unsupported),
                _ => Some(
                    if decision
                        .validation
                        .checks
                        .iter()
                        .any(|check| check.code == "operation.pending")
                    {
                        SynchronizationIssueKind::RecoveryRequired
                    } else {
                        SynchronizationIssueKind::Deferred
                    },
                ),
            };
            if let Some(blocker) = blocker {
                self.block(item, issue(blocker, false));
            }
            self.settle();
            return outcome;
        }
        if decision.execution.status != ExecutionStatus::Executable
            || decision.operations.is_empty()
        {
            self.block(item, issue(SynchronizationIssueKind::InvalidInput, false));
            self.settle();
            return outcome;
        }
        // Partial or unavailable producers suspend dispatch, without changing Analyzer decisions.
        if !self.sources.is_empty()
            && (self.sources.len() < 2 || self.sources.values().any(Option::is_some))
        {
            self.block(item, issue(SynchronizationIssueKind::Deferred, true));
            self.settle();
            return outcome;
        }
        let total = decision.operations.len() as u64;
        let mut completed = Vec::new();
        for mut operation in decision.operations {
            if *self.stop.borrow() {
                break;
            }
            if operation
                .depends_on
                .iter()
                .any(|step| !completed.contains(step))
            {
                self.block(
                    item.clone(),
                    issue(SynchronizationIssueKind::InvalidInput, false),
                );
                break;
            }
            let execution_id = match execution_id() {
                Ok(id) => id,
                Err(()) => {
                    self.block(
                        item.clone(),
                        issue(SynchronizationIssueKind::WorkerFailed, false),
                    );
                    break;
                }
            };
            if operation.action == OperationAction::Adopt {
                let selected: Vec<_> = file
                    .association_candidates
                    .iter()
                    .filter(|candidate| {
                        candidate.local.path == operation.local_path
                            && candidate.remote.id == operation.remote_id
                    })
                    .collect();
                if selected.len() == 1 {
                    file.local = selected[0].local.clone();
                    file.remote = selected[0].remote.clone();
                    file.identity.remote_id = file.remote.id.clone();
                    file.identity.local_identity = file.local.identity.clone();
                }
            }
            if file.identity.item_id.is_none()
                && matches!(
                    operation.action,
                    OperationAction::RegisterIdentity | OperationAction::Adopt
                )
                && file.registration_id.as_ref() == Some(&operation.item_ref)
            {
                file.identity.item_id = Some(operation.item_ref.clone());
                file.inventory_revision = Some(0);
            }
            // Refresh command preconditions only from verified receipts of earlier steps.
            operation.local_path = file.local.path.clone();
            operation.remote_id = file.identity.remote_id.clone();
            if let Some(preconditions) = &mut operation.preconditions {
                preconditions.observed_remote_version = match file.remote.version {
                    Evidence::Known(version) => Some(version),
                    _ => None,
                };
            }
            self.publish(
                SynchronizationState::Running,
                activity(operation.action),
                Some(item.clone()),
                Some(SynchronizationProgress {
                    completed: completed.len() as u64,
                    total,
                }),
                None,
            );
            let step = operation.step;
            let expected_id = execution_id.clone();
            let expected_item = operation.item_ref.clone();
            let expected_action = operation.action;
            let result = self
                .operator
                .execute(ExecuteOperationRequest {
                    operation,
                    execution_id,
                    expected_revision: file.inventory_revision,
                    identity: file.identity.clone(),
                    local: file.local.clone(),
                    remote: file.remote.clone(),
                    completed_steps: completed.clone(),
                })
                .await;
            let confirmed = result.operation_id == expected_id
                && result.item_id == expected_item
                && result.action == expected_action
                && result.status == OperationResultStatus::Succeeded
                && result.effect.as_ref().is_some_and(|effect| effect.verified)
                && result.inventory_write.as_ref().is_some_and(|write| {
                    write.status == WriteStatus::Written
                        && write
                            .revision
                            .is_some_and(|revision| revision > file.inventory_revision.unwrap_or(0))
                });
            if confirmed {
                if let Some(effect) = &result.effect {
                    if let Some(local) = &effect.local {
                        file.local = local.clone();
                        file.identity.local_identity = local.identity.clone();
                    }
                    if let Some(remote) = &effect.remote {
                        file.remote = remote.clone();
                    }
                    if effect.remote_id.is_some() {
                        file.identity.remote_id = effect.remote_id.clone();
                    }
                }
                file.inventory_revision = result
                    .inventory_write
                    .as_ref()
                    .and_then(|write| write.revision);
                completed.push(step);
            } else {
                let kind = match result.status {
                    OperationResultStatus::Unconfirmed => {
                        SynchronizationIssueKind::RecoveryRequired
                    }
                    OperationResultStatus::Skipped => SynchronizationIssueKind::PreconditionFailed,
                    OperationResultStatus::Succeeded => SynchronizationIssueKind::RecoveryRequired,
                    OperationResultStatus::Failed => SynchronizationIssueKind::OperationFailed,
                };
                self.block(
                    item.clone(),
                    issue(
                        kind,
                        kind != SynchronizationIssueKind::RecoveryRequired
                            && result.error.as_ref().is_some_and(|error| error.retryable),
                    ),
                );
            }
            outcome.results.push(result);
            self.publish(
                SynchronizationState::Running,
                SynchronizationActivity::Applying,
                Some(item.clone()),
                Some(SynchronizationProgress {
                    completed: completed.len() as u64,
                    total,
                }),
                None,
            );
            if !confirmed {
                break;
            }
        }
        if !*self.stop.borrow() {
            self.settle();
        }
        outcome
    }
}
fn activity(action: OperationAction) -> SynchronizationActivity {
    match action {
        OperationAction::Upload | OperationAction::UpdateRemoteContent => {
            SynchronizationActivity::Uploading
        }
        OperationAction::Download | OperationAction::UpdateLocalContent => {
            SynchronizationActivity::Downloading
        }
        _ => SynchronizationActivity::Applying,
    }
}
fn issue(kind: SynchronizationIssueKind, retryable: bool) -> SynchronizationIssue {
    SynchronizationIssue { kind, retryable }
}
fn is_failure(kind: SynchronizationIssueKind) -> bool {
    matches!(
        kind,
        SynchronizationIssueKind::InvalidInput
            | SynchronizationIssueKind::OperationFailed
            | SynchronizationIssueKind::WorkerFailed
    )
}
pub(crate) fn timestamp() -> DateTime {
    DateTime {
        unix_millis: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(i64::MAX as u128) as i64,
    }
}
fn execution_id() -> Result<Id, ()> {
    let mut bytes = [0; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|_| ())?;
    Ok(Id(bytes.iter().map(|byte| format!("{byte:02x}")).collect()))
}
#[cfg(test)]
mod tests;

fn key(item: &SynchronizationItem) -> SynchronizationItem {
    if item.item_id.is_some() {
        SynchronizationItem {
            item_id: item.item_id.clone(),
            local_path: None,
            remote_id: None,
        }
    } else if item.remote_id.is_some() {
        SynchronizationItem {
            item_id: None,
            local_path: None,
            remote_id: item.remote_id.clone(),
        }
    } else {
        item.clone()
    }
}

fn same_item(a: &SynchronizationItem, b: &SynchronizationItem) -> bool {
    if let (Some(a), Some(b)) = (&a.item_id, &b.item_id) {
        return a == b;
    }
    if let (Some(a), Some(b)) = (&a.remote_id, &b.remote_id) {
        return a == b;
    }
    matches!((&a.local_path,&b.local_path),(Some(a),Some(b)) if a==b)
}
