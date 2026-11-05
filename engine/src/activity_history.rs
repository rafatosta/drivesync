//! Durable observation history, independent of synchronization work.
use crate::{AccountId, LocalActivity, LocalBatch, LocalNotice, LocalStateError};
use rusqlite::{params, Connection, OpenFlags};
use std::{
    fs::{self, DirBuilder, OpenOptions},
    os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const PAGE_SIZE: usize = 100;
const RENAME_WINDOW: i64 = 3;

/// Operation confirmed by the remote-to-local synchronization flow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RemoteActivity {
    Added,
    Renamed,
    Updated,
    RenamedAndUpdated,
    Removed,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct StoredNotice {
    #[serde(flatten)]
    notice: LocalNotice,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    remote: Option<RemoteActivity>,
    #[serde(default)]
    root: Vec<u8>,
}

/// One observed local activity, ordered by the durable sequence number.
#[derive(Debug)]
pub struct ActivityRecord {
    pub id: i64,
    pub account: AccountId,
    pub observed_at: i64,
    pub notice: LocalNotice,
    pub remote: Option<RemoteActivity>,
}

/// One page and its cursor for older records, when available.
#[derive(Debug, Default)]
pub struct ActivityPage {
    pub records: Vec<ActivityRecord>,
    pub older: Option<i64>,
}

pub struct ActivityHistory(Connection);

impl ActivityHistory {
    /// Opens the private history in the application's data directory.
    pub fn application() -> Result<Self, LocalStateError> {
        let root = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share"))
            })
            .filter(|path| path.is_absolute())
            .ok_or(LocalStateError::Storage)?;
        Self::open(&root.join("drivesync/activity-history"))
    }

    fn open(directory: &Path) -> Result<Self, LocalStateError> {
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)
            .map_err(storage)?;
        let metadata = fs::symlink_metadata(directory).map_err(storage)?;
        if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
            return Err(LocalStateError::Storage);
        }
        let path = directory.join("history.sqlite3");
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
        {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(storage(error)),
        }
        let metadata = fs::symlink_metadata(&path).map_err(storage)?;
        if !metadata.is_file() || metadata.permissions().mode() & 0o077 != 0 {
            return Err(LocalStateError::Storage);
        }
        let db = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(storage)?;
        db.busy_timeout(Duration::from_secs(2)).map_err(storage)?;
        db.pragma_update(None, "synchronous", "FULL")
            .map_err(storage)?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS activity (
                id INTEGER PRIMARY KEY, client TEXT NOT NULL, account TEXT NOT NULL,
                observed_at INTEGER NOT NULL, notice BLOB NOT NULL);
             CREATE INDEX IF NOT EXISTS activity_account ON activity(client, account, id);
             CREATE INDEX IF NOT EXISTS activity_time ON activity(client, account, observed_at);
             CREATE TABLE IF NOT EXISTS activity_echoes (id INTEGER PRIMARY KEY, rename_id INTEGER NOT NULL);
             CREATE INDEX IF NOT EXISTS activity_echo_rename ON activity_echoes(rename_id);",
        )
        .map_err(storage)?;
        Ok(Self(db))
    }

    /// Records a batch in observation order. Coverage warnings remain separate.
    pub fn append(&mut self, batch: &LocalBatch) -> Result<(), LocalStateError> {
        let notices = batch
            .notices
            .iter()
            .filter(|notice| !matches!(notice.activity, crate::LocalActivity::Recheck(_)))
            .map(|notice| StoredNotice {
                notice: notice.clone(),
                remote: None,
                root: batch.scope.root().as_os_str().as_bytes().to_vec(),
            });
        self.append_notices(&batch.scope, notices)
    }

    pub(crate) fn append_remote(
        scope: &crate::LocalScope,
        notice: LocalNotice,
        remote: RemoteActivity,
    ) -> Result<(), LocalStateError> {
        Self::application()?.append_notices(
            scope,
            [StoredNotice {
                notice,
                remote: Some(remote),
                root: scope.root().as_os_str().as_bytes().to_vec(),
            }],
        )
    }

    fn append_notices(
        &mut self,
        scope: &crate::LocalScope,
        notices: impl IntoIterator<Item = StoredNotice>,
    ) -> Result<(), LocalStateError> {
        let observed_at = i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(storage)?
                .as_secs(),
        )
        .map_err(storage)?;
        let tx = self.0.transaction().map_err(storage)?;
        let mut check_renames = false;
        {
            let mut insert = tx.prepare(
                "INSERT INTO activity(client, account, observed_at, notice) VALUES(?1, ?2, ?3, ?4)",
            ).map_err(storage)?;
            for notice in notices {
                check_renames |= matches!(
                    notice.remote,
                    Some(RemoteActivity::Renamed | RemoteActivity::RenamedAndUpdated)
                ) || (notice.remote.is_none()
                    && matches!(
                        notice.notice.activity,
                        LocalActivity::MovedOut | LocalActivity::Created
                    ));
                let payload = serde_json::to_vec(&notice).map_err(storage)?;
                insert
                    .execute(params![
                        scope.client(),
                        scope.account(),
                        observed_at,
                        payload
                    ])
                    .map_err(storage)?;
            }
        }
        if check_renames {
            group_rename_echoes(&tx, scope, observed_at)?;
        }
        tx.commit().map_err(storage)
    }

    pub(crate) fn page(
        &self,
        client: &str,
        accounts: &[AccountId],
        before: Option<i64>,
    ) -> Result<ActivityPage, LocalStateError> {
        if accounts.is_empty() {
            return Ok(ActivityPage::default());
        }
        let accounts = serde_json::to_string(&accounts.iter().map(|id| &id.0).collect::<Vec<_>>())
            .map_err(storage)?;
        let mut query = self
            .0
            .prepare(
                "SELECT id, account, observed_at, notice FROM activity
             WHERE client = ?1 AND id < ?2 AND account IN (SELECT value FROM json_each(?3))
               AND id NOT IN (SELECT id FROM activity_echoes)
             ORDER BY id DESC LIMIT ?4",
            )
            .map_err(storage)?;
        let limit = (PAGE_SIZE + 1) as i64;
        let before = before.unwrap_or(i64::MAX);
        let mut rows = query
            .query(params![client, before, accounts, limit])
            .map_err(storage)?;
        let mut page = ActivityPage::default();
        while let Some(row) = rows.next().map_err(storage)? {
            if page.records.len() == PAGE_SIZE {
                page.older = page.records.last().map(|record| record.id);
                break;
            }
            let payload: Vec<u8> = row.get(3).map_err(storage)?;
            let stored: StoredNotice =
                serde_json::from_slice(&payload).map_err(|_| LocalStateError::Incompatible)?;
            page.records.push(ActivityRecord {
                id: row.get(0).map_err(storage)?,
                account: AccountId(row.get(1).map_err(storage)?),
                observed_at: row.get(2).map_err(storage)?,
                notice: stored.notice,
                remote: stored.remote,
            });
        }
        Ok(page)
    }
}

