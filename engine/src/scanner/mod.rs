//! Local inventory observation, isolated from detectors, synchronization decisions and transfers.
//! Each scan uses one finite blocking worker; status never waits for filesystem or SQLite work.
mod filesystem;
mod models;
mod traversal;
use crate::{inventory::*, models::*};
pub use models::*;
use std::{
    os::unix::ffi::OsStrExt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

/// Reusable scope-bound scanner. No worker exists until scan() is polled.
pub struct Scanner {
    inner: Arc<Inner>,
}
struct Inner {
    inventory: Arc<InventoryManager>,
    root: LocalRoot,
    order_source: Arc<dyn CausalOrderSource>,
    filesystem: filesystem::Root,
    native: filesystem::NativeQuery,
    status: Mutex<ScannerStatus>,
    running: AtomicBool,
    #[cfg(test)]
    hook: Option<ScanHook>,
}
#[cfg(test)]
type ScanHook = Arc<dyn Fn(Stage, &[u8]) + Send + Sync>;
#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Observed,
    DirectoryRead,
    BeforeRecord,
}
fn now() -> Timestamp {
    let unix_millis = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis().min(i64::MAX as u128) as i64,
        Err(error) => -(error.duration().as_millis().min(i64::MAX as u128) as i64),
    };
    Timestamp { unix_millis }
}
fn failure(kind: ScannerErrorKind) -> ScannerError {
    ScannerError::new(kind)
}
fn inventory_error(error: InventoryFailure) -> ScannerError {
    failure(match error.kind {
        InventoryError::InvalidIdentity => ScannerErrorKind::WrongSyncScope,
        InventoryError::CorruptDatabase | InventoryError::UnsupportedSchema => {
            ScannerErrorKind::InventoryUnusable
        }
        _ => ScannerErrorKind::InventoryUnavailable,
    })
}
impl Inner {
    fn status_update(&self, update: impl FnOnce(&mut ScannerStatus)) {
        update(&mut self.status.lock().unwrap_or_else(|e| e.into_inner()));
    }
    fn validate(&self) -> ScannerResult<()> {
        let info = self
            .inventory
            .get_sync_info(GetSyncInfoRequest {})
            .map_err(inventory_error)?;
        if info.info.sync_id != self.root.sync_id
            || info.local_root != self.root.path.as_os_str().as_bytes()
        {
            return Err(failure(ScannerErrorKind::WrongSyncScope));
        }
        self.filesystem
            .validate()
            .map_err(|_| failure(ScannerErrorKind::RootInaccessible))
    }
}
struct CancelOnDrop {
    flag: Arc<AtomicBool>,
}
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.flag.store(true, Ordering::Release);
    }
}
impl Scanner {
    /// Validates Inventory/root identity and pins the root without enumerating or starting capture.
    pub fn new(config: ScannerConfig) -> ScannerResult<Self> {
        if config.root.sync_id.0.trim().is_empty()
            || !config.root.path.is_absolute()
            || config.root.path.components().any(|c| {
                matches!(c, std::path::Component::ParentDir) || c.as_os_str() == ".drivesync"
            })
        {
            return Err(failure(ScannerErrorKind::InvalidConfiguration));
        }
        let info = config
            .inventory
            .get_sync_info(GetSyncInfoRequest {})
            .map_err(inventory_error)?;
        if info.info.sync_id != config.root.sync_id
            || info.local_root != config.root.path.as_os_str().as_bytes()
        {
            return Err(failure(ScannerErrorKind::WrongSyncScope));
        }
        let filesystem = filesystem::Root::new(&config.root.path).map_err(|e| {
            failure(match e.kind() {
                std::io::ErrorKind::NotFound => ScannerErrorKind::RootNotFound,
                std::io::ErrorKind::InvalidInput => ScannerErrorKind::InvalidConfiguration,
                _ => ScannerErrorKind::RootInaccessible,
            })
        })?;
        Ok(Self {
            inner: Arc::new(Inner {
                inventory: config.inventory,
                root: config.root,
                order_source: config.order_source,
                filesystem,
                native: crate::filesystem::metadata::statx,
                status: Mutex::new(ScannerStatus::default()),
                running: AtomicBool::new(false),
                #[cfg(test)]
                hook: None,
            }),
        })
    }
    /// Runs one local scan without requiring the caller to own a Tokio runtime.
    /// Dropping this future requests cooperative cancellation; earlier Inventory commits are preserved.
    /// Only a complete report permits subsequent analysis; no consumer is resumed here.
    pub async fn scan(&self, request: ScanRequest) -> ScannerResult<ScanReport> {
        if request.sync_id != self.inner.root.sync_id {
            return Err(failure(ScannerErrorKind::WrongSyncScope));
        }
        if self
            .inner
            .running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(failure(ScannerErrorKind::AlreadyRunning));
        }
        self.inner.status_update(|s| {
            *s = ScannerStatus {
                state: ScannerState::Running,
                started_at: Some(now()),
                ..ScannerStatus::default()
            }
        });
        let cancel = Arc::new(AtomicBool::new(false));
        let guard = CancelOnDrop {
            flag: cancel.clone(),
        };
        let inner = self.inner.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        let worker = std::thread::Builder::new()
            .name("local-inventory-scan".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    traversal::run(&inner, &cancel)
                }))
                .unwrap_or_else(|_| Err(failure(ScannerErrorKind::Internal)));
                inner.status_update(|s| {
                    s.finished_at = Some(now());
                    s.current_directory = None;
                    match &result {
                        Ok(report) => {
                            s.state = ScannerState::Completed;
                            s.last_report_status = Some(report.status);
                        }
                        Err(e) => {
                            s.state = if e.kind == ScannerErrorKind::Cancelled {
                                ScannerState::Cancelled
                            } else {
                                ScannerState::Failed
                            };
                            s.last_error = Some(e.kind);
                        }
                    }
                });
                inner.running.store(false, Ordering::Release);
                let _ = tx.send(result);
            });
        if worker.is_err() {
            self.inner.status_update(|s| {
                s.state = ScannerState::Failed;
                s.finished_at = Some(now());
                s.last_error = Some(ScannerErrorKind::Internal);
            });
            self.inner.running.store(false, Ordering::Release);
            return Err(failure(ScannerErrorKind::Internal));
        }
        let result = rx
            .await
            .unwrap_or_else(|_| Err(failure(ScannerErrorKind::Internal)));
        drop(guard);
        result
    }
    /// Returns the coherent current/last snapshot without waiting on enumeration or an Inventory transaction.
    pub fn status(&self) -> ScannerStatus {
        self.inner
            .status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}
#[cfg(test)]
mod tests;
