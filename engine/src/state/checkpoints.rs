//! Atomic change-feed checkpoints in their own exact, versioned SQLite format.
//! Observation commits precede download effects; the existing journal is untouched.
use super::{failed, Store};
use crate::{changes::Checkpoint, AccountId, AuthError, Cancellation, EngineError};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use std::path::Path;
const SCHEMA: &str = "CREATE TABLE checkpoints(client TEXT NOT NULL,account TEXT NOT NULL,remote TEXT NOT NULL,local TEXT NOT NULL,revision TEXT NOT NULL,record TEXT NOT NULL,PRIMARY KEY(client,account,remote,local));";
const MAX_BYTES: usize = 64 * 1024 * 1024;

/// Owns the dedicated feed-observation store; it cannot confirm file installation.
pub(crate) struct Checkpoints(Store);
/// Includes the feed's account namespace and the exact projected folder pair.
pub(crate) struct Scope<'a> {
    pub client: &'a str,
    pub account: &'a AccountId,
    pub remote: &'a str,
    pub local: &'a Path,
}
impl Checkpoints {
    pub(crate) fn open(path: &Path) -> Result<Self, EngineError> {
        Ok(Self(Store::open_format(path, 0x44534348, 1, SCHEMA)?))
    }
    /// Loads one validated inventory/cursor pair, preserving corrupt data on failure.
    pub(crate) fn load(
        &self,
        scope: &Scope<'_>,
    ) -> Result<Option<(String, Checkpoint)>, EngineError> {
        let row:Option<(String,String)>=self.0.0.query_row(
            "SELECT revision,record FROM checkpoints WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4",
            params![scope.client,scope.account.0,scope.remote,local(scope)?], |r| Ok((r.get(0)?,r.get(1)?)))
            .optional().map_err(failed)?;
        row.map(|(revision, json)| {
            if json.len() > MAX_BYTES || revision.is_empty() {
                return Err(EngineError::StateStorage);
            }
            let checkpoint: Checkpoint = serde_json::from_str(&json).map_err(failed)?;
            validate(scope, &checkpoint).map_err(|_| EngineError::StateStorage)?;
            Ok((revision, checkpoint))
        })
        .transpose()
    }
    /// Clears only the exact observed revision. CAS prevents a stale user action
    /// from suppressing newly discovered notices; inventory and cursor survive.
    pub(crate) fn acknowledge(
        &mut self,
        scope: &Scope<'_>,
        revision: &str,
        cancel: &Cancellation,
    ) -> Result<(), EngineError> {
        let (current, mut checkpoint) = self.load(scope)?.ok_or(EngineError::SyncBusy)?;
        if current != revision {
            return Err(EngineError::SyncBusy);
        }
        checkpoint.notices.clear();
        self.save(scope, Some(revision), &checkpoint, cancel)
            .map(|_| ())
    }
    /// CAS prevents a slow worker from replacing a newer cursor with stale data.
    /// Cursor and replayable inventory/notices either commit together or roll back.
    pub(crate) fn save(
        &mut self,
        scope: &Scope<'_>,
        expected: Option<&str>,
        checkpoint: &Checkpoint,
        cancel: &Cancellation,
    ) -> Result<String, EngineError> {
        validate(scope, checkpoint)?;
        let json = serde_json::to_string(checkpoint).map_err(failed)?;
        if json.len() > MAX_BYTES {
            return Err(EngineError::InventoryLimit);
        }
        if cancel.is_cancelled() {
            return Err(EngineError::Authentication(AuthError::Cancelled));
        }
        let tx = self
            .0
             .0
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failed)?;
        let actual:Option<String>=tx.query_row("SELECT revision FROM checkpoints WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4",params![scope.client,scope.account.0,scope.remote,local(scope)?],|r| r.get(0)).optional().map_err(failed)?;
        if actual.as_deref() != expected {
            return Err(EngineError::SyncBusy);
        }
        let revision = oauth2::CsrfToken::new_random().secret().clone();
        tx.execute("INSERT INTO checkpoints VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(client,account,remote,local) DO UPDATE SET revision=excluded.revision,record=excluded.record",params![scope.client,scope.account.0,scope.remote,local(scope)?,revision,json]).map_err(failed)?;
        if cancel.is_cancelled() {
            return Err(EngineError::Authentication(AuthError::Cancelled));
        }
        tx.commit().map_err(failed)?;
        Ok(revision)
    }
}
fn local<'a>(scope: &'a Scope<'_>) -> Result<&'a str, EngineError> {
    scope
        .local
        .to_str()
        .filter(|_| {
            scope.local.is_absolute() && !scope.client.is_empty() && !scope.account.0.is_empty()
        })
        .ok_or(EngineError::InvalidPlan)
}
fn validate(scope: &Scope<'_>, checkpoint: &Checkpoint) -> Result<(), EngineError> {
    local(scope)?;
    crate::changes::validate(scope.account, scope.remote, &checkpoint.inventory)?;
    if !crate::remote::valid_change_token(&checkpoint.cursor)
        || checkpoint.notices.len() > 100_000
        || checkpoint.notices.iter().any(|n| {
            !crate::configuration::valid_id(&n.id)
                || n.path.as_ref().is_some_and(|p| {
                    p.as_os_str().is_empty()
                        || p.components()
                            .any(|c| !matches!(c, std::path::Component::Normal(_)))
                })
        })
    {
        return Err(EngineError::InvalidPlan);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn checkpoint(cursor: &str) -> Checkpoint {
        Checkpoint {
            cursor: cursor.into(),
            inventory: crate::RemoteInventory {
                account_id: AccountId("account".into()),
                root_id: "root".into(),
                entries: Vec::new(),
            },
            notices: Vec::new(),
        }
    }
    #[test]
    fn atomic_reopen_isolation_cancellation_and_stale_worker() {
        let path = std::env::temp_dir()
            .join(format!(
                "drivesync-checkpoints-{}",
                oauth2::CsrfToken::new_random().secret()
            ))
            .join("changes.sqlite3");
        let account = AccountId("account".into());
        let scope = Scope {
            client: "client",
            account: &account,
            remote: "root",
            local: Path::new("/files"),
        };
        let mut store = Checkpoints::open(&path).unwrap();
        assert!(store.load(&scope).unwrap().is_none());
        store
            .save(&scope, None, &checkpoint("a"), &Cancellation::new())
            .unwrap();
        let (revision, old) = store.load(&scope).unwrap().unwrap();
        assert_eq!(old.cursor, "a");
        let mut reopened = Checkpoints::open(&path).unwrap();
        assert_eq!(reopened.load(&scope).unwrap().unwrap().0, revision);
        let cancelled = Cancellation::new();
        cancelled.cancel();
        assert!(reopened
            .save(&scope, Some(&revision), &checkpoint("b"), &cancelled)
            .is_err());
        reopened.0.0.execute_batch("CREATE TRIGGER fail BEFORE UPDATE ON checkpoints BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
        assert_eq!(
            reopened.save(
                &scope,
                Some(&revision),
                &checkpoint("b"),
                &Cancellation::new()
            ),
            Err(EngineError::StateStorage)
        );
        assert_eq!(reopened.load(&scope).unwrap().unwrap().1.cursor, "a");
        reopened.0 .0.execute_batch("DROP TRIGGER fail").unwrap();
        reopened
            .save(
                &scope,
                Some(&revision),
                &checkpoint("b"),
                &Cancellation::new(),
            )
            .unwrap();
        assert_eq!(
            store.save(
                &scope,
                Some(&revision),
                &checkpoint("c"),
                &Cancellation::new()
            ),
            Err(EngineError::SyncBusy)
        );
        assert_eq!(store.load(&scope).unwrap().unwrap().1.cursor, "b");
        for other in [
            Scope {
                client: "other",
                ..scope
            },
            Scope {
                remote: "other",
                ..scope
            },
            Scope {
                local: Path::new("/other"),
                ..scope
            },
        ] {
            assert!(store.load(&other).unwrap().is_none());
        }
        let other = AccountId("other".into());
        assert!(store
            .load(&Scope {
                account: &other,
                ..scope
            })
            .unwrap()
            .is_none());
        drop(store);
        drop(reopened);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn acknowledgment_is_durable_scoped_and_rejects_stale_or_cancelled_actions() {
        let path = std::env::temp_dir()
            .join(format!(
                "drivesync-ack-{}",
                oauth2::CsrfToken::new_random().secret()
            ))
            .join("changes.sqlite3");
        let account = AccountId("account".into());
        let scope = Scope {
            client: "client",
            account: &account,
            remote: "root",
            local: Path::new("/files"),
        };
        let mut store = Checkpoints::open(&path).unwrap();
        let mut original = checkpoint("cursor");
        original.notices.push(crate::changes::Notice {
            id: "old".into(),
            path: Some("old/file.txt".into()),
        });
        let revision = store
            .save(&scope, None, &original, &Cancellation::new())
            .unwrap();
        let other = Scope {
            local: Path::new("/other"),
            ..scope
        };
        store
            .save(&other, None, &original, &Cancellation::new())
            .unwrap();
        let cancelled = Cancellation::new();
        cancelled.cancel();
        assert!(store.acknowledge(&scope, &revision, &cancelled).is_err());
        assert_eq!(
            store.load(&scope).unwrap().unwrap().1.notices,
            original.notices
        );
        let newer = store
            .save(&scope, Some(&revision), &original, &Cancellation::new())
            .unwrap();
        assert_eq!(
            store.acknowledge(&scope, &revision, &Cancellation::new()),
            Err(EngineError::SyncBusy)
        );
        store
            .acknowledge(&scope, &newer, &Cancellation::new())
            .unwrap();
        let reopened = Checkpoints::open(&path).unwrap();
        let acknowledged = reopened.load(&scope).unwrap().unwrap().1;
        assert!(acknowledged.notices.is_empty());
        assert_eq!(acknowledged.cursor, original.cursor);
        assert_eq!(
            acknowledged.inventory.entries.len(),
            original.inventory.entries.len()
        );
        assert_eq!(
            reopened.load(&other).unwrap().unwrap().1.notices,
            original.notices
        );
        drop(reopened);
        drop(store);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn corrupt_foreign_and_symlink_stores_are_preserved() {
        let dir = std::env::temp_dir().join(format!(
            "drivesync-checkpoints-{}",
            oauth2::CsrfToken::new_random().secret()
        ));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("changes.sqlite3");
        std::fs::write(&path, b"invalid").unwrap();
        assert!(Checkpoints::open(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"invalid");
        let link = dir.join("link");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(Checkpoints::open(&link).is_err());
        let foreign = dir.join("foreign");
        drop(Store::open(&foreign).unwrap());
        assert!(Checkpoints::open(&foreign).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
