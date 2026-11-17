//! Cached execution diagnostics and per-pass cancellation; Analyzer owns dispatch.
use crate::{analyzer::*, operations::*};
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
);
impl ExecutionControl {
    pub fn new(enabled: bool) -> Arc<Self> {
        Arc::new(Self(
            Mutex::new(ExecutionStatus {
                enabled,
                ..Default::default()
            }),
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
}
impl CreationObserver for ExecutionControl {
    fn notify(&self, event: CreationEvent) {
        if let Some(publisher) = self.1.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            use crate::SynchronizationActivity as A;
            publisher.activity(match event.stage {
                CreationStage::Executing => Some(match &event.command {
                    Some(OperationCommand::Download { .. }) => A::Downloading,
                    Some(OperationCommand::CreateFile {
                        parent,
                        content: Some(_),
                        ..
                    }) => match parent {
                        DirectoryRef::LocalRoot | DirectoryRef::Local { .. } => A::Downloading,
                        DirectoryRef::Remote { .. } => A::Uploading,
                    },
                    Some(OperationCommand::Upload { .. }) => A::Uploading,
                    Some(OperationCommand::UpdateContent { target, .. }) => match target {
                        ResourceRef::Local { .. } => A::Downloading,
                        ResourceRef::Remote { .. } => A::Uploading,
                    },
                    _ => A::Reconciling,
                }),
                CreationStage::Checking | CreationStage::Verifying => Some(A::Reconciling),
                _ => None,
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
            publisher.progress(p.bytes_transferred, p.total_bytes);
        }
        self.0.lock().unwrap_or_else(|e| e.into_inner()).progress = Some(p);
        ProgressDelivery::Accepted
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
