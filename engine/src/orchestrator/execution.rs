//! Cached execution diagnostics and per-pass cancellation; Analyzer owns dispatch.
use crate::{
    analyzer::*,
    inventory::SyncItem,
    models::MetadataValue,
    operations::*,
    synchronization::{
        SynchronizationAction, SynchronizationActivity, SynchronizationIssue,
        SynchronizationIssueKind, SynchronizationItem, SynchronizationLocation,
        SynchronizationProgress, SynchronizationProgressUnit, SynchronizationState,
        SynchronizationWork, SynchronizationWorkDetail,
    },
};
use std::os::unix::ffi::OsStringExt;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

#[derive(Clone, Debug, Default)]
pub struct ExecutionStatus {
    pub enabled: bool,
    pub completed: u64,
    pub failed: u64,
    pub progress: Option<ExecutionProgress>,
    pub recent: Vec<CreationEvent>,
    /// Read-only prerequisite failures retried after reconnect/backoff.
    pub prerequisite_retries:
        std::collections::BTreeMap<crate::inventory::SyncItemId, crate::models::Timestamp>,
}
pub(super) struct ExecutionControl(
    Mutex<ExecutionStatus>,
    Mutex<Option<Arc<crate::synchronization::Publisher>>>,
    Mutex<Option<SynchronizationWork>>,
);
impl ExecutionControl {
    pub fn new(enabled: bool) -> Arc<Self> {
        Arc::new(Self(
            Mutex::new(ExecutionStatus {
                enabled,
                ..Default::default()
            }),
            Mutex::new(None),
            Mutex::new(None),
        ))
    }
    pub fn set_publisher(&self, publisher: Arc<crate::synchronization::Publisher>) {
        *self.1.lock().unwrap_or_else(|e| e.into_inner()) = Some(publisher);
    }
    /// Consume expired wakeups; a failed probe schedules its own next check.
    /// Items retired or confirmed in the meantime must not cause a hot loop.
    pub fn probe_retry_due(&self) -> bool {
        let now = super::analysis::now().unix_millis;
        let mut snapshot = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let count = snapshot.prerequisite_retries.len();
        snapshot
            .prerequisite_retries
            .retain(|_, at| at.unix_millis > now);
        snapshot.prerequisite_retries.len() != count
    }
    pub fn snapshot(&self) -> ExecutionStatus {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    pub fn publish_analysis(
        &self,
        item: &SyncItem,
        phase: AnalysisPhase,
        processed: u64,
        total: Option<u64>,
    ) {
        let work = SynchronizationWork {
            action: SynchronizationAction::Analyze,
            item: Some(item_summary(item)),
            location: item
                .local_latest
                .as_ref()
                .or(item.local.as_ref())
                .map(|local| SynchronizationLocation::Local(local.relative_path.clone()))
                .or_else(|| {
                    item.remote_latest
                        .as_ref()
                        .or(item.remote.as_ref())
                        .map(|remote| SynchronizationLocation::Remote {
                            reference: remote.reference.clone(),
                            parent_id: match &remote.parent_ids {
                                MetadataValue::Known(ids) if ids.len() == 1 => Some(ids[0].clone()),
                                _ => None,
                            },
                            name: match &remote.name {
                                MetadataValue::Known(name) => Some(name.clone()),
                                _ => None,
                            },
                        })
                }),
            detail: SynchronizationWorkDetail::Analysis(phase),
            event: None,
        };
        *self.2.lock().unwrap_or_else(|e| e.into_inner()) = Some(work.clone());
        if let Some(publisher) = self.1.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            publisher.update(|status| {
                status.state = SynchronizationState::Running;
                status.activity = SynchronizationActivity::Analyzing;
                status.work = Some(work);
                status.progress = Some(SynchronizationProgress {
                    completed: processed,
                    total,
                    unit: SynchronizationProgressUnit::Items,
                    phase: None,
                });
                status.counters.analysis_items_processed = processed;
                status.counters.analysis_items_total = total;
            });
        }
    }
    pub fn update_analysis_progress(&self, processed: u64, total: Option<u64>) {
        if let Some(publisher) = self.1.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            publisher.update(|status| {
                status.counters.analysis_items_processed = processed;
                status.counters.analysis_items_total = total;
                if status.activity == SynchronizationActivity::Analyzing {
                    status.progress = Some(SynchronizationProgress {
                        completed: processed,
                        total,
                        unit: SynchronizationProgressUnit::Items,
                        phase: None,
                    });
                }
            });
        }
    }
}
impl CreationObserver for ExecutionControl {
    fn notify(&self, event: CreationEvent) {
        let previous = self.2.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let action = event
            .command
            .as_ref()
            .map(command_action)
            .unwrap_or(SynchronizationAction::Analyze);
        let mut item = previous.as_ref().and_then(|work| work.item.clone());
        if let (Some(summary), Some(command)) = (&mut item, event.command.as_ref()) {
            apply_command_name(summary, command);
        }
        let work = SynchronizationWork {
            action,
            item,
            location: event
                .command
                .as_ref()
                .and_then(command_location)
                .or_else(|| previous.as_ref().and_then(|work| work.location.clone())),
            detail: SynchronizationWorkDetail::Creation(event.stage),
            event: previous.as_ref().and_then(|work| work.event.clone()),
        };
        *self.2.lock().unwrap_or_else(|e| e.into_inner()) = Some(work.clone());
        if let Some(publisher) = self.1.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            publisher.update(|status| {
                status.state = SynchronizationState::Running;
                status.activity = match event.stage {
                    CreationStage::Checking | CreationStage::Blocked => {
                        SynchronizationActivity::Checking
                    }
                    CreationStage::Verifying => SynchronizationActivity::Verifying,
                    CreationStage::Executing => activity_for(action),
                    CreationStage::Completed | CreationStage::Failed => {
                        SynchronizationActivity::Analyzing
                    }
                };
                status.work = Some(work);
                if event.stage != CreationStage::Verifying {
                    status.progress = None;
                }
                if matches!(event.stage, CreationStage::Blocked | CreationStage::Failed) {
                    let kind = event.error.unwrap_or(OperatorError::Internal);
                    let retryable = matches!(
                        kind,
                        OperatorError::Network
                            | OperatorError::Io
                            | OperatorError::CheckpointUnavailable
                    );
                    status.issue = Some(SynchronizationIssue {
                        kind: SynchronizationIssueKind::Operation(kind),
                        retryable,
                        item: status.work.as_ref().and_then(|work| work.item.clone()),
                        count: 1,
                    });
                } else if event.stage == CreationStage::Completed
                    && status.issue.as_ref().is_some_and(|issue| {
                        issue.item.as_ref().and_then(|item| item.id.as_ref())
                            == Some(&event.item_id)
                    })
                {
                    status.issue = None;
                }
            });
        }
        let mut s = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if event.stage == CreationStage::Checking || event.stage == CreationStage::Executing {
            s.progress = None;
            s.prerequisite_retries.remove(&event.item_id);
        }
        if event.stage == CreationStage::Blocked
            && matches!(
                event.error,
                Some(OperatorError::Network | OperatorError::PermissionDenied)
            )
            && event.operation_id.is_none()
        {
            s.prerequisite_retries.insert(
                event.item_id.clone(),
                crate::models::Timestamp {
                    unix_millis: super::analysis::now().unix_millis.saturating_add(30_000),
                },
            );
        }
        if event.stage == CreationStage::Completed {
            s.completed = s.completed.saturating_add(1);
        }
        if event.stage == CreationStage::Failed {
            s.failed = s.failed.saturating_add(1);
        }
        if s.recent.len() == 64 {
            s.recent.remove(0);
        }
        s.recent.push(event);
    }
}
impl ProgressReporter for ExecutionControl {
    fn try_report(&self, p: ExecutionProgress) -> ProgressDelivery {
        if let Some(publisher) = self.1.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            let work = self.2.lock().unwrap_or_else(|e| e.into_inner()).clone();
            publisher.update(|status| {
                status.state = SynchronizationState::Running;
                status.activity = match p.phase {
                    ExecutionPhase::Preparing | ExecutionPhase::Finalizing => {
                        SynchronizationActivity::Checking
                    }
                    ExecutionPhase::Transferring => work
                        .as_ref()
                        .map(|work| activity_for(work.action))
                        .unwrap_or(SynchronizationActivity::Applying),
                    ExecutionPhase::Applying => SynchronizationActivity::Applying,
                    ExecutionPhase::Verifying => SynchronizationActivity::Verifying,
                };
                status.work = work.clone().map(|mut work| {
                    work.detail = SynchronizationWorkDetail::Transfer(p.phase);
                    work
                });
                status.progress = p
                    .bytes_transferred
                    .map(|completed| SynchronizationProgress {
                        completed,
                        total: p.total_bytes,
                        unit: SynchronizationProgressUnit::Bytes,
                        phase: Some(p.phase),
                    });
            });
        }
        self.0.lock().unwrap_or_else(|e| e.into_inner()).progress = Some(p);
        ProgressDelivery::Accepted
    }
}

