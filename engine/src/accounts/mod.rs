//! Durable profile discovery. Credentials and authentication remain in auth.
use crate::{AccountId, AccountSummary, AuthError, EngineError, FolderConfiguration};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, DirBuilder, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::PathBuf,
    sync::Mutex,
};
const MAX_INDEX: u64 = 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Index {
    version: u32,
    clients: BTreeMap<String, Vec<AccountSummary>>,
    #[serde(default)]
    folders: BTreeMap<String, Vec<FolderConfiguration>>,
}
impl Default for Index {
    fn default() -> Self {
        Self {
            version: 2,
            clients: BTreeMap::new(),
            folders: BTreeMap::new(),
        }
    }
}

pub(crate) struct Accounts {
    path: Option<PathBuf>,
    lock: Mutex<()>,
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
        Self::validate(&index)?;
        Ok(index)
    }
    fn validate(index: &Index) -> Result<(), AuthError> {
        if index.version != 2 {
            return Err(AuthError::AccountStorage);
        }
        for (client, profiles) in &index.clients {
            crate::auth::OAuthConfig {
                client_id: client.clone(),
                client_secret: None,
            }
            .validate()
            .map_err(|_| AuthError::AccountStorage)?;
            let mut ids = HashSet::new();
            for account in profiles {
                if account.provider_subject.is_empty()
                    || account.provider_subject.len() > 255
                    || account.provider_subject.chars().any(char::is_control)
                    || account.id != AccountId::from_subject(&account.provider_subject)
                    || !ids.insert(&account.id)
                    || account
                        .email
                        .as_ref()
                        .is_some_and(|e| e.len() > 1024 || e.chars().any(char::is_control))
                {
                    return Err(AuthError::AccountStorage);
                }
            }
        }
        for (client, folders) in &index.folders {
            let mut ids = HashSet::new();
            for folder in folders {
                if !index
                    .clients
                    .get(client)
                    .is_some_and(|profiles| profiles.iter().any(|p| p.id == folder.account_id))
                    || !folder.remote.valid()
                    || !folder.local.is_absolute()
                    || !folder.local_display.is_absolute()
                    || !ids.insert(&folder.account_id)
                {
                    return Err(AuthError::AccountStorage);
                }
            }
        }
        Ok(())
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
        Self::validate(&index)?;
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

#[cfg(test)]
mod tests;
