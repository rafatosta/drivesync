//! Private transactional state for local inventory observations.
//!
//! The caller must establish that an inventory is complete and its root is
//! accessible before committing. This journal cannot validate a live scan.

use super::{
    reconcile_local_inventories, valid_path, LocalEntry, LocalInventory, LocalObservation,
};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    time::Duration,
};

/// Persistent identity of a local monitoring scope.
///
/// The byte-exact absolute root is part of the scope. Changing the selected
/// root creates a separate queue and does not erase the previous one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalScope {
    client: String,
    account: String,
    root: Vec<u8>,
}

impl LocalScope {
    /// Constructs a scope with an absolute, normalized, non-root Unix path.
    ///
    /// # Errors
    ///
    /// Returns [`LocalJournalError::InvalidScope`] for empty identifiers,
    /// control characters, or an invalid root. This does not open the path or
    /// verify the account; callers must do so using the configured account.
    pub fn new(client: &str, account: &str, root: &Path) -> Result<Self, LocalJournalError> {
        let bytes = root.as_os_str().as_bytes();
        if client.is_empty()
            || account.is_empty()
            || client.chars().any(char::is_control)
            || account.chars().any(char::is_control)
            || !bytes.starts_with(b"/")
            || bytes.len() == 1
            || !valid_path(&bytes[1..])
        {
            return Err(LocalJournalError::InvalidScope);
        }
        Ok(Self {
            client: client.to_owned(),
            account: account.to_owned(),
            root: bytes.to_vec(),
        })
    }
}

/// A durable inventory and its optimistic concurrency generation.
#[derive(Debug, PartialEq, Eq)]
pub struct LocalCheckpoint {
    /// Generation to pass to the next commit.
    pub generation: i64,
    /// Last inventory whose observations were committed.
    pub inventory: LocalInventory,
}

/// An observation that remains pending until explicitly acknowledged.
#[derive(Debug, PartialEq, Eq)]
pub struct LocalDelivery {
    /// Stable delivery ID in this journal.
    pub id: i64,
    /// Baseline or conservative re-evaluation observation.
    pub observation: LocalObservation,
}

/// Journal failure categories without database or path contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalJournalError {
    /// The caller supplied an invalid account, client, root, or event ID.
    InvalidScope,
    /// An I/O or database operation failed; a commit error may be ambiguous.
    Storage,
    /// The database version or stored content is unsupported or invalid.
    Incompatible,
    /// Another writer committed after the caller read the checkpoint.
    Stale,
    /// The root's observed device or inode changed since the last checkpoint.
    RootChanged,
}

impl std::fmt::Display for LocalJournalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidScope => "The local detection scope or event ID is invalid.",
            Self::Storage => "The local detection journal could not be accessed.",
            Self::Incompatible => "The local detection journal has incompatible or invalid data.",
            Self::Stale => "The local detection checkpoint changed concurrently.",
            Self::RootChanged => "The local detection root identity changed.",
        })
    }
}

impl std::error::Error for LocalJournalError {}

/// A separate SQLite journal stored in a private application directory.
pub struct LocalJournal {
    path: PathBuf,
}