fn activity_for(action: SynchronizationAction) -> SynchronizationActivity {
    match action {
        SynchronizationAction::Upload => SynchronizationActivity::Uploading,
        SynchronizationAction::Download => SynchronizationActivity::Downloading,
        SynchronizationAction::Verify => SynchronizationActivity::Verifying,
        _ => SynchronizationActivity::Applying,
    }
}

fn item_summary(item: &SyncItem) -> SynchronizationItem {
    let local = item.local_latest.as_ref().or(item.local.as_ref());
    let remote = item.remote_latest.as_ref().or(item.remote.as_ref());
    let mut summary = SynchronizationItem {
        id: Some(item.id.clone()),
        kind: Some(item.kind.clone()),
        local_name: None,
        remote_name: None,
        local_path: local.map(|file| file.relative_path.clone()),
        remote_reference: remote
            .map(|file| file.reference.clone())
            .or(item.remote_ref.clone()),
    };
    if let Some(local) = local {
        if let Some(name) = local
            .relative_path
            .as_bytes()
            .rsplit(|byte| *byte == b'/')
            .next()
        {
            summary.local_name = Some(std::ffi::OsString::from_vec(name.to_vec()));
        }
    }
    if let Some(remote) = remote {
        if let MetadataValue::Known(name) = &remote.name {
            summary.remote_name = Some(name.clone());
        }
    }
    summary
}

