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
pub(super) struct ExecutionControl(Mutex<ExecutionStatus>);
impl ExecutionControl {
    pub fn new(enabled: bool) -> Arc<Self> {
        Arc::new(Self(Mutex::new(ExecutionStatus {
            enabled,
            ..Default::default()
        })))
    }
    pub fn probe_retry_due(&self) -> bool {
        let now = super::analysis::now().unix_millis;
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .prerequisite_retries
            .values()
            .any(|at| at.unix_millis <= now)
    }
    pub fn snapshot(&self) -> ExecutionStatus {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}
impl CreationObserver for ExecutionControl {
    fn notify(&self, event: CreationEvent) {
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
