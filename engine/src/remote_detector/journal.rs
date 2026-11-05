//! Transactional checkpoint and at-least-once delivery journal.
//!
//! A checkpoint and its associated events are committed together. An
//! optimistic generation prevents competing checks from advancing a scope
//! based on stale state. This module never opens an old engine database.

use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::Duration,
};

/// Persistent identity of a monitored root and its change-log corpus.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Scope {
    pub client: String,
    pub account: String,
    pub root: String,
    pub corpus: String,
}

/// Persisted checkpoint payloads, interpreted by the detector.
pub(super) struct Checkpoint {
    pub generation: i64,
    pub cursor: String,
    pub inventory: Vec<u8>,
}

/// An event remains pending until explicitly acknowledged.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct PendingEvent {
    pub id: i64,
    pub payload: Vec<u8>,
}

/// Sanitized journal errors; database content is never included.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum JournalError {
    Storage,
    Incompatible,
    Stale,
}

/// Owns the current-format database in a private application directory.
pub(super) struct Journal {
    path: PathBuf,
}

impl Journal {
    /// Opens a private directory and initializes a new database when absent.
    /// Existing incompatible databases are rejected without being recreated.
    pub fn open(directory: &Path) -> Result<Self, JournalError> {
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)
            .map_err(|_| JournalError::Storage)?;
        let metadata = fs::symlink_metadata(directory).map_err(|_| JournalError::Storage)?;
        if !metadata.file_type().is_dir() || metadata.permissions().mode() & 0o077 != 0 {
            return Err(JournalError::Storage);
        }