fn command_action(command: &OperationCommand) -> SynchronizationAction {
    match command {
        OperationCommand::CreateFile {
            parent,
            content: Some(_),
            ..
        } => match parent {
            DirectoryRef::LocalRoot | DirectoryRef::Local { .. } => SynchronizationAction::Download,
            DirectoryRef::Remote { .. } => SynchronizationAction::Upload,
        },
        OperationCommand::CreateFile { .. } => SynchronizationAction::CreateFile,
        OperationCommand::CreateDirectory { .. } => SynchronizationAction::CreateDirectory,
        OperationCommand::Upload { .. } => SynchronizationAction::Upload,
        OperationCommand::Download { .. } => SynchronizationAction::Download,
        OperationCommand::UpdateContent { target, .. } => match target {
            ResourceRef::Local { .. } => SynchronizationAction::Download,
            ResourceRef::Remote { .. } => SynchronizationAction::Upload,
        },
        OperationCommand::Rename { .. } => SynchronizationAction::Rename,
        OperationCommand::Move { .. } => SynchronizationAction::Move,
        OperationCommand::Trash { .. } => SynchronizationAction::Trash,
        OperationCommand::Restore { .. } => SynchronizationAction::Restore,
    }
}

fn command_location(command: &OperationCommand) -> Option<SynchronizationLocation> {
    fn directory(value: &DirectoryRef) -> SynchronizationLocation {
        match value {
            DirectoryRef::LocalRoot => SynchronizationLocation::LocalRoot,
            DirectoryRef::Local { relative_path, .. } => {
                SynchronizationLocation::Local(relative_path.clone())
            }
            DirectoryRef::Remote { reference, .. } => SynchronizationLocation::Remote {
                reference: reference.clone(),
                parent_id: None,
                name: None,
            },
        }
    }
    fn resource(value: &ResourceRef) -> SynchronizationLocation {
        match value {
            ResourceRef::Local { relative_path, .. } => {
                SynchronizationLocation::Local(relative_path.clone())
            }
            ResourceRef::Remote { reference, .. } => SynchronizationLocation::Remote {
                reference: reference.clone(),
                parent_id: None,
                name: None,
            },
        }
    }
    match command {
        OperationCommand::CreateFile { parent, .. }
        | OperationCommand::CreateDirectory { parent, .. } => Some(directory(parent)),
        OperationCommand::Upload { destination, .. }
        | OperationCommand::Download { destination, .. } => Some(directory(destination)),
        OperationCommand::UpdateContent { target, .. }
        | OperationCommand::Rename { target, .. }
        | OperationCommand::Trash { target, .. } => Some(resource(target)),
        OperationCommand::Move { destination, .. } => Some(directory(destination)),
        OperationCommand::Restore { destination, .. } => destination.as_ref().map(directory),
    }
}

