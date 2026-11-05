//! Durable observation history, independent of synchronization work.
use crate::{AccountId, LocalBatch, LocalNotice, LocalStateError};
use rusqlite::{params, Connection, OpenFlags};
use std::{
    fs::{self, DirBuilder, OpenOptions},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const PAGE_SIZE: usize = 100;

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
             CREATE INDEX IF NOT EXISTS activity_account ON activity(client, account, id);",
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
        {
            let mut insert = tx.prepare(
                "INSERT INTO activity(client, account, observed_at, notice) VALUES(?1, ?2, ?3, ?4)",
            ).map_err(storage)?;
            for notice in notices {
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

fn storage(_: impl std::fmt::Debug) -> LocalStateError {
    LocalStateError::Storage
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LocalActivity, LocalCoverageGap, LocalItemKind, LocalScope, LocalWatchState};
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

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
