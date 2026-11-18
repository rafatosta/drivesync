//! Private SQLite ownership and transaction boundary for the inventory.
use super::analysis::*;
use super::models::*;
use crate::models::*;
use rusqlite::{Connection, OpenFlags, TransactionBehavior};
use std::{fs::File as FsFile, sync::Mutex};
mod filesystem;
pub mod inspection;

/// Persistent state access for one synchronization; owns no observer or transfer worker.
pub struct InventoryManager {
    connection: Mutex<Connection>,
    status: Mutex<InventoryStatusResult>,
    order_source: Mutex<Option<std::sync::Arc<dyn crate::models::CausalOrderSource>>>,
    _directory: FsFile,
}
pub(super) fn failure(kind: InventoryError) -> InventoryFailure {
    InventoryFailure::new(kind)
}
pub(super) fn sql(error: rusqlite::Error) -> InventoryFailure {
    use rusqlite::ErrorCode::*;
    failure(match error.sqlite_error_code() {
        Some(DatabaseCorrupt | NotADatabase) => InventoryError::CorruptDatabase,
        Some(DiskFull | TooBig) => InventoryError::StorageFull,
        Some(ConstraintViolation) => InventoryError::DuplicateAssociation,
        Some(DatabaseBusy | DatabaseLocked | CannotOpen | SystemIoFailure) => {
            InventoryError::StorageUnavailable
        }
        Some(PermissionDenied | ReadOnly) => InventoryError::PermissionDenied,
        _ => InventoryError::TransactionFailed,
    })
}
const SCHEMA: &str = include_str!("schema.sql");
mod activity;
mod analysis;
mod baseline;
mod creation;
mod execution;
mod items;
mod metadata;
mod operations;
mod pagination;
mod planning;
impl InventoryManager {
    /// Opens the private database, rejecting incompatible identity/schema without replacement.
    /// Creation is explicit and preserves user entries in an existing configured root.
    pub fn open(request: OpenInventoryRequest) -> InventoryResult<OpenInventoryResult> {
        use std::os::{fd::AsRawFd, unix::ffi::OsStrExt};
        if request.expected.sync_id.0.trim().is_empty()
            || request.expected.account_id.0.trim().is_empty()
            || request.expected.remote_root_id.trim().is_empty()
        {
            return Err(failure(InventoryError::InvalidIdentity));
        }
        let (directory, file, created) = filesystem::open(&request)?;
        let path = format!("/proc/self/fd/{}/inventory.sqlite3", directory.as_raw_fd());
        let mut db = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(sql)?;
        db.busy_timeout(std::time::Duration::from_millis(1000))
            .map_err(sql)?;
        db.pragma_update(None, "foreign_keys", "ON").map_err(sql)?;
        db.pragma_update(None, "trusted_schema", "OFF")
            .map_err(sql)?;
        db.pragma_update(None, "synchronous", "EXTRA")
            .map_err(sql)?;
        if !created {
            verify(&db, &request)?;
        }
        let mode: String = db
            .query_row("PRAGMA journal_mode=DELETE", [], |r| r.get(0))
            .map_err(sql)?;
        if mode != "delete" {
            return Err(failure(InventoryError::StorageUnavailable));
        }
        let check: String = db
            .query_row("PRAGMA quick_check", [], |r| r.get(0))
            .map_err(sql)?;
        if check != "ok" {
            return Err(failure(InventoryError::CorruptDatabase));
        }
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sql)?;
        if created {
            tx.execute_batch(SCHEMA).map_err(sql)?;
            tx.execute(
                "INSERT INTO sync_info VALUES(1,?1,?2,?3,?4,1,0)",
                rusqlite::params![
                    request.expected.sync_id.0,
                    request.expected.account_id.0,
                    request.expected.remote_root_id,
                    request.local_root.as_os_str().as_bytes()
                ],
            )
            .map_err(sql)?;
        }
        verify(&tx, &request)?;
        let info = sync_info(&tx)?;
        tx.commit().map_err(sql)?;
        if created {
            file.sync_all()
                .map_err(|_| failure(InventoryError::StorageUnavailable))?;
            directory
                .sync_all()
                .map_err(|_| failure(InventoryError::StorageUnavailable))?;
        }
        drop(file);
        let inventory = Self {
            connection: Mutex::new(db),
            status: Mutex::new(InventoryStatusResult {
                availability: InventoryAvailability::Ready,
                recovery: InventoryRecoveryOutcome::Normal,
                last_error: None,
            }),
            order_source: Mutex::new(None),
            _directory: directory,
        };
        Ok(OpenInventoryResult {
            inventory,
            info,
            created,
            recovery: InventoryRecoveryOutcome::Normal,
        })
    }
    /// Binds the synchronization-wide durable order source used by Scanner and Operator results.
    pub(crate) fn set_order_source(
        &self,
        source: std::sync::Arc<dyn crate::models::CausalOrderSource>,
    ) {
        *self
            .order_source
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(source);
    }
    pub(super) fn reserve_order(&self) -> InventoryResult<Option<u64>> {
        let source = self
            .order_source
            .lock()
            .map_err(|_| failure(InventoryError::StorageUnavailable))?
            .clone();
        source
            .map(|source| {
                source
                    .reserve_causal_order()
                    .map_err(|_| failure(InventoryError::StorageUnavailable))
            })
            .transpose()
    }
    /// Reads cached availability without waiting for SQL or a transaction.
    pub fn status(&self, _request: InventoryStatusRequest) -> InventoryStatusResult {
        self.status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub(super) fn run<T>(
        &self,
        write: bool,
        work: impl FnOnce(&Connection) -> InventoryResult<T>,
    ) -> InventoryResult<T> {
        if self.status(InventoryStatusRequest {}).availability == InventoryAvailability::Unavailable
        {
            return Err(failure(InventoryError::StorageUnavailable));
        }
        let result = (|| {
            let mut db = self
                .connection
                .lock()
                .map_err(|_| failure(InventoryError::StorageUnavailable))?;
            let tx = db
                .transaction_with_behavior(if write {
                    TransactionBehavior::Immediate
                } else {
                    TransactionBehavior::Deferred
                })
                .map_err(sql)?;
            let value = work(&tx)?;
            tx.commit().map_err(sql)?;
            Ok(value)
        })();
        let mut status = self.status.lock().unwrap_or_else(|e| e.into_inner());
        status.last_error = result
            .as_ref()
            .err()
            .map(|error: &InventoryFailure| error.kind.clone());
        if matches!(
            status.last_error,
            Some(InventoryError::CorruptDatabase | InventoryError::UnsupportedSchema)
        ) {
            status.availability = InventoryAvailability::Unavailable;
        }
        result
    }
    /// Reads persistent synchronization identity and materialization progress.
    pub fn get_sync_info(
        &self,
        _request: GetSyncInfoRequest,
    ) -> InventoryResult<GetSyncInfoResult> {
        self.run(false, |tx| {
            Ok(GetSyncInfoResult {
                info: sync_info(tx)?,
                local_root: tx
                    .query_row("SELECT local_root FROM sync_info WHERE id=1", [], |r| {
                        r.get(0)
                    })
                    .map_err(sql)?,
            })
        })
    }
    /// Marks materialization complete under generation and previous-state preconditions.
    pub fn mark_initial_download_complete(
        &self,
        request: MarkInitialDownloadCompleteRequest,
    ) -> InventoryResult<MarkInitialDownloadCompleteResult> {
        self.run(true, |tx| {
            let info = sync_info(tx)?;
            if info.generation != request.expected_generation
                || info.initial_download_complete != request.expected_complete
            {
                return Err(failure(InventoryError::VersionMismatch));
            }
            if request.expected_complete {
                return Err(failure(InventoryError::InvalidTransition));
            }
            let unfinished: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM operations WHERE direction='Download' AND state!='Completed') OR EXISTS(SELECT 1 FROM items WHERE status='PendingDownload')",
                [], |r| r.get(0),
            ).map_err(sql)?;
            if unfinished {
                return Err(failure(InventoryError::InvalidTransition));
            }
            tx.execute("UPDATE sync_info SET initial_complete=1 WHERE id=1", [])
                .map_err(sql)?;
            Ok(MarkInitialDownloadCompleteResult {
                info: sync_info(tx)?,
            })
        })
    }
}
fn sync_info(db: &Connection) -> InventoryResult<SyncInfo> {
    db.query_row(
        "SELECT sync,account,remote_root,generation,initial_complete FROM sync_info WHERE id=1",
        [],
        |r| {
            Ok(SyncInfo {
                sync_id: SyncId(r.get(0)?),
                account_id: AccountId(r.get(1)?),
                remote_root_id: r.get(2)?,
                generation: r.get(3)?,
                initial_download_complete: r.get(4)?,
            })
        },
    )
    .map_err(sql)
}
fn verify(db: &Connection, request: &OpenInventoryRequest) -> InventoryResult<()> {
    use std::os::unix::ffi::OsStrExt;
    let version: u32 = db
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(sql)?;
    if version != 10 {
        return Err(failure(InventoryError::UnsupportedSchema));
    }
    let info = sync_info(db)?;
    let root: Vec<u8> = db
        .query_row("SELECT local_root FROM sync_info WHERE id=1", [], |r| {
            r.get(0)
        })
        .map_err(sql)?;
    if info.sync_id != request.expected.sync_id
        || info.account_id != request.expected.account_id
        || info.remote_root_id != request.expected.remote_root_id
        || root != request.local_root.as_os_str().as_bytes()
    {
        return Err(failure(InventoryError::InvalidIdentity));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cached_status_is_available_while_a_transaction_holds_the_connection() {
        use std::{
            sync::{mpsc, Arc},
            time::Duration,
        };
        let path = std::env::temp_dir().join(format!("inventory-status-{}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        let manager = Arc::new(
            InventoryManager::open(OpenInventoryRequest {
                local_root: path.clone(),
                expected: SyncIdentity {
                    sync_id: SyncId("sync".into()),
                    account_id: AccountId("account".into()),
                    remote_root_id: "root".into(),
                },
                create_if_missing: true,
            })
            .unwrap()
            .inventory,
        );
        let (entered_tx, entered) = mpsc::channel();
        let (release_tx, release) = mpsc::channel();
        let writing = manager.clone();
        let writer = std::thread::spawn(move || {
            writing.run(true, |_| {
                entered_tx.send(()).unwrap();
                release.recv().unwrap();
                Ok(())
            })
        });
        entered.recv_timeout(Duration::from_secs(2)).unwrap();
        let reading = manager.clone();
        let (status_tx, status) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            status_tx
                .send(reading.status(InventoryStatusRequest {}))
                .unwrap()
        });
        let result = status.recv_timeout(Duration::from_secs(2));
        release_tx.send(()).unwrap();
        writer.join().unwrap().unwrap();
        reader.join().unwrap();
        assert_eq!(result.unwrap().availability, InventoryAvailability::Ready);
        drop(manager);
        std::fs::remove_dir_all(path).unwrap();
    }
}