        let path = directory.join("remote-detector.sqlite3");
        match fs::symlink_metadata(&path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Self::initialize(directory, &path)?;
            }
            Err(_) => return Err(JournalError::Storage),
        }
        let metadata = fs::symlink_metadata(&path).map_err(|_| JournalError::Storage)?;
        if !metadata.file_type().is_file() || metadata.permissions().mode() & 0o077 != 0 {
            return Err(JournalError::Storage);
        }
        let journal = Self { path };
        let version: i64 = journal
            .connect()?
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|_| JournalError::Storage)?;
        if version != 1 {
            return Err(JournalError::Incompatible);
        }
        Ok(journal)
    }

    /// Publishes a fully initialized database without replacing an existing
    /// file. Concurrent creators can race safely; abandoned temporary files
    /// are never interpreted as the active database.
    fn initialize(directory: &Path, path: &Path) -> Result<(), JournalError> {
        let temporary = directory.join(format!(
            ".remote-detector-{}.tmp",
            oauth2::CsrfToken::new_random().secret()
        ));
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|_| JournalError::Storage)?;
        let result = (|| {
            let mut connection = Self::connect_path(&temporary)?;
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|_| JournalError::Storage)?;
            transaction
                .execute_batch(
                    "CREATE TABLE scopes (
                        client TEXT NOT NULL, account TEXT NOT NULL,
                        root TEXT NOT NULL, corpus TEXT NOT NULL,
                        generation INTEGER NOT NULL, cursor TEXT NOT NULL,
                        inventory BLOB NOT NULL,
                        PRIMARY KEY (client, account, root, corpus)
                    );
                    CREATE TABLE events (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        client TEXT NOT NULL, account TEXT NOT NULL,
                        root TEXT NOT NULL, corpus TEXT NOT NULL,
                        payload BLOB NOT NULL
                    );
                    CREATE INDEX event_scope ON events(client, account, root, corpus, id);
                    PRAGMA user_version = 1;",
                )
                .map_err(|_| JournalError::Storage)?;
            transaction.commit().map_err(|_| JournalError::Storage)?;
            drop(connection);
            File::open(&temporary)
                .and_then(|file| file.sync_all())
                .map_err(|_| JournalError::Storage)?;
            match fs::hard_link(&temporary, path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => return Err(JournalError::Storage),
            }
            File::open(directory)
                .and_then(|dir| dir.sync_all())
                .map_err(|_| JournalError::Storage)?;
            Ok(())
        })();
        let cleanup = fs::remove_file(&temporary).map_err(|_| JournalError::Storage);
        result.and(cleanup)
    }

    fn connect(&self) -> Result<Connection, JournalError> {
        Self::connect_path(&self.path)
    }

    fn connect_path(path: &Path) -> Result<Connection, JournalError> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(|_| JournalError::Storage)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|_| JournalError::Storage)?;
        connection
            .pragma_update(None, "synchronous", "FULL")
            .map_err(|_| JournalError::Storage)?;
        Ok(connection)
    }

    pub fn known_corpus(
        &self,
        client: &str,
        account: &str,
        root: &str,
    ) -> Result<Option<String>, JournalError> {
        let connection = self.connect()?;
        connection.query_row(
            "SELECT CASE WHEN count(*) = 1 THEN min(corpus) END FROM scopes WHERE client=?1 AND account=?2 AND root=?3",
            params![client, account, root],
            |row| row.get(0),
        ).map_err(|_| JournalError::Storage)
    }

    /// Returns the last committed checkpoint, if the scope has been initialized.
    pub fn load(&self, scope: &Scope) -> Result<Option<Checkpoint>, JournalError> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT generation, cursor, inventory FROM scopes
                 WHERE client=?1 AND account=?2 AND root=?3 AND corpus=?4",
                params![scope.client, scope.account, scope.root, scope.corpus],
                |row| {
                    Ok(Checkpoint {
                        generation: row.get(0)?,
                        cursor: row.get(1)?,
                        inventory: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(|_| JournalError::Storage)
    }

    /// Atomically advances a matching generation and enqueues its observations.
    ///
    /// `expected` is `None` for first initialization. A stale writer changes
    /// neither checkpoint nor events. A commit error can have an ambiguous
    /// outcome; callers must reload before retrying.
    pub fn commit(
        &self,
        scope: &Scope,
        expected: Option<i64>,
        cursor: &str,
        inventory: &[u8],
        events: &[Vec<u8>],
    ) -> Result<(), JournalError> {
        let mut connection = self.connect()?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| JournalError::Storage)?;
        let current: Option<i64> = tx
            .query_row(
                "SELECT generation FROM scopes WHERE client=?1 AND account=?2 AND root=?3 AND corpus=?4",
                params![scope.client, scope.account, scope.root, scope.corpus],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| JournalError::Storage)?;
        if current != expected {
            return Err(JournalError::Stale);
        }
        let generation = current
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(JournalError::Storage)?;
        tx.execute(
            "INSERT INTO scopes (client, account, root, corpus, generation, cursor, inventory)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(client, account, root, corpus) DO UPDATE SET
               generation=excluded.generation, cursor=excluded.cursor, inventory=excluded.inventory",
            params![scope.client, scope.account, scope.root, scope.corpus, generation, cursor, inventory],
        )
        .map_err(|_| JournalError::Storage)?;
        for event in events {
            tx.execute(
                "INSERT INTO events (client, account, root, corpus, payload) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![scope.client, scope.account, scope.root, scope.corpus, event],
            )
            .map_err(|_| JournalError::Storage)?;
        }
        tx.commit().map_err(|_| JournalError::Storage)
    }

    /// Reads pending events without acknowledging them.
    pub fn pending(&self, scope: &Scope) -> Result<Vec<PendingEvent>, JournalError> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(
                "SELECT id, payload FROM events
                 WHERE client=?1 AND account=?2 AND root=?3 AND corpus=?4 ORDER BY id",
            )
            .map_err(|_| JournalError::Storage)?;
        let rows = statement
            .query_map(
                params![scope.client, scope.account, scope.root, scope.corpus],
                |row| {
                    Ok(PendingEvent {
                        id: row.get(0)?,
                        payload: row.get(1)?,
                    })
                },
            )
            .map_err(|_| JournalError::Storage)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| JournalError::Storage)
    }

    pub fn acknowledge_many(&self, scope: &Scope, ids: &[i64]) -> Result<(), JournalError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| JournalError::Storage)?;
        {
            let mut delete = transaction.prepare(
                "DELETE FROM events WHERE client=?1 AND account=?2 AND root=?3 AND corpus=?4 AND id=?5"
            ).map_err(|_| JournalError::Storage)?;
            for id in ids {
                delete
                    .execute(params![
                        scope.client,
                        scope.account,
                        scope.root,
                        scope.corpus,
                        id
                    ])
                    .map_err(|_| JournalError::Storage)?;
            }
        }
        transaction.commit().map_err(|_| JournalError::Storage)
    }

    /// Idempotently acknowledges a single event within its exact scope.
    pub fn acknowledge(&self, scope: &Scope, id: i64) -> Result<bool, JournalError> {
        let connection = self.connect()?;
        let removed = connection
            .execute(
                "DELETE FROM events WHERE id=?1 AND client=?2 AND account=?3 AND root=?4 AND corpus=?5",
                params![id, scope.client, scope.account, scope.root, scope.corpus],
            )
            .map_err(|_| JournalError::Storage)?;
        Ok(removed != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Directory(PathBuf);

    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "drivesync-detector-{}",
                oauth2::CsrfToken::new_random().secret()
            ));
            Self(path)
        }
    }

    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn scope(account: &str, root: &str) -> Scope {
        Scope {
            client: "test-client".into(),
            account: account.into(),
            root: root.into(),
            corpus: "user".into(),
        }
    }

    #[test]
    fn checkpoint_and_queue_survive_restart_and_acknowledgement_is_scoped() {
        let directory = Directory::new();
        let a = scope("a", "root");
        let b = scope("b", "root");
        let other_root = scope("a", "other-root");
        let journal = Journal::open(&directory.0).unwrap();
        assert!(journal.load(&a).unwrap().is_none());
        journal
            .commit(&a, None, "cursor-a", b"inventory-a", &[b"event-a".to_vec()])
            .unwrap();
        journal
            .commit(&b, None, "cursor-b", b"inventory-b", &[b"event-b".to_vec()])
            .unwrap();
        drop(journal);
        let journal = Journal::open(&directory.0).unwrap();
        let checkpoint = journal.load(&a).unwrap().unwrap();
        assert_eq!(checkpoint.generation, 1);
        assert_eq!(checkpoint.cursor, "cursor-a");
        assert_eq!(checkpoint.inventory, b"inventory-a");
        assert!(journal.load(&other_root).unwrap().is_none());
        let event = journal.pending(&a).unwrap().remove(0);
        assert!(!journal.acknowledge(&b, event.id).unwrap());
        assert_eq!(journal.pending(&a).unwrap(), vec![event]);
        let id = journal.pending(&a).unwrap()[0].id;
        assert!(journal.acknowledge(&a, id).unwrap());
        assert!(!journal.acknowledge(&a, id).unwrap());
        assert!(journal.pending(&a).unwrap().is_empty());
        assert_eq!(journal.pending(&b).unwrap().len(), 1);
    }

    #[test]
    fn stale_writer_cannot_advance_or_duplicate_events() {
        let directory = Directory::new();
        let first = Journal::open(&directory.0).unwrap();
        let second = Journal::open(&directory.0).unwrap();
        let scope = scope("a", "root");
        first.commit(&scope, None, "one", b"one", &[]).unwrap();
        second
            .commit(&scope, Some(1), "two", b"two", &[b"event".to_vec()])
            .unwrap();
        assert_eq!(
            first.commit(&scope, Some(1), "stale", b"stale", &[b"duplicate".to_vec()]),
            Err(JournalError::Stale)
        );
        assert_eq!(first.load(&scope).unwrap().unwrap().cursor, "two");
        assert_eq!(first.pending(&scope).unwrap().len(), 1);
    }

    #[test]
    fn failed_event_insertion_rolls_back_checkpoint_and_prior_events() {
        let directory = Directory::new();
        let journal = Journal::open(&directory.0).unwrap();
        let scope = scope("a", "root");
        journal
            .commit(&scope, None, "one", b"inventory-one", &[])
            .unwrap();
        journal
            .connect()
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER reject_event BEFORE INSERT ON events
                 WHEN NEW.payload = X'626164'
                 BEGIN SELECT RAISE(ABORT, 'injected failure'); END;",
            )
            .unwrap();
        assert_eq!(
            journal.commit(
                &scope,
                Some(1),
                "two",
                b"inventory-two",
                &[b"first".to_vec(), b"bad".to_vec()]
            ),
            Err(JournalError::Storage)
        );
        let checkpoint = journal.load(&scope).unwrap().unwrap();
        assert_eq!(checkpoint.generation, 1);
        assert_eq!(checkpoint.cursor, "one");
        assert_eq!(checkpoint.inventory, b"inventory-one");
        assert!(journal.pending(&scope).unwrap().is_empty());
    }

    #[test]
    fn incompatible_database_is_preserved_and_private_permissions_are_required() {
        let directory = Directory::new();
        let journal = Journal::open(&directory.0).unwrap();
        let path = journal.path.clone();
        journal
            .connect()
            .unwrap()
            .pragma_update(None, "user_version", 99)
            .unwrap();
        let original = fs::read(&path).unwrap();
        assert!(matches!(
            Journal::open(&directory.0),
            Err(JournalError::Incompatible)
        ));
        assert_eq!(fs::read(&path).unwrap(), original);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(
            Journal::open(&directory.0),
            Err(JournalError::Storage)
        ));
    }

    #[test]
    fn interrupted_initialization_does_not_publish_partial_database() {
        let directory = Directory::new();
        DirBuilder::new().mode(0o700).create(&directory.0).unwrap();
        let abandoned = directory.0.join(".remote-detector-interrupted.tmp");
        fs::write(&abandoned, b"partial").unwrap();
        let journal = Journal::open(&directory.0).unwrap();
        assert!(journal.load(&scope("a", "root")).unwrap().is_none());
        assert_eq!(fs::read(abandoned).unwrap(), b"partial");
    }

    #[test]
    fn concurrent_initialization_publishes_one_complete_database() {
        use std::sync::{Arc, Barrier};
        let directory = Directory::new();
        DirBuilder::new().mode(0o700).create(&directory.0).unwrap();
        let barrier = Arc::new(Barrier::new(8));
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let path = directory.0.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    Journal::open(&path).unwrap()
                })
            })
            .collect();
        for thread in threads {
            assert!(thread
                .join()
                .unwrap()
                .load(&scope("a", "root"))
                .unwrap()
                .is_none());
        }
    }

    #[test]
    fn killed_writer_recovers_uncommitted_sqlite_transaction() {
        const CHILD: &str = "DRIVESYNC_TEST_KILLED_JOURNAL";
        if let Some(path) = std::env::var_os(CHILD) {
            let journal = Journal::open(Path::new(&path)).unwrap();
            let result = journal.commit(
                &scope("a", "root"),
                Some(1),
                "new-cursor",
                b"new-inventory",
                &[b"new-event".to_vec()],
            );
            panic!("writer unexpectedly finished: {result:?}");
        }

        let directory = Directory::new();
        let journal = Journal::open(&directory.0).unwrap();
        let scope = scope("a", "root");
        journal
            .commit(
                &scope,
                None,
                "old-cursor",
                b"old-inventory",
                &[b"old-event".to_vec()],
            )
            .unwrap();
        let lock = journal.connect().unwrap();
        // A shared reader lets the writer modify pages but prevents its commit.
        lock.execute_batch("BEGIN; SELECT count(*) FROM scopes;")
            .unwrap();

        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "remote_detector::journal::tests::killed_writer_recovers_uncommitted_sqlite_transaction",
                "--nocapture",
            ])
            .env(CHILD, &directory.0)
            .spawn()
            .unwrap();
        let rollback_journal = directory.0.join("remote-detector.sqlite3-journal");
        let deadline = std::time::Instant::now() + Duration::from_secs(4);
        let mut started = false;
        while std::time::Instant::now() < deadline {
            if fs::metadata(&rollback_journal).is_ok_and(|metadata| metadata.len() > 0) {
                started = true;
                break;
            }
            if child.try_wait().unwrap().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let kill_result = child.kill();
        let exit = child.wait().unwrap();
        lock.execute_batch("ROLLBACK;").unwrap();
        assert!(started, "writer did not start a SQLite transaction");
        kill_result.expect("writer exited before interruption");
        assert!(!exit.success());

        let reopened = Journal::open(&directory.0).unwrap();
        let checkpoint = reopened.load(&scope).unwrap().unwrap();
        assert_eq!(checkpoint.generation, 1);
        assert_eq!(checkpoint.cursor, "old-cursor");
        assert_eq!(checkpoint.inventory, b"old-inventory");
        let events = reopened.pending(&scope).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload, b"old-event");
    }
}