fn apply_command_name(item: &mut SynchronizationItem, command: &OperationCommand) {
    let name = match command {
        OperationCommand::CreateFile { name, .. }
        | OperationCommand::CreateDirectory { name, .. }
        | OperationCommand::Upload { name, .. }
        | OperationCommand::Download { name, .. } => Some(name),
        OperationCommand::Rename { new_name, .. } => Some(new_name),
        _ => None,
    };
    match name {
        Some(ItemName::Local(name)) => item.local_name = Some(name.clone()),
        Some(ItemName::Remote(name)) => item.remote_name = Some(name.clone()),
        None => {}
    }
}
struct Cancellation {
    epoch: Arc<AtomicU64>,
    expected: u64,
}
impl CancellationSignal for Cancellation {
    fn is_cancelled(&self) -> bool {
        self.epoch.load(Ordering::Acquire) != self.expected
    }
}
pub(super) fn configure(
    control: Arc<ExecutionControl>,
    inventory: Arc<crate::inventory::InventoryManager>,
    connector: crate::Engine,
    epoch: Arc<AtomicU64>,
    expected: u64,
) -> Result<Option<AnalyzerExecution>, super::PreparationFailure> {
    if !control.snapshot().enabled {
        return Ok(None);
    }
    let info = inventory
        .get_sync_info(crate::inventory::GetSyncInfoRequest {})
        .map_err(super::PreparationFailure::Inventory)?;
    let operator = DefaultOperatorFactory
        .create(OperatorConfig {
            connector,
            scope: OperatorScope {
                sync_id: info.info.sync_id,
                account_id: info.info.account_id,
                remote_root_id: crate::models::RemoteFileId(info.info.remote_root_id),
                local_root: std::path::PathBuf::from(
                    <std::ffi::OsString as std::os::unix::ffi::OsStringExt>::from_vec(
                        info.local_root,
                    ),
                ),
            },
            journal: inventory,
        })
        .map_err(|_| super::PreparationFailure::Internal)?;
    Ok(Some(AnalyzerExecution {
        operator,
        options: ExecutionOptions {
            cancellation: Some(Arc::new(Cancellation { epoch, expected })),
            progress: Some(control.clone()),
        },
        observer: control,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expired_prerequisite_retry_is_consumed_once() {
        let control = ExecutionControl::new(true);
        control.0.lock().unwrap().prerequisite_retries.insert(
            crate::inventory::SyncItemId("retired-item".into()),
            crate::models::Timestamp { unix_millis: 0 },
        );
        assert!(control.probe_retry_due());
        assert!(!control.probe_retry_due());
    }
}