impl LocalJournal {
    /// Opens or creates a journal without reading old engine databases.
    ///
    /// The directory and database must be private and not symbolic links.
    /// Initial creation publishes a fully initialized file without replacing
    /// an existing database; incompatible data is preserved.
    ///
    /// # Errors
    ///
    /// Returns a storage or incompatible error if access, initialization,
    /// permissions, or format validation fails.
    pub fn open(directory: &Path) -> Result<Self, LocalJournalError> {
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)
            .map_err(|_| LocalJournalError::Storage)?;
        let metadata = fs::symlink_metadata(directory).map_err(|_| LocalJournalError::Storage)?;
        if !metadata.file_type().is_dir() || metadata.permissions().mode() & 0o077 != 0 {
            return Err(LocalJournalError::Storage);
        }
        let path = directory.join("local-detector.sqlite3");
        match fs::symlink_metadata(&path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Self::initialize(directory, &path)?;
            }
            Err(_) => return Err(LocalJournalError::Storage),
        }
        let metadata = fs::symlink_metadata(&path).map_err(|_| LocalJournalError::Storage)?;
        if !metadata.file_type().is_file() || metadata.permissions().mode() & 0o077 != 0 {
            return Err(LocalJournalError::Storage);
        }
        let journal = Self { path };
        let version: i64 = journal
            .connect()?
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|_| LocalJournalError::Storage)?;
        if version != 1 {
            return Err(LocalJournalError::Incompatible);
        }
        Ok(journal)
    }

    fn initialize(directory: &Path, path: &Path) -> Result<(), LocalJournalError> {
        let temporary = directory.join(format!(
            ".local-detector-{}.tmp",
            oauth2::CsrfToken::new_random().secret()
        ));
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|_| LocalJournalError::Storage)?;
        let result = (|| {
            let mut connection = Self::connect_path(&temporary)?;
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|_| LocalJournalError::Storage)?;
            tx.execute_batch(
                "CREATE TABLE scopes (
                    client TEXT NOT NULL, account TEXT NOT NULL, root BLOB NOT NULL,
                    generation INTEGER NOT NULL, inventory BLOB NOT NULL,
                    PRIMARY KEY (client, account, root)
                );
                CREATE TABLE events (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    client TEXT NOT NULL, account TEXT NOT NULL, root BLOB NOT NULL,
                    payload BLOB NOT NULL
                );
                CREATE INDEX event_scope ON events(client, account, root, id);
                PRAGMA user_version = 1;",
            )
            .map_err(|_| LocalJournalError::Storage)?;
            tx.commit().map_err(|_| LocalJournalError::Storage)?;
            drop(connection);
            File::open(&temporary)
                .and_then(|file| file.sync_all())
                .map_err(|_| LocalJournalError::Storage)?;
            match fs::hard_link(&temporary, path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => return Err(LocalJournalError::Storage),
            }
            File::open(directory)
                .and_then(|dir| dir.sync_all())
                .map_err(|_| LocalJournalError::Storage)?;
            Ok(())
        })();
        let cleanup = fs::remove_file(&temporary).map_err(|_| LocalJournalError::Storage);
        result.and(cleanup)
    }

    fn connect(&self) -> Result<Connection, LocalJournalError> {
        Self::connect_path(&self.path)
    }

    fn connect_path(path: &Path) -> Result<Connection, LocalJournalError> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(|_| LocalJournalError::Storage)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|_| LocalJournalError::Storage)?;
        connection
            .pragma_update(None, "synchronous", "FULL")
            .map_err(|_| LocalJournalError::Storage)?;
        Ok(connection)
    }

    /// Reads the last committed checkpoint for exactly this scope.
    ///
    /// # Errors
    ///
    /// Returns an error on storage failure or invalid persisted data. An
    /// incompatible inventory is never interpreted as an empty inventory.
    pub fn load(&self, scope: &LocalScope) -> Result<Option<LocalCheckpoint>, LocalJournalError> {
        load(&self.connect()?, scope)
    }

    /// Atomically compares and publishes a complete inventory and its events.
    ///
    /// `expected` must be `None` for initialization or the previously read
    /// generation. The caller must independently verify scanning completeness,
    /// root membership and filesystem identity. Device and inode comparisons
    /// detect some root replacements but cannot prevent inode reuse.
    ///
    /// # Errors
    ///
    /// Returns [`LocalJournalError::Stale`] for concurrent writers and
    /// [`LocalJournalError::RootChanged`] for an observed root identity
    /// mismatch. A storage error during commit may have an ambiguous outcome:
    /// reload the checkpoint and queue before retrying.
    pub fn commit(
        &self,
        scope: &LocalScope,
        expected: Option<i64>,
        inventory: &LocalInventory,
    ) -> Result<(), LocalJournalError> {
        let mut connection = self.connect()?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| LocalJournalError::Storage)?;
        let old = load(&tx, scope)?;
        if old.as_ref().map(|checkpoint| checkpoint.generation) != expected {
            return Err(LocalJournalError::Stale);
        }
        if let Some(previous) = &old {
            let old_root = &previous.inventory.entries[&[][..]].metadata;
            let new_root = &inventory.entries[&[][..]].metadata;
            if old_root.device != new_root.device || old_root.inode != new_root.inode {
                return Err(LocalJournalError::RootChanged);
            }
        }
        let generation = expected
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(LocalJournalError::Incompatible)?;
        let entries: Vec<_> = inventory.entries.iter().collect();
        let data = serde_json::to_vec(&entries).map_err(|_| LocalJournalError::Storage)?;
        let events = reconcile_local_inventories(old.as_ref().map(|c| &c.inventory), inventory);
        tx.execute(
            "INSERT INTO scopes (client, account, root, generation, inventory)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(client, account, root) DO UPDATE SET
               generation=excluded.generation, inventory=excluded.inventory",
            params![scope.client, scope.account, scope.root, generation, data],
        )
        .map_err(|_| LocalJournalError::Storage)?;
        for event in events {
            let data = serde_json::to_vec(&event).map_err(|_| LocalJournalError::Storage)?;
            tx.execute(
                "INSERT INTO events (client, account, root, payload) VALUES (?1, ?2, ?3, ?4)",
                params![scope.client, scope.account, scope.root, data],
            )
            .map_err(|_| LocalJournalError::Storage)?;
        }
        tx.commit().map_err(|_| LocalJournalError::Storage)
    }

    /// Reads pending events in delivery order without acknowledging them.
    ///
    /// # Errors
    ///
    /// Returns an error on storage failure or an invalid persisted observation.
    pub fn pending(&self, scope: &LocalScope) -> Result<Vec<LocalDelivery>, LocalJournalError> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(
                "SELECT id, payload FROM events
                 WHERE client=?1 AND account=?2 AND root=?3 ORDER BY id",
            )
            .map_err(|_| LocalJournalError::Storage)?;
        let rows = statement
            .query_map(params![scope.client, scope.account, scope.root], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))
            })
            .map_err(|_| LocalJournalError::Storage)?;
        rows.map(|row| {
            let (id, payload) = row.map_err(|_| LocalJournalError::Storage)?;
            Ok(LocalDelivery {
                id,
                observation: decode_observation(&payload)?,
            })
        })
        .collect()
    }

    /// Acknowledges one delivery only in its exact scope.
    ///
    /// Repeating an acknowledgement returns `false`. A corrupt observation is
    /// never removed by this operation.
    ///
    /// # Errors
    ///
    /// Returns an error for a nonpositive ID, inaccessible storage, or an
    /// invalid persisted observation.
    pub fn acknowledge(&self, scope: &LocalScope, id: i64) -> Result<bool, LocalJournalError> {
        if id <= 0 {
            return Err(LocalJournalError::InvalidScope);
        }
        let mut connection = self.connect()?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| LocalJournalError::Storage)?;
        let payload: Option<Vec<u8>> = tx
            .query_row(
                "SELECT payload FROM events WHERE id=?1 AND client=?2 AND account=?3 AND root=?4",
                params![id, scope.client, scope.account, scope.root],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| LocalJournalError::Storage)?;
        let removed = if let Some(payload) = payload {
            decode_observation(&payload)?;
            tx.execute(
                "DELETE FROM events WHERE id=?1 AND client=?2 AND account=?3 AND root=?4",
                params![id, scope.client, scope.account, scope.root],
            )
            .map_err(|_| LocalJournalError::Storage)?
                == 1
        } else {
            false
        };
        tx.commit().map_err(|_| LocalJournalError::Storage)?;
        Ok(removed)
    }
}

