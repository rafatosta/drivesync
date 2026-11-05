//! Private SQLite publication of bounded scope documents and their work queues.
use super::models::*;
use crate::{LocalBatch, LocalScope};
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

const APPLICATION_ID: i64 = 0x44534c53;

/// Separate current-format state; methods never open observed files or contact Google.
pub struct LocalStateStore {
    path: PathBuf,
    portable: Option<LocalScope>,
}
impl LocalStateStore {
    pub(crate) fn portable(
        destination: &Path,
        scope: &LocalScope,
        remote: &str,
    ) -> Result<Self, LocalStateError> {
        let mut store = Self::open(&destination.join(".drivesync"))?;
        let key = LocalScope::new(remote, scope.account(), Path::new("/drivesync"))
            .map_err(|_| LocalStateError::Invalid)?;
        store.portable = Some(key);
        if store.load(scope)?.is_none() {
            match store.mutate(scope, Some(None), |_| Ok(())) {
                Ok(_) | Err(LocalStateError::Stale) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(store)
    }

    fn scope<'a>(
        &'a self,
        connection: &Connection,
        scope: &'a LocalScope,
    ) -> Result<&'a LocalScope, LocalStateError> {
        let Some(key) = &self.portable else {
            return Ok(scope);
        };
        let foreign: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM state_scopes WHERE client != ?1 OR account != ?2 OR root != ?3)",
            params![key.client(), key.account(), key.root().as_os_str().as_bytes()],
            |row| row.get(0),
        ).map_err(storage)?;
        if foreign {
            return Err(LocalStateError::Invalid);
        }
        Ok(key)
    }

    /// Opens/creates a private database, preserving incompatible existing data.
    /// # Errors
    /// Returns storage or incompatible errors for unsafe access, format or schema.
    pub fn open(directory: &Path) -> Result<Self, LocalStateError> {
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)
            .map_err(storage)?;
        let metadata = fs::symlink_metadata(directory).map_err(storage)?;
        if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
            return Err(LocalStateError::Storage);
        }
        // Keep the storage-root exclusion effective even for relative/lexical aliases.
        let directory = fs::canonicalize(directory).map_err(storage)?;
        let path = directory.join("local-state.sqlite3");
        match fs::symlink_metadata(&path) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Self::initialize(&directory, &path)?
            }
            Err(_) => return Err(LocalStateError::Storage),
        }
        let metadata = fs::symlink_metadata(&path).map_err(storage)?;
        if !metadata.is_file() || metadata.permissions().mode() & 0o077 != 0 {
            return Err(LocalStateError::Storage);
        }
        let store = Self {
            path,
            portable: None,
        };
        let connection = store.connect()?;
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(storage)?;
        let app: i64 = connection
            .pragma_query_value(None, "application_id", |r| r.get(0))
            .map_err(storage)?;
        if version != 2 || app != APPLICATION_ID {
            return Err(LocalStateError::Incompatible);
        }
        connection
            .prepare("SELECT client, account, root, generation, payload FROM state_scopes LIMIT 0")
            .map_err(|_| LocalStateError::Incompatible)?;
        Ok(store)
    }

    fn initialize(directory: &Path, path: &Path) -> Result<(), LocalStateError> {
        let temporary = directory.join(format!(
            ".local-state-{}.tmp",
            oauth2::CsrfToken::new_random().secret()
        ));
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(storage)?;
        let result = (|| {
            let mut connection = connect_path(&temporary)?;
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(storage)?;
            tx.execute_batch(
                "CREATE TABLE state_scopes (
                client TEXT NOT NULL, account TEXT NOT NULL, root BLOB NOT NULL,
                generation INTEGER NOT NULL CHECK(generation > 0), payload BLOB NOT NULL,
                PRIMARY KEY(client, account, root));
                PRAGMA user_version = 2; PRAGMA application_id = 1146309715;",
            )
            .map_err(storage)?;
            tx.commit().map_err(storage)?;
            drop(connection);
            File::open(&temporary)
                .and_then(|f| f.sync_all())
                .map_err(storage)?;
            match fs::hard_link(&temporary, path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => return Err(LocalStateError::Storage),
            }
            File::open(directory)
                .and_then(|f| f.sync_all())
                .map_err(storage)
        })();
        let cleanup = fs::remove_file(&temporary).map_err(storage);
        result.and(cleanup)
    }
    fn connect(&self) -> Result<Connection, LocalStateError> {
        connect_path(&self.path)
    }

    /// Reads exactly one scope, rejecting invalid payloads rather than returning empty state.
    pub fn load(
        &self,
        scope: &LocalScope,
    ) -> Result<Option<LocalStateCheckpoint>, LocalStateError> {
        let connection = self.connect()?;
        load(&connection, self.scope(&connection, scope)?)
            .map(|value| value.map(|(generation, document)| checkpoint(generation, document)))
    }

    pub(crate) fn initial_download_complete(
        &self,
        scope: &LocalScope,
    ) -> Result<bool, LocalStateError> {
        let connection = self.connect()?;
        Ok(load(&connection, self.scope(&connection, scope)?)?
            .is_some_and(|(_, document)| document.initial_download_complete))
    }

    pub(crate) fn finish_initial_download(
        &self,
        scope: &LocalScope,
        root_item: &ItemId,
    ) -> Result<(), LocalStateError> {
        self.mutate(scope, None, |document| {
            if document.root.as_ref() != Some(root_item) {
                return Err(LocalStateError::Stale);
            }
            document.initial_download_complete = true;
            Ok(())
        })
        .map(|_| ())
    }

    /// Registers an explicitly verified item. Raw events must never call this method.
    /// `confirmed` is true only for a verified last-synchronized baseline.
    /// # Errors
    /// Rejects stale generations, duplicate bindings, invalid trees and limits atomically.
    pub fn register_verified_item(
        &self,
        scope: &LocalScope,
        expected: Option<i64>,
        remote_id: Option<String>,
        snapshot: ItemSnapshot,
        confirmed: bool,
    ) -> Result<(ItemId, i64), LocalStateError> {
        self.mutate(scope, Some(expected), |document| {
            let id = ItemId::new();
            if snapshot.parent.is_none() {
                if document.root.is_some() {
                    return Err(LocalStateError::Invalid);
                }
                document.root = Some(id.clone());
            }
            let state = ItemState::Present(snapshot);
            document.items.push(TrackedItem {
                id: id.clone(),
                remote_id,
                confirmed: confirmed.then_some(state.clone()),
                observed: state,
            });
            Ok(id)
        })
    }

    /// Attaches a verified Drive ID after creating a remote object, keeping the local ID.
    /// Existing different bindings are rejected; duplicate Drive IDs are never shared.
    pub fn bind_remote_id(
        &self,
        scope: &LocalScope,
        expected: i64,
        id: &ItemId,
        remote_id: String,
    ) -> Result<i64, LocalStateError> {
        self.mutate(scope, Some(Some(expected)), |document| {
            let item = document
                .items
                .iter_mut()
                .find(|item| &item.id == id)
                .ok_or(LocalStateError::Missing)?;
            if item.remote_id.as_ref().is_some_and(|old| old != &remote_id) {
                return Err(LocalStateError::Invalid);
            }
            item.remote_id = Some(remote_id);
            Ok(())
        })
        .map(|(_, generation)| generation)
    }

    /// Records verified current states, preserving identity and all confirmed baselines.
    /// Multiple updates permit atomic subtree absence without invalid intermediate trees.
    pub fn record_observed(
        &self,
        scope: &LocalScope,
        expected: i64,
        updates: Vec<(ItemId, ItemState)>,
    ) -> Result<i64, LocalStateError> {
        self.mutate(scope, Some(Some(expected)), |document| {
            let mut seen = std::collections::HashSet::new();
            for (id, state) in updates {
                if !seen.insert(id.clone()) {
                    return Err(LocalStateError::Invalid);
                }
                document
                    .items
                    .iter_mut()
                    .find(|item| item.id == id)
                    .ok_or(LocalStateError::Missing)?
                    .observed = state;
            }
            Ok(())
        })
        .map(|(_, generation)| generation)
    }

    /// Marks current states confirmed only after the caller has verified external results.
    /// This method executes no transfer and does not acknowledge investigation work.
    pub fn confirm_observed(
        &self,
        scope: &LocalScope,
        expected: i64,
        ids: &[ItemId],
    ) -> Result<i64, LocalStateError> {
        self.mutate(scope, Some(Some(expected)), |document| {
            for id in ids {
                let item = document
                    .items
                    .iter_mut()
                    .find(|item| &item.id == id)
                    .ok_or(LocalStateError::Missing)?;
                item.confirmed = Some(item.observed.clone());
            }
            Ok(())
        })
        .map(|(_, generation)| generation)
    }

    /// Persists hints before delivery. Never mutates identity records or infers operations.
    /// A commit error can be ambiguous: reload state before retrying a decision.
    pub fn ingest(
        &self,
        batch: &LocalBatch,
        connectivity: LocalConnectivity,
    ) -> Result<i64, LocalStateError> {
        // Never observe writes to our own state as new user work indefinitely.
        if self.portable.is_none() && self.path.starts_with(batch.scope.root()) {
            return Err(LocalStateError::Invalid);
        }
        self.mutate(&batch.scope, None, |document| {
            document.ingest(batch, connectivity)
        })
        .map(|(_, generation)| generation)
    }

    /// Completes only the investigated revision at the expected scope generation.
    /// This is not permission to synchronize. Concurrent hints force a reload.
    pub fn complete_investigation(
        &self,
        scope: &LocalScope,
        expected: i64,
        id: i64,
        revision: i64,
    ) -> Result<i64, LocalStateError> {
        self.mutate(scope, Some(Some(expected)), |document| {
            let index = document
                .pending
                .iter()
                .position(|w| w.id == id)
                .ok_or(LocalStateError::Missing)?;
            if document.pending[index].revision != revision {
                return Err(LocalStateError::Stale);
            }
            document.pending.remove(index);
            Ok(())
        })
        .map(|(_, generation)| generation)
    }

    /// Derives current or confirmed paths without depending on stored path identity.
    pub fn item_paths(
        &self,
        scope: &LocalScope,
        confirmed: bool,
    ) -> Result<std::collections::BTreeMap<Vec<u8>, ItemId>, LocalStateError> {
        let connection = self.connect()?;
        let Some((_, document)) = load(&connection, self.scope(&connection, scope)?)? else {
            return Ok(Default::default());
        };
        document.paths(confirmed)
    }

    /// Serializes writers, verifies consumer generation and publishes all changes together.
    /// `None` outer expected is a producer merge; Some(inner) is optimistic consumer CAS.
    fn mutate<T>(
        &self,
        scope: &LocalScope,
        expected: Option<Option<i64>>,
        change: impl FnOnce(&mut Document) -> Result<T, LocalStateError>,
    ) -> Result<(T, i64), LocalStateError> {
        let mut connection = self.connect()?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let scope = self.scope(&tx, scope)?;
        let old = load(&tx, scope)?;
        let generation = old.as_ref().map(|(g, _)| *g);
        if expected.is_some_and(|expected| expected != generation) {
            return Err(LocalStateError::Stale);
        }
        let mut document = old.map(|(_, d)| d).unwrap_or_default();
        let result = change(&mut document)?;
        document.validate()?;
        let payload = serde_json::to_vec(&document).map_err(storage)?;
        if payload.len() > MAX_PAYLOAD {
            return Err(LocalStateError::Limit);
        }
        let next = generation
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(LocalStateError::Limit)?;
        tx.execute(
            "INSERT INTO state_scopes(client,account,root,generation,payload)
            VALUES(?1,?2,?3,?4,?5) ON CONFLICT(client,account,root) DO UPDATE SET
            generation=excluded.generation,payload=excluded.payload",
            params![
                scope.client(),
                scope.account(),
                scope.root().as_os_str().as_bytes(),
                next,
                payload
            ],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok((result, next))
    }
}