// Group only complete, unambiguous pairs. Keep the original rows so this
// presentation rule never changes the observation or synchronization records.
fn group_rename_echoes(
    db: &Connection,
    scope: &crate::LocalScope,
    now: i64,
) -> Result<(), LocalStateError> {
    let mut query = db
        .prepare(
            "SELECT id, observed_at, notice FROM activity
         WHERE client = ?1 AND account = ?2 AND observed_at BETWEEN ?3 AND ?4
           AND id NOT IN (SELECT id FROM activity_echoes)
         ORDER BY id DESC LIMIT 257",
        )
        .map_err(storage)?;
    let rows = query
        .query_map(
            params![
                scope.client(),
                scope.account(),
                now - 2 * RENAME_WINDOW,
                now + RENAME_WINDOW
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            },
        )
        .map_err(storage)?;
    let mut records = Vec::new();
    for row in rows {
        let (id, time, payload) = row.map_err(storage)?;
        let record: StoredNotice =
            serde_json::from_slice(&payload).map_err(|_| LocalStateError::Incompatible)?;
        if record.root == scope.root().as_os_str().as_bytes() {
            records.push((id, time, record));
        }
    }
    if records.len() >= 257 {
        return Ok(());
    }
    let mut pairs = Vec::new();
    for (rename_id, time, record) in &records {
        if !matches!(
            record.remote,
            Some(RemoteActivity::Renamed | RemoteActivity::RenamedAndUpdated)
        ) {
            continue;
        }
        let grouped: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM activity_echoes WHERE rename_id = ?1)",
                [rename_id],
                |row| row.get(0),
            )
            .map_err(storage)?;
        if grouped {
            continue;
        }
        let LocalActivity::Moved { from } = &record.notice.activity else {
            continue;
        };
        let matching = |path: &Path, activity: LocalActivity| {
            let mut matches = records.iter().filter(|(_, observed, candidate)| {
                observed.abs_diff(*time) <= RENAME_WINDOW as u64
                    && candidate.remote.is_none()
                    && candidate.notice.path == path
                    && candidate.notice.activity == activity
            });
            let first = matches.next()?.0;
            matches.next().is_none().then_some(first)
        };
        if let (Some(source), Some(destination)) = (
            matching(from, LocalActivity::MovedOut),
            matching(&record.notice.path, LocalActivity::Created),
        ) {
            pairs.push((*rename_id, source, destination));
        }
    }
    for &(rename_id, source, destination) in &pairs {
        if pairs
            .iter()
            .filter(|(_, a, b)| *a == source || *b == destination)
            .count()
            == 1
        {
            db.execute(
                "INSERT OR IGNORE INTO activity_echoes(id, rename_id) VALUES (?1, ?3), (?2, ?3)",
                params![source, destination, rename_id],
            )
            .map_err(storage)?;
        }
    }
    Ok(())
}

