//! Local directory validation and Flatpak document-portal path resolution.
//!
//! The functions here validate local roots, map portal aliases back to host
//! locations and compare canonical roots without touching user file contents.
use crate::EngineError;
use std::{
    fs::{self, OpenOptions},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

/// Selects and probes a writable local directory for folder configuration.
///
/// The `path` must be absolute and refer to an existing directory other than
/// the filesystem root. A unique, mode-`0600` temporary file is created and
/// removed to check write access and attempt shared locks; no user files are changed.
/// A fresh portal selection uses its host origin when already accessible through
/// no-follow descriptors. Existing `.drivesync` state prevents this selection.
///
/// # Errors
///
/// Returns [`EngineError::InvalidFolder`] when the path is invalid,
/// cannot be canonicalized, is not a directory, or the access probe fails.
/// Unsupported regular-file flock does not prevent folder configuration.
pub(crate) fn local_folder(path: &Path) -> Result<PathBuf, EngineError> {
    if !path.is_absolute() {
        return Err(EngineError::InvalidFolder);
    }
    let path = fs::canonicalize(path).map_err(|_| EngineError::InvalidFolder)?;
    if !path.is_dir() || path.parent().is_none() {
        return Err(EngineError::InvalidFolder);
    }
    let origin = local_origin(&path)?;
    let path = fresh_root_access(&path, &origin);
    let probe = path.join(format!(
        ".drivesync-access-{}",
        oauth2::CsrfToken::new_random().secret()
    ));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&probe)
        .map_err(|_| EngineError::InvalidFolder)?;
    let capability = probe_lock(&file);
    drop(file);
    fs::remove_file(probe).map_err(|_| EngineError::InvalidFolder)?;
    capability?;
    Ok(path)
}

/// Preserves the existing no-hardlink and cooperative-lock checks for the probe.
fn probe_lock(file: &std::fs::File) -> Result<(), EngineError> {
    let regular = || {
        let metadata = file.metadata().map_err(|error| {
            if error.kind() == std::io::ErrorKind::PermissionDenied {
                EngineError::LocalAccessDenied
            } else {
                EngineError::DownloadIo
            }
        })?;
        if !metadata.is_file() || metadata.nlink() != 1 {
            return Err(EngineError::DownloadConflict);
        }
        Ok(())
    };
    regular()?;
    match rustix::fs::flock(file, rustix::fs::FlockOperation::NonBlockingLockShared) {
        Ok(()) | Err(rustix::io::Errno::NOSYS | rustix::io::Errno::OPNOTSUPP) => regular(),
        Err(rustix::io::Errno::WOULDBLOCK) => Err(EngineError::LocalWriting),
        Err(rustix::io::Errno::ACCESS | rustix::io::Errno::PERM) => {
            Err(EngineError::LocalAccessDenied)
        }
        Err(rustix::io::Errno::NOSPC | rustix::io::Errno::DQUOT) => {
            Err(EngineError::LocalStorageFull)
        }
        Err(_) => Err(EngineError::DownloadIo),
    }
}