fn storage(_: impl std::fmt::Debug) -> LocalStateError {
    LocalStateError::Storage
}
fn connect_path(path: &Path) -> Result<Connection, LocalStateError> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .map_err(storage)?;
    connection
        .busy_timeout(Duration::from_secs(2))
        .map_err(storage)?;
    connection
        .pragma_update(None, "synchronous", "FULL")
        .map_err(storage)?;
    Ok(connection)
}
fn load(
    connection: &Connection,
    scope: &LocalScope,
) -> Result<Option<(i64, Document)>, LocalStateError> {
    let data: Option<(i64, Option<Vec<u8>>)> = connection
        .query_row(
            "SELECT generation,
        CASE WHEN length(payload) <= ?4 THEN payload END FROM state_scopes
        WHERE client=?1 AND account=?2 AND root=?3",
            params![
                scope.client(),
                scope.account(),
                scope.root().as_os_str().as_bytes(),
                MAX_PAYLOAD as i64
            ],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(storage)?;
    data.map(|(generation, payload)| {
        let payload = payload.ok_or(LocalStateError::Incompatible)?;
        if generation < 1 || payload.len() > MAX_PAYLOAD {
            return Err(LocalStateError::Incompatible);
        }
        let document: Document =
            serde_json::from_slice(&payload).map_err(|_| LocalStateError::Incompatible)?;
        document
            .validate()
            .map_err(|_| LocalStateError::Incompatible)?;
        Ok((generation, document))
    })
    .transpose()
}
fn checkpoint(generation: i64, document: Document) -> LocalStateCheckpoint {
    let mut pending = document.pending;
    pending.sort_by_key(|p| (p.priority, p.id));
    LocalStateCheckpoint {
        generation,
        items: document.items,
        pending,
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod portable_tests {
    use super::*;

    #[test]
    fn portable_checkpoint_survives_relocation_and_rejects_other_bindings() {
        let suffix = oauth2::CsrfToken::new_random();
        let base = std::env::temp_dir().join(format!("drivesync-portable-{}", suffix.secret()));
        let original = base.join("original");
        let moved = base.join("moved");
        let before = LocalScope::new("old-client", "same-account", &original).unwrap();
        let mut state = LocalStateStore::open(&original.join(".drivesync")).unwrap();
        state.portable =
            Some(LocalScope::new("remote-root", "same-account", Path::new("/drivesync")).unwrap());
        let (root, _) = state
            .register_verified_item(
                &before,
                None,
                Some("remote-root".into()),
                ItemSnapshot {
                    parent: None,
                    name: vec![],
                    kind: crate::LocalItemKind::Directory,
                    content_version: None,
                    remote_version: None,
                },
                true,
            )
            .unwrap();
        state.finish_initial_download(&before, &root).unwrap();
        drop(state);
        fs::rename(&original, &moved).unwrap();

        let after = LocalScope::new("new-client", "same-account", &moved).unwrap();
        let state = LocalStateStore::portable(&moved, &after, "remote-root").unwrap();
        assert!(state.initial_download_complete(&after).unwrap());
        assert_eq!(state.load(&after).unwrap().unwrap().items[0].id, root);
        assert!(matches!(
            LocalStateStore::portable(&moved, &after, "other-root"),
            Err(LocalStateError::Invalid)
        ));
        let other = LocalScope::new("new-client", "other-account", &moved).unwrap();
        assert!(matches!(
            LocalStateStore::portable(&moved, &other, "remote-root"),
            Err(LocalStateError::Invalid)
        ));
        fs::remove_dir_all(base).unwrap();
    }
}
