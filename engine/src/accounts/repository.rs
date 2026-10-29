//! Durable repository for non-secret account and folder settings.
//!
//! Operations validate the index before use and serialize changes within this
//! repository instance. Credential storage and remote authentication stay in
//! the auth module; user file contents are never managed here.
use super::{models::Index, validation};
use crate::{AccountId, AccountSummary, AuthError, EngineError, FolderConfiguration};
use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::PathBuf,
    sync::Mutex,
};

/// Maximum accepted size in bytes of serialized account-index JSON (1 MiB).
pub(super) const MAX_INDEX: u64 = 1024 * 1024;

/// Profile and folder store backed by a JSON file.
///
/// The mutex serializes operations through this instance, not across
/// independent processes opening the same file.
pub(crate) struct Accounts {
    /// Location of the index, or `None` without a usable data/home directory.
    pub(super) path: Option<PathBuf>,
    /// Protects in-process read/modify/write operations.
    pub(super) lock: Mutex<()>,
}
impl Default for Accounts {
    fn default() -> Self {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .filter(|p| p.is_absolute())
                    .map(|p| p.join(".local/share"))
            });
        Self {
            path: base.map(|p| p.join("drivesync/accounts.json")),
            lock: Mutex::new(()),
        }
    }
}
impl Accounts {
    /// Supplies an isolated store to engine unit tests without global environment changes.
    #[cfg(test)]
    pub(crate) fn for_test(path: PathBuf) -> Self {
        Self {
            path: Some(path),
            lock: Mutex::new(()),
        }
    }