/// Selects an already sandbox-authorized host origin only before any root state
/// exists. Portal and direct inode evidence is never interchangeable. An absent
/// or inaccessible origin keeps the selected view; no permission is requested.
fn fresh_root_access(selected: &Path, origin: &Path) -> PathBuf {
    if selected == origin
        || !matches!(fs::symlink_metadata(selected.join(".drivesync")), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
        || !matches!(fs::symlink_metadata(origin.join(".drivesync")), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
        || access_path(origin)
            .and_then(|path| super::identity::open_root(&path))
            .is_err()
    {
        return selected.into();
    }
    origin.into()
}
/// Resolves a Flatpak document-portal alias to its host-side location.
///
/// Non-portal paths are returned unchanged; callers should canonicalize local
/// paths beforehand. Portal paths are resolved with the session D-Bus portal
/// so two different document IDs cannot conceal overlapping host folders.
///
/// # Errors
///
/// Returns [`EngineError::LocalFolderIdentity`] if the portal is unavailable,
/// its returned paths are inconsistent, or the origin cannot be established.
pub(crate) fn local_origin(path: &Path) -> Result<PathBuf, EngineError> {
    let parts: Vec<_> = path.components().collect();
    let portal_path = path.starts_with("/run/flatpak/doc")
        || (path.starts_with("/run/user") && parts.get(4).is_some_and(|c| c.as_os_str() == "doc"));
    if !portal_path {
        return Ok(path.to_path_buf());
    }
    let connection =
        dbus::blocking::Connection::new_session().map_err(|_| EngineError::LocalFolderIdentity)?;
    let proxy = connection.with_proxy(
        "org.freedesktop.portal.Documents",
        "/org/freedesktop/portal/documents",
        std::time::Duration::from_secs(10),
    );
    let (mount,): (Vec<u8>,) = proxy
        .method_call("org.freedesktop.portal.Documents", "GetMountPoint", ())
        .map_err(|_| EngineError::LocalFolderIdentity)?;
    use std::os::unix::ffi::OsStringExt;
    let mount = PathBuf::from(std::ffi::OsString::from_vec(
        mount.into_iter().take_while(|b| *b != 0).collect(),
    ));
    let mount = access_path(&mount)?;
    let path = access_path(path)?;
    let relative = path
        .strip_prefix(&mount)
        .map_err(|_| EngineError::LocalFolderIdentity)?;
    let id = relative
        .components()
        .next()
        .and_then(|c| c.as_os_str().to_str())
        .ok_or(EngineError::LocalFolderIdentity)?;
    let (paths,): (std::collections::HashMap<String, Vec<u8>>,) = proxy
        .method_call(
            "org.freedesktop.portal.Documents",
            "GetHostPaths",
            (vec![id],),
        )
        .map_err(|_| EngineError::LocalFolderIdentity)?;
    let host = paths.get(id).ok_or(EngineError::LocalFolderIdentity)?;
    let host = PathBuf::from(std::ffi::OsString::from_vec(
        host.iter().copied().take_while(|b| *b != 0).collect(),
    ));
    portal_origin(&path, &mount, &host)
}
/// Reconstructs a host path from a validated portal mount and its export.
///
/// Rejects unexpected mount prefixes, malformed components, relative host
/// locations and exports whose final component disagrees with the host path.
///
/// # Errors
///
/// Returns [`EngineError::LocalFolderIdentity`] for an inconsistent mapping.
pub(super) fn portal_origin(
    path: &Path,
    mount: &Path,
    host: &Path,
) -> Result<PathBuf, EngineError> {
    let relative = path
        .strip_prefix(mount)
        .map_err(|_| EngineError::LocalFolderIdentity)?;
    let parts: Vec<_> = relative.components().collect();
    if parts.len() < 2
        || !host.is_absolute()
        || host.file_name() != Some(parts[1].as_os_str())
        || parts
            .iter()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(EngineError::LocalFolderIdentity);
    }
    Ok(host.join(parts.iter().skip(2).collect::<PathBuf>()))
}
/// Detects equal or nested directory roots using path components.
///
/// Both paths must already represent the appropriate canonical/host origin.
/// The function does not resolve symlinks, query the portal or access files.
pub(crate) fn overlap(a: &Path, b: &Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}

/// Translates only Flatpak's fixed runtime document-mount alias. User-selected
/// symlinks, exported directory entries and descendants are never canonicalized.
/// Persisted configuration/scope strings remain unchanged; this is an I/O path.
pub(crate) fn access_path(path: &Path) -> Result<PathBuf, EngineError> {
    if !Path::new("/.flatpak-info").is_file() {
        return Ok(path.into());
    }
    let parts: Vec<_> = path.components().collect();
    if parts.len() < 5 {
        return Ok(path.into());
    }
    let mount: PathBuf = parts.iter().take(5).collect();
    if !runtime_document_mount(&mount)
        || !fs::symlink_metadata(&mount).is_ok_and(|m| m.file_type().is_symlink())
    {
        return Ok(path.into());
    }
    let link = fs::read_link(&mount).map_err(|_| EngineError::LocalFolderIdentity)?;
    portal_alias_target(path, &link).ok_or(EngineError::LocalFolderIdentity)
}

fn runtime_document_mount(path: &Path) -> bool {
    let parts: Vec<_> = path.components().collect();
    parts.len() == 5
        && path.starts_with("/run/user")
        && parts[4].as_os_str() == "doc"
        && parts[3]
            .as_os_str()
            .to_str()
            .is_some_and(|uid| !uid.is_empty() && uid.bytes().all(|b| b.is_ascii_digit()))
}

/// Only the fixed sandbox mount destination is admissible. Lexical parent
/// traversal is rejected before constructing a descriptor-relative access path.
fn portal_alias_target(path: &Path, target: &Path) -> Option<PathBuf> {
    let parts: Vec<_> = path.components().collect();
    if parts.len() < 5
        || !runtime_document_mount(&parts.iter().take(5).collect::<PathBuf>())
        || parts
            .iter()
            .skip(1)
            .any(|p| !matches!(p, std::path::Component::Normal(_)))
        || !matches!(
            target.to_str(),
            Some("../../flatpak/doc" | "/run/flatpak/doc")
        )
    {
        return None;
    }
    Some(Path::new("/run/flatpak/doc").join(parts.iter().skip(5).collect::<PathBuf>()))
}

#[cfg(test)]
mod access_tests {
    use super::*;
    #[test]
    fn fresh_portal_selection_uses_only_accessible_origins_without_existing_state() {
        let base = std::env::temp_dir().join(format!(
            "drivesync-access-{}",
            oauth2::CsrfToken::new_random().secret()
        ));
        fs::create_dir(&base).unwrap();
        let selected = base.join("selected");
        let origin = base.join("origin");
        fs::create_dir(&selected).unwrap();
        fs::create_dir(&origin).unwrap();
        assert_eq!(fresh_root_access(&selected, &origin), origin);
        assert_eq!(
            fresh_root_access(&selected, &base.join("missing")),
            selected
        );
        fs::create_dir(selected.join(".drivesync")).unwrap();
        fs::write(selected.join(".drivesync/evidence"), b"preserve").unwrap();
        assert_eq!(fresh_root_access(&selected, &origin), selected);
        assert_eq!(
            fs::read(selected.join(".drivesync/evidence")).unwrap(),
            b"preserve"
        );
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn local_folder_capability_probe_preserves_existing_files_and_cleans_itself() {
        let base = std::env::temp_dir().join(format!(
            "drivesync-probe-{}",
            oauth2::CsrfToken::new_random().secret()
        ));
        fs::create_dir(&base).unwrap();
        fs::write(base.join("existing"), b"preserve").unwrap();
        assert_eq!(
            local_folder(&base).unwrap(),
            fs::canonicalize(&base).unwrap()
        );
        assert_eq!(fs::read(base.join("existing")).unwrap(), b"preserve");
        assert_eq!(fs::read_dir(&base).unwrap().count(), 1);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn probe_rejects_hardlinks_and_contended_locks() {
        let base = std::env::temp_dir().join(format!(
            "drivesync-probe-{}",
            oauth2::CsrfToken::new_random().secret()
        ));
        fs::create_dir(&base).unwrap();
        let path = base.join("file");
        fs::write(&path, b"preserve").unwrap();
        let file = std::fs::File::open(&path).unwrap();
        fs::hard_link(&path, base.join("link")).unwrap();
        assert_eq!(probe_lock(&file), Err(EngineError::DownloadConflict));
        fs::remove_file(base.join("link")).unwrap();
        let exclusive = std::fs::File::open(&path).unwrap();
        rustix::fs::flock(
            &exclusive,
            rustix::fs::FlockOperation::NonBlockingLockExclusive,
        )
        .unwrap();
        assert_eq!(probe_lock(&file), Err(EngineError::LocalWriting));
        drop(exclusive);
        assert_eq!(probe_lock(&file), Ok(()));
        assert_eq!(fs::read(path).unwrap(), b"preserve");
        drop(file);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn only_the_fixed_flatpak_runtime_alias_is_translated() {
        for target in ["../../flatpak/doc", "/run/flatpak/doc"] {
            assert_eq!(
                portal_alias_target(
                    Path::new("/run/user/1000/doc/grant/Folder/sub"),
                    Path::new(target)
                ),
                Some("/run/flatpak/doc/grant/Folder/sub".into())
            );
            assert_eq!(
                portal_alias_target(Path::new("/run/user/1000/doc"), Path::new(target)),
                Some("/run/flatpak/doc".into())
            );
        }
        for (path, target) in [
            ("/tmp/doc/grant/Folder", "../../flatpak/doc"),
            ("/run/user/name/doc/grant/Folder", "../../flatpak/doc"),
            ("/run/user/1000/document/grant/Folder", "../../flatpak/doc"),
            ("/run/user/1000/doc/../escape", "../../flatpak/doc"),
            ("/run/user/1000/doc/grant/Folder", "/tmp/other"),
            ("/run/user/1000/doc/grant/Folder", "../../flatpak/other"),
            (
                "/run/user/1000/doc/grant/Folder",
                "/run/user/1000/doc/by-app/other",
            ),
        ] {
            assert_eq!(
                portal_alias_target(Path::new(path), Path::new(target)),
                None
            );
        }
    }
}
