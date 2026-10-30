//! Portable local-root identity that survives loss of application data.
//!
//! The marker binds a local root to one OAuth client, stable account subject and
//! remote Drive folder. It contains no credentials and is excluded from uploads.
use super::FolderConfiguration;
use crate::{AccountId, EngineError};
use rustix::fs::{self, AtFlags, Mode, OFlags};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Read, Write},
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
};

const DIRECTORY: &str = ".drivesync";
const IDENTITY: &str = "identity.json";
const STATE: &str = "state.sqlite3";
const MAX_MARKER: usize = 16 * 1024;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Marker {
    format: u32,
    id: String,
    client: String,
    account: AccountId,
    remote: String,
}

/// Creates a marker for a new association or verifies an existing exact match.
///
/// Existing invalid or differently scoped markers fail closed and are never
/// replaced. The file is created through a directory descriptor, atomically
/// installed without replacement and synchronized before success is returned.
///
/// # Errors
/// Returns [`EngineError::FolderIdentityMismatch`] for another or invalid
/// association and [`EngineError::FolderIdentityStorage`] for unsafe I/O.
pub(crate) fn ensure(folder: &FolderConfiguration, client: &str) -> Result<(), EngineError> {
    if client.is_empty() || folder.account_id.0.is_empty() || folder.remote.id.is_empty() {
        return Err(EngineError::FolderIdentityStorage);
    }
    let root = open_root(&folder.local)?;
    let metadata = root
        .metadata()
        .map_err(|_| EngineError::FolderIdentityStorage)?;
    if !metadata.is_dir() {
        return Err(EngineError::FolderIdentityStorage);
    }
    let directory = open_state_dir(&root)?;
    if let Some(marker) = read_marker(&directory)? {
        if marker.format == 1
            && !marker.id.is_empty()
            && marker.client == client
            && marker.account == folder.account_id
            && marker.remote == folder.remote.id
        {
            return Ok(());
        }
        return Err(EngineError::FolderIdentityMismatch);
    }

    let marker = Marker {
        format: 1,
        id: oauth2::CsrfToken::new_random().secret().to_owned(),
        client: client.to_owned(),
        account: folder.account_id.clone(),
        remote: folder.remote.id.clone(),
    };
    let bytes = serde_json::to_vec(&marker).map_err(|_| EngineError::FolderIdentityStorage)?;
    let temporary = format!("identity-{}.tmp", oauth2::CsrfToken::new_random().secret());
    let mut file = fs::openat(
        &directory,
        temporary.as_str(),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map(File::from)
    .map_err(|_| EngineError::FolderIdentityStorage)?;
    let result = (|| {
        file.write_all(&bytes)
            .map_err(|_| EngineError::FolderIdentityStorage)?;
        file.sync_all()
            .map_err(|_| EngineError::FolderIdentityStorage)?;
        match fs::renameat_with(
            &directory,
            temporary.as_str(),
            &directory,
            IDENTITY,
            fs::RenameFlags::NOREPLACE,
        ) {
            Ok(()) => {
                directory
                    .sync_all()
                    .map_err(|_| EngineError::FolderIdentityStorage)?;
                Ok(())
            }
            Err(rustix::io::Errno::EXIST) => match read_marker(&directory)? {
                Some(existing)
                    if existing.format == 1
                        && !existing.id.is_empty()
                        && existing.client == client
                        && existing.account == folder.account_id
                        && existing.remote == folder.remote.id =>
                {
                    Ok(())
                }
                _ => Err(EngineError::FolderIdentityMismatch),
            },
            Err(_) => Err(EngineError::FolderIdentityStorage),
        }
    })();
    let _ = fs::unlinkat(&directory, temporary.as_str(), AtFlags::empty());
    result
}

/// Reads an existing association without creating metadata or probing writes.
/// Missing, corrupt or mismatched markers never authorize a new association.
pub(crate) fn verify(folder: &FolderConfiguration, client: &str) -> Result<String, EngineError> {
    let root = open_root(&folder.local)
        .map_err(|_| EngineError::RootSuspended(crate::RootSuspension::LocalUnavailable))?;
    verify_at(&root, folder, client)
}

/// Verifies the marker through the same descriptor used by the active cycle.
pub(crate) fn verify_at(
    root: &File,
    folder: &FolderConfiguration,
    client: &str,
) -> Result<String, EngineError> {
    let directory = fs::openat(
        root,
        DIRECTORY,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| EngineError::RootSuspended(crate::RootSuspension::IdentityUnverified))?;
    let marker = read_marker(&directory)
        .map_err(|_| EngineError::RootSuspended(crate::RootSuspension::IdentityUnverified))?
        .ok_or(EngineError::RootSuspended(
            crate::RootSuspension::IdentityUnverified,
        ))?;
    if marker.format != 1
        || marker.id.is_empty()
        || marker.client != client
        || marker.account != folder.account_id
        || marker.remote != folder.remote.id
    {
        return Err(EngineError::RootSuspended(
            crate::RootSuspension::IdentityUnverified,
        ));
    }
    Ok(marker.id)
}

/// Returns the operation-journal path carried with this root.
///
/// Verifies the existing root marker before returning the location. The
/// journal contains no credentials and is stored in a file ignored by uploads.
pub(crate) fn state_path(
    folder: &FolderConfiguration,
    client: &str,
) -> Result<PathBuf, EngineError> {
    metadata_path(folder, client, STATE)
}

/// Returns the per-root remote change checkpoint path after validating its root.
///
/// # Errors
/// Returns an error if the existing root identity cannot be verified safely.
pub(crate) fn changes_path(
    folder: &FolderConfiguration,
    client: &str,
) -> Result<PathBuf, EngineError> {
    metadata_path(folder, client, "changes.sqlite3")
}

/// Returns the per-root upload settings and intent path after validating its root.
///
/// # Errors
/// Returns an error if the existing root identity cannot be verified safely.
pub(crate) fn uploads_path(
    folder: &FolderConfiguration,
    client: &str,
) -> Result<PathBuf, EngineError> {
    metadata_path(folder, client, "uploads.sqlite3")
}

pub(crate) fn metadata_path(
    folder: &FolderConfiguration,
    client: &str,
    name: &str,
) -> Result<PathBuf, EngineError> {
    verify(folder, client)?;
    crate::state::recovery::resolve(&folder.local.join(DIRECTORY), name)
}

/// Creates or opens the private metadata directory without following symlinks.
fn open_state_dir(root: &File) -> Result<File, EngineError> {
    match fs::mkdirat(root, DIRECTORY, Mode::RWXU) {
        Ok(()) => root
            .sync_all()
            .map_err(|_| EngineError::FolderIdentityStorage)?,
        Err(rustix::io::Errno::EXIST) => {}
        Err(_) => return Err(EngineError::FolderIdentityStorage),
    }
    fs::openat(
        root,
        DIRECTORY,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| EngineError::FolderIdentityStorage)
}

/// Opens every root component without following symlinks.
fn open_root(path: &Path) -> Result<File, EngineError> {
    if !path.is_absolute() {
        return Err(EngineError::FolderIdentityStorage);
    }
    let mut directory = fs::open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| EngineError::FolderIdentityStorage)?;
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                directory = fs::openat(
                    &directory,
                    name,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map(File::from)
                .map_err(|_| EngineError::FolderIdentityStorage)?;
            }
            _ => return Err(EngineError::FolderIdentityStorage),
        }
    }
    Ok(directory)
}

