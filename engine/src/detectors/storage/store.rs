//! One transactional queue per account/synchronization/root identity.
use super::{codec, filesystem, models::*};
use crate::{detectors::LocalDetectorScope, models::File};
use rusqlite::{
    params, Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior,
};
use sha2::{Digest, Sha256};
use std::{
    fs::File as FsFile,
    os::{fd::AsRawFd, unix::ffi::OsStrExt},
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
const SCHEMA: &str = "
CREATE TABLE identity(account TEXT NOT NULL, sync TEXT NOT NULL, root BLOB NOT NULL);
CREATE TABLE control(id INTEGER PRIMARY KEY CHECK(id=1), delivery INTEGER NOT NULL CHECK(delivery IN (0,1)));
INSERT INTO control VALUES(1,0);
CREATE TABLE leases(id TEXT PRIMARY KEY, expires INTEGER NOT NULL, state INTEGER NOT NULL CHECK(state BETWEEN 0 AND 3), count INTEGER NOT NULL, disposition INTEGER);
CREATE TABLE events(seq INTEGER PRIMARY KEY AUTOINCREMENT, id TEXT NOT NULL UNIQUE, payload BLOB NOT NULL, digest BLOB NOT NULL, state INTEGER NOT NULL DEFAULT 0 CHECK(state BETWEEN 0 AND 2), lease TEXT REFERENCES leases(id));
CREATE INDEX delivery_order ON events(state,seq);
PRAGMA user_version=1;";
/// Retains private directory descriptors for the lifetime of SQLite's file names.
pub(crate) struct Store {
    connection: Mutex<Connection>,
    pub(crate) created: bool,
    _directory: FsFile,
    sync: String,
    clock: Arc<dyn Fn() -> i64 + Send + Sync>,
    cached: Mutex<QueueStats>,
}
fn sql(error: rusqlite::Error) -> QueueFailure {
    use rusqlite::ErrorCode::*;
    let kind = match error.sqlite_error_code() {
        Some(DatabaseCorrupt | NotADatabase) => QueueError::Corrupt,
        Some(DiskFull | TooBig) => QueueError::CapacityExceeded,
        Some(
            DatabaseBusy | DatabaseLocked | PermissionDenied | CannotOpen | SystemIoFailure
            | ReadOnly,
        ) => QueueError::Unavailable,
        Some(ConstraintViolation) => QueueError::Conflict,
        _ => QueueError::Corrupt,
    };
    QueueFailure::new(kind)
}
fn clock() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}
fn token() -> QueueResult<String> {
    let mut bytes = [0u8; 24];
    let mut offset = 0;
    while offset < bytes.len() {
        // SAFETY: the remaining byte slice is writable and the length is exact.
        let count = unsafe {
            libc::getrandom(
                bytes[offset..].as_mut_ptr().cast(),
                bytes.len() - offset,
                libc::GRND_NONBLOCK,
            )
        };
        if count < 0 {
            if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(QueueFailure::new(QueueError::Unavailable));
        }
        if count == 0 {
            return Err(QueueFailure::new(QueueError::Unavailable));
        }
        offset += count as usize;
    }
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn reap(tx: &Transaction<'_>, now: i64) -> QueueResult<()> {
    tx.execute("UPDATE events SET state=0,lease=NULL WHERE state=1 AND lease IN (SELECT id FROM leases WHERE state=0 AND expires<=?1)", [now]).map_err(sql)?;
    tx.execute(
        "UPDATE leases SET state=3 WHERE state=0 AND expires<=?1",
        [now],
    )
    .map_err(sql)?;
    Ok(())
}
impl Store {
    pub(crate) fn open(base: &Path, scope: &LocalDetectorScope) -> QueueResult<Self> {
        Self::open_clock(base, scope, Arc::new(clock))
    }
    pub(super) fn open_clock(
        base: &Path,
        scope: &LocalDetectorScope,
        clock: Arc<dyn Fn() -> i64 + Send + Sync>,
    ) -> QueueResult<Self> {
        if scope.account_id.0.trim().is_empty()
            || scope.sync_id.0.trim().is_empty()
            || !scope.local_root.is_absolute()
        {
            return Err(QueueFailure::new(QueueError::InvalidRequest));
        }
        let identity = serde_json::to_vec(&(
            &scope.account_id.0,
            &scope.sync_id.0,
            scope.local_root.as_os_str().as_bytes(),
        ))
        .map_err(|_| QueueFailure::new(QueueError::InvalidRequest))?;
        let key = format!("{:x}", Sha256::digest(&identity));
        let location = base.join(&key);
        if location.starts_with(&scope.local_root) || scope.local_root.starts_with(&location) {
            return Err(QueueFailure::new(QueueError::InvalidRequest));
        }
        // Validate the supplied app-private base before creating its identity directory.
        let _base = filesystem::directory(base)?;
        let directory = filesystem::directory(&base.join(&key))?;
        let (file, created) = filesystem::database(&directory)?;
        // SAFETY: flock operates on our live owned database descriptor. It serializes initialization only.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(QueueFailure::new(QueueError::Unavailable));
        }
        let mut connection = Connection::open_with_flags(
            base.join(key).join("events.sqlite3"),
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(sql)?;
        connection
            .busy_timeout(Duration::from_millis(1000))
            .map_err(sql)?;
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(sql)?;
        connection
            .pragma_update(None, "trusted_schema", "OFF")
            .map_err(sql)?;
        connection
            .pragma_update(None, "synchronous", "EXTRA")
            .map_err(sql)?;
        if !created {
            verify(&connection, scope)?;
        }
        // Rollback journal avoids WAL sidecar/backup assumptions; transactions still sync before acceptance.
        let mode: String = connection
            .query_row("PRAGMA journal_mode=DELETE", [], |r| r.get(0))
            .map_err(sql)?;
        if mode != "delete" {
            return Err(QueueFailure::new(QueueError::Unavailable));
        }
        connection
            .pragma_update(None, "max_page_count", 131072)
            .map_err(sql)?;
        let check: String = connection
            .query_row("PRAGMA quick_check", [], |r| r.get(0))
            .map_err(sql)?;
        if check != "ok" {
            return Err(QueueFailure::new(QueueError::Corrupt));
        }
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sql)?;
        if created {
            tx.execute_batch(SCHEMA).map_err(sql)?;
            tx.execute(
                "INSERT INTO identity VALUES(?1,?2,?3)",
                params![
                    scope.account_id.0,
                    scope.sync_id.0,
                    scope.local_root.as_os_str().as_bytes()
                ],
            )
            .map_err(sql)?;
        }
        verify(&tx, scope)?;
        reap(&tx, clock())?;
        tx.commit().map_err(sql)?;
        // SAFETY: release the initialization lock before descriptor destruction.
        unsafe {
            libc::flock(file.as_raw_fd(), libc::LOCK_UN);
        }
        drop(file);
        let store = Self {
            connection: Mutex::new(connection),
            created,
            _directory: directory,
            sync: scope.sync_id.0.clone(),
            clock,
            cached: Mutex::new(QueueStats::default()),
        };
        store.refresh()?;
        Ok(store)
    }
    fn run<T>(
        &self,
        clear_errors: bool,
        operation: impl FnOnce(&mut Connection, i64) -> QueueResult<T>,
    ) -> QueueResult<T> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| QueueFailure::new(QueueError::Internal))?;
        if let Some(
            kind @ (QueueError::Corrupt
            | QueueError::SchemaUnsupported
            | QueueError::IdentityMismatch),
        ) = self
            .cached
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .last_error
        {
            return Err(QueueFailure::new(kind));
        }
        let now = (self.clock)();
        let result = operation(&mut connection, now);
        let mut cached = self.cached.lock().unwrap_or_else(|e| e.into_inner());
        match &result {
            Ok(_) => match stats(&connection, now) {
                Ok(mut stats) => {
                    if !clear_errors {
                        stats.last_error = cached.last_error;
                    }
                    *cached = stats;
                }
                Err(error) => {
                    cached.last_error = Some(error.kind);
                    return Err(error);
                }
            },
            Err(error) => cached.last_error = Some(error.kind),
        }
        result
    }
    pub(crate) fn snapshot(&self) -> QueueStats {
        self.cached
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub(crate) fn refresh(&self) -> QueueResult<()> {
        self.run(false, |_, _| Ok(()))
    }
    pub(crate) fn enqueue(&self, file: &File) -> QueueResult<bool> {
        if file.sync_id.0 != self.sync {
            return Err(QueueFailure::new(QueueError::IdentityMismatch));
        }
        let payload = codec::encode(file)?;
        let digest = Sha256::digest(&payload).to_vec();
        self.run(true, |connection, _| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(sql)?;
            let existing: Option<(Vec<u8>, Vec<u8>)> = tx
                .query_row(
                    "SELECT payload,digest FROM events WHERE id=?1",
                    [&file.id.0],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()
                .map_err(sql)?;
            if let Some((existing, stored_digest)) = existing {
                self.decode(file.id.0.clone(), &existing, stored_digest)?;
                if existing != payload {
                    return Err(QueueFailure::new(QueueError::Conflict));
                }
                return Ok(false);
            }
            let count: i64 = tx
                .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
                .map_err(sql)?;
            if count >= 100_000 {
                return Err(QueueFailure::new(QueueError::CapacityExceeded));
            }
            tx.execute(
                "INSERT INTO events(id,payload,digest) VALUES(?1,?2,?3)",
                params![file.id.0, payload, digest],
            )
            .map_err(sql)?;
            tx.commit().map_err(sql)?;
            Ok(true)
        })
    }
    pub(crate) fn set_delivery(&self, state: DeliveryState) -> QueueResult<()> {
        self.run(false, |connection, _| {
            connection
                .execute(
                    "UPDATE control SET delivery=?1 WHERE id=1",
                    [if state == DeliveryState::Enabled {
                        1
                    } else {
                        0
                    }],
                )
                .map_err(sql)?;
            Ok(())
        })
    }
    pub(crate) fn claim(&self, request: TakeEventsRequest) -> QueueResult<FileBatch> {
        let millis: i64 = request
            .lease_duration
            .as_millis()
            .try_into()
            .map_err(|_| QueueFailure::new(QueueError::InvalidRequest))?;
        if request.max_events == 0 || request.max_events > 64 || millis <= 0 || millis > 3_600_000 {
            return Err(QueueFailure::new(QueueError::InvalidRequest));
        }
        self.run(false, |connection, now| {
            let expires = now
                .checked_add(millis)
                .ok_or_else(|| QueueFailure::new(QueueError::InvalidRequest))?;
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(sql)?;
            let enabled: bool = tx
                .query_row("SELECT delivery FROM control WHERE id=1", [], |r| r.get(0))
                .map_err(sql)?;
            if !enabled {
                return Err(QueueFailure::new(QueueError::DeliveryPaused));
            }
            reap(&tx, now)?;
            let busy: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM leases WHERE state=0)",
                    [],
                    |r| r.get(0),
                )
                .map_err(sql)?;
            if busy {
                tx.commit().map_err(sql)?;
                return Ok(FileBatch {
                    batch_id: None,
                    files: vec![],
                });
            }
            let files = {
                let mut statement = tx
                    .prepare(
                        "SELECT id,payload,digest FROM events WHERE state=0 ORDER BY seq LIMIT ?1",
                    )
                    .map_err(sql)?;
                let mut rows = statement.query([request.max_events]).map_err(sql)?;
                let mut files = Vec::new();
                let mut bytes = 0;
                while let Some(row) = rows.next().map_err(sql)? {
                    let payload: Vec<u8> = row.get(1).map_err(sql)?;
                    if bytes + payload.len() > 8 * 1024 * 1024 {
                        break;
                    }
                    bytes += payload.len();
                    files.push(self.decode(
                        row.get(0).map_err(sql)?,
                        &payload,
                        row.get(2).map_err(sql)?,
                    )?);
                }
                files
            };
            if files.is_empty() {
                tx.commit().map_err(sql)?;
                return Ok(FileBatch {
                    batch_id: None,
                    files,
                });
            }
            let id = token()?;
            tx.execute(
                "INSERT INTO leases(id,expires,state,count) VALUES(?1,?2,0,?3)",
                params![id, expires, files.len()],
            )
            .map_err(sql)?;
            for file in &files {
                tx.execute(
                    "UPDATE events SET state=1,lease=?1 WHERE id=?2 AND state=0",
                    params![id, file.id.0],
                )
                .map_err(sql)?;
            }
            tx.commit().map_err(sql)?;
            Ok(FileBatch {
                batch_id: Some(BatchId(id)),
                files,
            })
        })
    }
    pub(crate) fn acknowledge(
        &self,
        id: &BatchId,
        disposition: EventDisposition,
    ) -> QueueResult<u32> {
        self.finish(id, Some(disposition))
    }
    pub(crate) fn release(&self, id: &BatchId) -> QueueResult<u32> {
        self.finish(id, None)
    }
    fn finish(&self, id: &BatchId, disposition: Option<EventDisposition>) -> QueueResult<u32> {
        self.run(false, |connection, now| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(sql)?;
            let lease: Option<(i64, i64, u32, Option<i64>)> = tx
                .query_row(
                    "SELECT state,expires,count,disposition FROM leases WHERE id=?1",
                    [&id.0],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .optional()
                .map_err(sql)?;
            let (state, expires, count, old_disposition) =
                lease.ok_or_else(|| QueueFailure::new(QueueError::InvalidLease))?;
            let value = disposition.map(|d| {
                if d == EventDisposition::Processed {
                    0i64
                } else {
                    1i64
                }
            });
            if state == 1 && value.is_some() && old_disposition == value {
                return Ok(count);
            }
            if state == 2 && value.is_none() {
                return Ok(count);
            }
            if state != 0 {
                return Err(QueueFailure::new(if state == 3 {
                    QueueError::LeaseExpired
                } else {
                    QueueError::InvalidLease
                }));
            }
            if expires <= now {
                return Err(QueueFailure::new(QueueError::LeaseExpired));
            }
            {
                let mut statement = tx
                    .prepare("SELECT id,payload,digest FROM events WHERE state=1 AND lease=?1")
                    .map_err(sql)?;
                let mut rows = statement.query([&id.0]).map_err(sql)?;
                while let Some(row) = rows.next().map_err(sql)? {
                    let payload: Vec<u8> = row.get(1).map_err(sql)?;
                    self.decode(row.get(0).map_err(sql)?, &payload, row.get(2).map_err(sql)?)?;
                }
            }
            let changed = if value.is_some() {
                tx.execute(
                    "UPDATE events SET state=2 WHERE state=1 AND lease=?1",
                    [&id.0],
                )
                .map_err(sql)?
            } else {
                tx.execute(
                    "UPDATE events SET state=0,lease=NULL WHERE state=1 AND lease=?1",
                    [&id.0],
                )
                .map_err(sql)?
            };
            if changed != count as usize {
                return Err(QueueFailure::new(QueueError::Corrupt));
            }
            tx.execute(
                "UPDATE leases SET state=?1,disposition=?2 WHERE id=?3",
                params![if value.is_some() { 1 } else { 2 }, value, id.0],
            )
            .map_err(sql)?;
            tx.commit().map_err(sql)?;
            Ok(count)
        })
    }
    fn decode(&self, id: String, payload: &[u8], digest: Vec<u8>) -> QueueResult<File> {
        if Sha256::digest(payload).as_slice() != digest {
            return Err(QueueFailure::new(QueueError::Corrupt));
        }
        let file = codec::decode(payload)?;
        if file.id.0 != id || file.sync_id.0 != self.sync {
            return Err(QueueFailure::new(QueueError::Corrupt));
        }
        Ok(file)
    }
    pub(crate) fn recent(&self) -> QueueResult<Vec<File>> {
        self.run(false, |connection, _| {
            let mut statement = connection
                .prepare("SELECT id,payload,digest FROM events ORDER BY seq DESC LIMIT 100")
                .map_err(sql)?;
            let mut rows = statement.query([]).map_err(sql)?;
            let mut files = Vec::new();
            while let Some(row) = rows.next().map_err(sql)? {
                let payload: Vec<u8> = row.get(1).map_err(sql)?;
                files.push(self.decode(
                    row.get(0).map_err(sql)?,
                    &payload,
                    row.get(2).map_err(sql)?,
                )?);
            }
            files.reverse();
            Ok(files)
        })
    }
}
fn stats(connection: &Connection, now: i64) -> QueueResult<QueueStats> {
    let enabled: bool = connection
        .query_row("SELECT delivery FROM control WHERE id=1", [], |r| r.get(0))
        .map_err(sql)?;
    connection.query_row("SELECT (SELECT count(*) FROM events WHERE state=0) + (SELECT count(*) FROM events e JOIN leases l ON e.lease=l.id WHERE e.state=1 AND l.expires<=?1), (SELECT count(*) FROM events e JOIN leases l ON e.lease=l.id WHERE e.state=1 AND l.expires>?1), (SELECT count(*) FROM events WHERE state=2), (SELECT count(*) FROM events)", [now], |r| Ok(QueueStats { pending: r.get(0)?, leased: r.get(1)?, confirmed: r.get(2)?, detected: r.get(3)?, last_error: None, delivery: if enabled { DeliveryState::Enabled } else { DeliveryState::Paused } })).map_err(sql)
}