    /// Reads and validates the index; an absent file means an empty index.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::AccountStorage`] for missing storage location,
    /// I/O failures, oversized content or invalid saved data. Corrupt data
    /// is never silently recreated.
    fn read(&self) -> Result<Index, AuthError> {
        let path = self.path.as_ref().ok_or(AuthError::AccountStorage)?;
        let file = match File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Index::default()),
            Err(_) => return Err(AuthError::AccountStorage),
        };
        let mut data = Vec::new();
        file.take(MAX_INDEX + 1)
            .read_to_end(&mut data)
            .map_err(|_| AuthError::AccountStorage)?;
        if data.len() as u64 > MAX_INDEX {
            return Err(AuthError::AccountStorage);
        }
        let index: Index = serde_json::from_slice(&data).map_err(|_| AuthError::AccountStorage)?;
        validation::validate(&index)?;
        Ok(index)
    }
    /// Inserts or updates a saved profile for an OAuth client.
    ///
    /// `client` identifies the credential namespace; `account` contains the
    /// non-secret identity. Existing profiles with the same ID are replaced,
    /// then sorted. Other accounts and folder settings are preserved.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError`] if locking, validation or persistence fails.
    pub fn record(&self, client: &str, account: AccountSummary) -> Result<(), AuthError> {
        let _guard = self.lock.lock().map_err(|_| AuthError::Internal)?;
        let mut index = self.read()?;
        let profiles = index.clients.entry(client.into()).or_default();
        profiles.retain(|p| p.id != account.id);
        profiles.push(account);
        profiles.sort_by(|a, b| a.id.0.cmp(&b.id.0));
        self.write(index)
    }
    /// Validates and replaces the index using a temporary file and rename.
    ///
    /// The temporary file and parent directory are synced for durability.
    /// If directory sync fails after rename, an error may be returned even
    /// though the new index is already visible on disk.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::AccountStorage`] for invalid data or write errors.
    fn write(&self, index: Index) -> Result<(), AuthError> {
        validation::validate(&index)?;
        let data = serde_json::to_vec(&index).map_err(|_| AuthError::AccountStorage)?;
        if data.len() as u64 > MAX_INDEX {
            return Err(AuthError::AccountStorage);
        }
        let path = self.path.as_ref().ok_or(AuthError::AccountStorage)?;
        let directory = path.parent().ok_or(AuthError::AccountStorage)?;
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)
            .map_err(|_| AuthError::AccountStorage)?;
        let temporary = directory.join(format!(
            ".accounts-{}.tmp",
            oauth2::CsrfToken::new_random().secret()
        ));
        let result = (|| -> std::io::Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)?;
            file.write_all(&data)?;
            file.sync_all()?;
            fs::rename(&temporary, path)?;
            File::open(directory)?.sync_all()
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.map_err(|_| AuthError::AccountStorage)
    }
    /// Forgets a profile and its folder settings in the specified OAuth client.
    ///
    /// `id` identifies the account to forget. This only changes app metadata:
    /// it does not delete local files, access the credential vault or revoke
    /// Google access. Removing an already absent account is supported.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError`] if locking, reading or writing the index fails.
    pub fn forget(&self, client: &str, id: &AccountId) -> Result<(), AuthError> {
        let _guard = self.lock.lock().map_err(|_| AuthError::Internal)?;
        let mut index = self.read()?;
        if let Some(profiles) = index.clients.get_mut(client) {
            profiles.retain(|p| &p.id != id);
        }
        if let Some(folders) = index.folders.get_mut(client) {
            folders.retain(|f| &f.account_id != id);
        }
        if let Some(descriptions) = index.descriptions.get_mut(client) {
            descriptions.remove(&id.0);
        }
        self.write(index)
    }
    /// Returns saved remote/local folder pairs for the given OAuth client.
    ///
    /// The result is cached configuration, not proof of current access.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError`] for locking, read or validation errors.
    pub fn folders(&self, client: &str) -> Result<Vec<FolderConfiguration>, AuthError> {
        let _guard = self.lock.lock().map_err(|_| AuthError::Internal)?;
        Ok(self.read()?.folders.remove(client).unwrap_or_default())
    }
    /// Opens availability metadata independently of the selected volume.
    pub(crate) fn roots(&self) -> Result<crate::state::Roots, EngineError> {
        let path = self
            .path
            .as_ref()
            .ok_or(EngineError::StateStorage)?
            .with_file_name("roots.sqlite3");
        crate::state::Roots::open(&path)
    }

    /// Configures one remote/local folder pair for an existing account.
    ///
    /// `client` scopes the account namespace. `folder` supplies the remote
    /// selection and local directory. The local path is canonicalized and
    /// checked against other accounts' roots, including nested paths and
    /// filesystem/portal aliases. Reconfiguration replaces the prior pair.
    /// No user files are created, transferred or deleted.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::Authentication`] for missing profiles or index
    /// failures, [`EngineError::InvalidFolder`] for an invalid local path,
    /// [`EngineError::LocalFolderIdentity`] when its host identity cannot be
    /// established, or [`EngineError::FolderOverlap`] for overlapping roots.
    pub fn configure(
        &self,
        client: &str,
        mut folder: FolderConfiguration,
    ) -> Result<(), EngineError> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| EngineError::Authentication(AuthError::Internal))?;
        let mut index = self.read().map_err(EngineError::Authentication)?;
        if !index
            .clients
            .get(client)
            .is_some_and(|p| p.iter().any(|a| a.id == folder.account_id))
        {
            return Err(EngineError::Authentication(
                AuthError::ReauthorizationRequired,
            ));
        }
        folder.local =
            std::fs::canonicalize(&folder.local).map_err(|_| EngineError::InvalidFolder)?;
        folder.local_display = crate::configuration::local_origin(&folder.local)?;
        for (other_client, folders) in &index.folders {
            for other in folders {
                if other_client == client && other.account_id == folder.account_id {
                    continue;
                }
                // Resolve existing aliases again before comparing; missing roots are not silently ignored.
                let other_path =
                    std::fs::canonicalize(&other.local).map_err(|_| EngineError::InvalidFolder)?;
                let origin = crate::configuration::local_origin(&other_path)?;
                if crate::configuration::overlap(&folder.local_display, &origin) {
                    return Err(EngineError::FolderOverlap);
                }
            }
        }
        let roots = self.roots()?;
        let _association_lock = roots.lock(client, &folder)?;
        let existing_pair = index.folders.get(client).is_some_and(|folders| {
            folders.iter().any(|saved| {
                saved.account_id == folder.account_id
                    && saved.remote.id == folder.remote.id
                    && saved.local == folder.local
            })
        });
        if existing_pair || roots.marker(client, &folder)?.is_some() {
            crate::configuration::identity::verify(&folder, client)?;
        } else {
            crate::configuration::identity::ensure(&folder, client)?;
        }
        let marker = crate::configuration::identity::verify(&folder, client)?;
        self.roots()?.bind(client, &folder, &marker)?;
        let folders = index.folders.entry(client.into()).or_default();
        folders.retain(|f| f.account_id != folder.account_id);
        folders.push(folder);
        self.write(index).map_err(EngineError::Authentication)
    }
    /// Reads a local label for an existing account without verifying credentials.
    ///
    /// # Errors
    /// Returns storage failures or reauthorization-required for an unknown account.
    pub fn description(&self, client: &str, id: &AccountId) -> Result<Option<String>, AuthError> {
        let _guard = self.lock.lock().map_err(|_| AuthError::Internal)?;
        let index = self.read()?;
        if !index
            .clients
            .get(client)
            .is_some_and(|profiles| profiles.iter().any(|p| &p.id == id))
        {
            return Err(AuthError::ReauthorizationRequired);
        }
        Ok(index
            .descriptions
            .get(client)
            .and_then(|labels| labels.get(&id.0))
            .cloned())
    }

    /// Atomically saves a trimmed local label, or clears it for blank input.
    ///
    /// The existing lock and validated writer preserve profiles, folders and
    /// other clients' labels. Provider identity refresh never overwrites labels.
    ///
    /// # Errors
    /// Returns invalid-description for control characters or more than 128
    /// Unicode characters, or an authentication/storage error on lookup/write.
    pub fn set_description(
        &self,
        client: &str,
        id: &AccountId,
        value: &str,
    ) -> Result<Option<String>, EngineError> {
        if value.chars().any(char::is_control) || value.trim().chars().count() > 128 {
            return Err(EngineError::InvalidAccountDescription);
        }
        let value = value.trim();
        let _guard = self
            .lock
            .lock()
            .map_err(|_| EngineError::Authentication(AuthError::Internal))?;
        let mut index = self.read().map_err(EngineError::Authentication)?;
        if !index
            .clients
            .get(client)
            .is_some_and(|profiles| profiles.iter().any(|p| &p.id == id))
        {
            return Err(EngineError::Authentication(
                AuthError::ReauthorizationRequired,
            ));
        }
        let labels = index.descriptions.entry(client.into()).or_default();
        let result = if value.is_empty() {
            labels.remove(&id.0);
            None
        } else {
            labels.insert(id.0.clone(), value.into());
            Some(value.into())
        };
        self.write(index).map_err(EngineError::Authentication)?;
        Ok(result)
    }

    /// Lists saved profiles belonging to one OAuth client.
    ///
    /// Saved metadata does not imply that a session is authenticated.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError`] for locking, read or validation errors.
    pub fn list(&self, client: &str) -> Result<Vec<AccountSummary>, AuthError> {
        let _guard = self.lock.lock().map_err(|_| AuthError::Internal)?;
        Ok(self.read()?.clients.remove(client).unwrap_or_default())
    }
}
