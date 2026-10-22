//! Durable account index operations. Credentials remain in auth.
use super::{models::Index, validation};
use crate::{AccountId, AccountSummary, AuthError, EngineError, FolderConfiguration};
use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::PathBuf,
    sync::Mutex,
};

pub(super) const MAX_INDEX: u64 = 1024 * 1024;

pub(crate) struct Accounts {
    pub(super) path: Option<PathBuf>,
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
    pub fn record(&self, client: &str, account: AccountSummary) -> Result<(), AuthError> {
        let _guard = self.lock.lock().map_err(|_| AuthError::Internal)?;
        let mut index = self.read()?;
        let profiles = index.clients.entry(client.into()).or_default();
        profiles.retain(|p| p.id != account.id);
        profiles.push(account);
        profiles.sort_by(|a, b| a.id.0.cmp(&b.id.0));
        self.write(index)
    }
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
    /// Forget app metadata only. Never access or delete configured user folders.
    pub fn forget(&self, client: &str, id: &AccountId) -> Result<(), AuthError> {
        let _guard = self.lock.lock().map_err(|_| AuthError::Internal)?;
        let mut index = self.read()?;
        if let Some(profiles) = index.clients.get_mut(client) {
            profiles.retain(|p| &p.id != id);
        }
        if let Some(folders) = index.folders.get_mut(client) {
            folders.retain(|f| &f.account_id != id);
        }
        self.write(index)
    }
    pub fn folders(&self, client: &str) -> Result<Vec<FolderConfiguration>, AuthError> {
        let _guard = self.lock.lock().map_err(|_| AuthError::Internal)?;
        Ok(self.read()?.folders.remove(client).unwrap_or_default())
    }
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
        let folders = index.folders.entry(client.into()).or_default();
        folders.retain(|f| f.account_id != folder.account_id);
        folders.push(folder);
        self.write(index).map_err(EngineError::Authentication)
    }
    pub fn list(&self, client: &str) -> Result<Vec<AccountSummary>, AuthError> {
        let _guard = self.lock.lock().map_err(|_| AuthError::Internal)?;
        Ok(self.read()?.clients.remove(client).unwrap_or_default())
    }
}