fn verify(connection: &Connection, scope: &LocalDetectorScope) -> QueueResult<()> {
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(sql)?;
    if version != 1 {
        return Err(QueueFailure::new(QueueError::SchemaUnsupported));
    }
    let stored: (String, String, Vec<u8>) = connection
        .query_row("SELECT account,sync,root FROM identity", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .map_err(sql)?;
    if stored
        != (
            scope.account_id.0.clone(),
            scope.sync_id.0.clone(),
            scope.local_root.as_os_str().as_bytes().to_vec(),
        )
    {
        return Err(QueueFailure::new(QueueError::IdentityMismatch));
    }
    let count: i64 = connection
        .query_row("SELECT count(*) FROM identity", [], |r| r.get(0))
        .map_err(sql)?;
    let invalid: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM events e LEFT JOIN leases l ON e.lease=l.id WHERE (e.state=0 AND e.lease IS NOT NULL) OR (e.state=1 AND (l.id IS NULL OR l.state!=0)) OR (e.state=2 AND (l.id IS NULL OR l.state!=1)))", [], |r| r.get(0)).map_err(sql)?;
    let controls: i64 = connection
        .query_row(
            "SELECT count(*) FROM control WHERE id=1 AND delivery IN(0,1)",
            [],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if count != 1 || controls != 1 || invalid {
        return Err(QueueFailure::new(QueueError::Corrupt));
    }
    Ok(())
}