fn load(
    connection: &Connection,
    scope: &LocalScope,
) -> Result<Option<LocalCheckpoint>, LocalJournalError> {
    let data: Option<(i64, Vec<u8>)> = connection
        .query_row(
            "SELECT generation, inventory FROM scopes WHERE client=?1 AND account=?2 AND root=?3",
            params![scope.client, scope.account, scope.root],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|_| LocalJournalError::Storage)?;
    data.map(|(generation, data)| {
        if generation <= 0 {
            return Err(LocalJournalError::Incompatible);
        }
        let entries: Vec<(Vec<u8>, LocalEntry)> =
            serde_json::from_slice(&data).map_err(|_| LocalJournalError::Incompatible)?;
        let inventory =
            LocalInventory::new(entries).map_err(|_| LocalJournalError::Incompatible)?;
        Ok(LocalCheckpoint {
            generation,
            inventory,
        })
    })
    .transpose()
}

fn decode_observation(data: &[u8]) -> Result<LocalObservation, LocalJournalError> {
    let observation: LocalObservation =
        serde_json::from_slice(data).map_err(|_| LocalJournalError::Incompatible)?;
    let valid = match &observation {
        LocalObservation::Initial { path, .. } => valid_path(path),
        LocalObservation::InitialComplete => true,
        LocalObservation::Reevaluate {
            path,
            previous,
            current,
        } => valid_path(path) && (previous.is_some() || current.is_some()),
    };
    if !valid {
        return Err(LocalJournalError::Incompatible);
    }
    Ok(observation)
}

#[cfg(test)]
mod tests;