/// Reads only a regular, bounded marker without following symlinks.
fn read_marker(directory: &File) -> Result<Option<Marker>, EngineError> {
    let fd = match fs::openat(
        directory,
        IDENTITY,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => File::from(fd),
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(_) => return Err(EngineError::FolderIdentityStorage),
    };
    let metadata = fd
        .metadata()
        .map_err(|_| EngineError::FolderIdentityStorage)?;
    if !metadata.is_file() || metadata.size() > MAX_MARKER as u64 {
        return Err(EngineError::FolderIdentityMismatch);
    }
    let mut bytes = Vec::with_capacity(metadata.size() as usize);
    fd.take((MAX_MARKER + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| EngineError::FolderIdentityStorage)?;
    if bytes.len() > MAX_MARKER {
        return Err(EngineError::FolderIdentityMismatch);
    }
    let marker: Marker =
        serde_json::from_slice(&bytes).map_err(|_| EngineError::FolderIdentityMismatch)?;
    Ok(Some(marker))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};

    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "drivesync-root-id-{}",
                oauth2::CsrfToken::new_random().secret()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn folder(&self) -> FolderConfiguration {
            FolderConfiguration {
                account_id: AccountId("google:subject-a".into()),
                remote: super::super::DriveFolder {
                    id: "remote-a".into(),
                    name: "Remote".into(),
                },
                local: self.0.clone(),
                local_display: self.0.clone(),
            }
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn marker_is_portable_idempotent_and_account_scoped() {
        let dir = Directory::new();
        let folder = dir.folder();
        ensure(&folder, "client-a").unwrap();
        let path = dir.0.join(DIRECTORY).join(IDENTITY);
        assert_eq!(
            state_path(&folder, "client-a").unwrap(),
            dir.0.join(DIRECTORY).join(STATE)
        );
        assert_eq!(
            changes_path(&folder, "client-a").unwrap(),
            dir.0.join(DIRECTORY).join("changes.sqlite3")
        );
        assert_eq!(
            uploads_path(&folder, "client-a").unwrap(),
            dir.0.join(DIRECTORY).join("uploads.sqlite3")
        );
        let first = fs::read(&path).unwrap();
        ensure(&folder, "client-a").unwrap();
        assert_eq!(fs::read(&path).unwrap(), first);

        let mut other = folder.clone();
        other.account_id = AccountId("google:subject-b".into());
        assert_eq!(
            ensure(&other, "client-a"),
            Err(EngineError::FolderIdentityMismatch)
        );
        let mut other = folder.clone();
        other.remote.id = "remote-b".into();
        assert_eq!(
            ensure(&other, "client-a"),
            Err(EngineError::FolderIdentityMismatch)
        );
        assert_eq!(
            ensure(&folder, "client-b"),
            Err(EngineError::FolderIdentityMismatch)
        );
        assert_eq!(fs::read(path).unwrap(), first);
    }

    #[test]
    fn marker_rejects_corruption_and_symlinks_without_replacing_them() {
        let dir = Directory::new();
        let path = dir.0.join(DIRECTORY).join(IDENTITY);
        fs::create_dir(dir.0.join(DIRECTORY)).unwrap();
        fs::write(&path, b"{bad json").unwrap();
        assert_eq!(
            ensure(&dir.folder(), "client-a"),
            Err(EngineError::FolderIdentityMismatch)
        );
        assert_eq!(fs::read(&path).unwrap(), b"{bad json");

        fs::remove_dir_all(dir.0.join(DIRECTORY)).unwrap();
        let target = dir.0.join("target");
        fs::create_dir(&target).unwrap();
        std::os::unix::fs::symlink(&target, dir.0.join(DIRECTORY)).unwrap();
        assert_eq!(
            ensure(&dir.folder(), "client-a"),
            Err(EngineError::FolderIdentityStorage)
        );
        assert!(fs::symlink_metadata(dir.0.join(DIRECTORY))
            .unwrap()
            .file_type()
            .is_symlink());
    }
}
