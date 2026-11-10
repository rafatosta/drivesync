//! Versioned private JSON storage with atomic publication and exclusive access.
use super::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::PathBuf,
};
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Record {
    pub account: Account,
    pub key: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Intent {
    pub before: Option<Record>,
    pub after: Option<Record>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Data {
    pub version: u32,
    pub application: String,
    pub records: BTreeMap<AccountId, Record>,
    pub intent: Option<Intent>,
}
/// The lock is retained until the last owner drops this store.
pub(super) struct Storage {
    directory: PathBuf,
    _lock: File,
    pub data: Data,
}
pub(super) fn storage_error() -> AccountsFailure {
    AccountsFailure::new(AccountsError::StorageUnavailable)
}
impl Storage {
    pub fn open(directory: PathBuf, application: String) -> AccountsResult<Self> {
        if !directory.is_absolute() || application.trim().is_empty() {
            return Err(AccountsFailure::new(AccountsError::InvalidRequest));
        }
        if !directory.exists() {
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&directory)
                .map_err(|_| storage_error())?;
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| storage_error())?;
        }
        let meta = std::fs::symlink_metadata(&directory).map_err(|_| storage_error())?;
        if !meta.is_dir() || meta.permissions().mode() & 0o077 != 0 {
            return Err(storage_error());
        }
        let lock = private_file(&directory.join("accounts.lock"), true)?;
        lock.try_lock().map_err(|_| storage_error())?;
        let path = directory.join("accounts.json");
        let exists = path.try_exists().map_err(|_| storage_error())?;
        let data = if exists {
            let meta = path.symlink_metadata().map_err(|_| storage_error())?;
            if !meta.is_file() || meta.permissions().mode() & 0o077 != 0 {
                return Err(storage_error());
            }
            let mut bytes = Vec::new();
            private_file(&path, false)?
                .take(16 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| storage_error())?;
            let data: Data = serde_json::from_slice(&bytes).map_err(|_| storage_error())?;
            if data.version != 1 || data.application != application {
                return Err(storage_error());
            }
            let mut identities = std::collections::BTreeSet::new();
            for (id, r) in &data.records {
                if id != &r.account.id
                    || id.0.is_empty()
                    || r.key.is_empty()
                    || r.account.provider_user_id.trim().is_empty()
                    || !identities.insert((&r.account.provider, &r.account.provider_user_id))
                {
                    return Err(storage_error());
                }
            }
            if let Some(intent) = &data.intent {
                let visible = intent
                    .before
                    .as_ref()
                    .or(intent.after.as_ref())
                    .ok_or_else(storage_error)?;
                let same_identity = |r: &Record| {
                    r.account.id == visible.account.id
                        && r.account.provider == visible.account.provider
                        && r.account.provider_user_id == visible.account.provider_user_id
                };
                if !intent
                    .before
                    .iter()
                    .chain(intent.after.iter())
                    .all(same_identity)
                    || !data.records.get(&visible.account.id).is_some_and(|r| {
                        r.key == visible.key && r.account.state == AccountState::Unavailable
                    })
                    || intent
                        .before
                        .as_ref()
                        .zip(intent.after.as_ref())
                        .is_some_and(|(a, b)| a.key == b.key)
                {
                    return Err(storage_error());
                }
            }
            data
        } else {
            Data {
                version: 1,
                application,
                records: BTreeMap::new(),
                intent: None,
            }
        };
        let mut store = Self {
            directory,
            _lock: lock,
            data,
        };
        if !exists {
            store.save(store.data.clone())?;
        }
        Ok(store)
    }
    /// Publish only after the complete replacement is durable. Never reset on error.
    pub fn save(&mut self, data: Data) -> AccountsResult<()> {
        let bytes = serde_json::to_vec(&data).map_err(|_| storage_error())?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(storage_error());
        }
        let path = self.directory.join("accounts.next");
        if path.symlink_metadata().is_ok_and(|m| !m.is_file()) {
            return Err(storage_error());
        }
        let mut file = private_file(&path, true)?;
        file.set_len(0).map_err(|_| storage_error())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| storage_error())?;
        std::fs::rename(path, self.directory.join("accounts.json")).map_err(|_| storage_error())?;
        // A failed directory sync has an uncertain durable outcome; keep memory aligned
        // with the published file and return an error without unsafe compensation.
        self.data = data;
        File::open(&self.directory)
            .and_then(|f| f.sync_all())
            .map_err(|_| storage_error())
    }
}

/// Validate the opened descriptor before any truncation; path prechecks alone
/// would allow hard-link writes and symlink replacement races.
fn private_file(path: &std::path::Path, create: bool) -> AccountsResult<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(create)
        .create(create)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| storage_error())?;
    let meta = file.metadata().map_err(|_| storage_error())?;
    if !meta.is_file() || meta.nlink() != 1 || meta.permissions().mode() & 0o077 != 0 {
        return Err(storage_error());
    }
    Ok(file)
}