fn storage(_: impl std::fmt::Debug) -> LocalStateError {
    LocalStateError::Storage
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LocalActivity, LocalCoverageGap, LocalItemKind, LocalScope, LocalWatchState};
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    #[test]
    fn confirmed_rename_groups_one_pair_and_preserves_independent_activity() {
        let suffix = oauth2::CsrfToken::new_random();
        let directory = std::env::temp_dir().join(format!("drivesync-echo-{}", suffix.secret()));
        let mut history = ActivityHistory::open(&directory).unwrap();
        let scope = LocalScope::new("client", "account", Path::new("/test")).unwrap();
        let event = |path: &str, activity, remote, root: &[u8]| StoredNotice {
            notice: LocalNotice {
                path: path.into(),
                object_type: LocalItemKind::File,
                activity,
            },
            remote,
            root: root.to_vec(),
        };
        history
            .append_notices(
                &scope,
                [
                    event("old", LocalActivity::MovedOut, None, b"/test"),
                    event(
                        "new",
                        LocalActivity::Moved { from: "old".into() },
                        Some(RemoteActivity::Renamed),
                        b"/test",
                    ),
                    event("new", LocalActivity::Created, None, b"/another-root"),
                ],
            )
            .unwrap();
        let accounts = [AccountId("account".into())];
        assert_eq!(
            history
                .page("client", &accounts, None)
                .unwrap()
                .records
                .len(),
            3
        );
        history
            .append_notices(
                &scope,
                [event("new", LocalActivity::Created, None, b"/test")],
            )
            .unwrap();
        let page = history.page("client", &accounts, None).unwrap();
        assert_eq!(page.records.len(), 2);
        assert_eq!(page.records[1].remote, Some(RemoteActivity::Renamed));
        history
            .append_notices(
                &scope,
                [
                    event("old", LocalActivity::MovedOut, None, b"/test"),
                    event("new", LocalActivity::Created, None, b"/test"),
                ],
            )
            .unwrap();
        assert_eq!(
            history
                .page("client", &accounts, None)
                .unwrap()
                .records
                .len(),
            4
        );
        let count: i64 = history
            .0
            .query_row("SELECT count(*) FROM activity", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 6);
        drop(history);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn history_survives_reopen_and_pages_only_selected_accounts() {
        let suffix = oauth2::CsrfToken::new_random();
        let directory =
            std::env::temp_dir().join(format!("drivesync-activity-{}", suffix.secret()));
        let mut store = ActivityHistory::open(&directory).unwrap();
        let path = PathBuf::from(OsString::from_vec(b"old-\xff".to_vec()));
        let notice = LocalNotice {
            path: "new".into(),
            object_type: LocalItemKind::File,
            activity: LocalActivity::Moved { from: path },
        };
        let batch = |client, account, count| LocalBatch {
            scope: LocalScope::new(client, account, Path::new("/test")).unwrap(),
            state: LocalWatchState::Observing,
            notices: std::iter::repeat_n(notice.clone(), count)
                .chain([LocalNotice {
                    path: PathBuf::new(),
                    object_type: LocalItemKind::Unknown,
                    activity: LocalActivity::Recheck(LocalCoverageGap::ResourceLimit),
                }])
                .collect(),
        };
        store
            .append(&batch("client", "first", PAGE_SIZE + 1))
            .unwrap();
        store.append(&batch("client", "second", 1)).unwrap();
        store.append(&batch("other-client", "first", 1)).unwrap();
        store
            .append_notices(
                &batch("client", "second", 0).scope,
                [StoredNotice {
                    notice: notice.clone(),
                    remote: Some(RemoteActivity::Renamed),
                    root: b"/test".to_vec(),
                }],
            )
            .unwrap();
        drop(store);

        let store = ActivityHistory::open(&directory).unwrap();
        let first = AccountId("first".into());
        let second = AccountId("second".into());
        let all = store
            .page("client", &[first.clone(), second.clone()], None)
            .unwrap();
        assert_eq!(all.records[0].account, second);
        assert_eq!(all.records[0].remote, Some(RemoteActivity::Renamed));
        assert_eq!(all.records[1].remote, None);
        let page = store
            .page("client", std::slice::from_ref(&first), None)
            .unwrap();
        assert_eq!(page.records.len(), PAGE_SIZE);
        assert!(page
            .records
            .iter()
            .all(|record| record.account == first && record.notice == notice));
        let older = store.page("client", &[first], page.older).unwrap();
        assert_eq!(older.records.len(), 1);
        assert!(older.records[0].id < page.records.last().unwrap().id);
        assert!(older.older.is_none());
        drop(store);
        fs::remove_dir_all(directory).unwrap();
    }
}
