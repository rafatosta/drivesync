//! Local directory validation and Flatpak document-portal path resolution.
//!
//! The functions here validate local roots, map portal aliases back to host
//! locations and compare canonical roots without touching user file contents.
use crate::EngineError;
use std::{
    fs::{self, OpenOptions},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

/// Canonicalizes and probes a writable local directory.
///
/// The `path` must be absolute and refer to an existing directory other than
/// the filesystem root. A unique, mode-`0600` temporary file is created and
/// removed to check write access; no user files are changed.
///
/// # Errors
///
/// Returns [`EngineError::InvalidFolder`] when the path is invalid,
/// cannot be canonicalized, is not a directory, or the access probe fails.
pub(crate) fn local_folder(path: &Path) -> Result<PathBuf, EngineError> {
    if !path.is_absolute() {
        return Err(EngineError::InvalidFolder);
    }
    let path = fs::canonicalize(path).map_err(|_| EngineError::InvalidFolder)?;
    if !path.is_dir() || path.parent().is_none() {
        return Err(EngineError::InvalidFolder);
    }
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
    drop(file);
    fs::remove_file(probe).map_err(|_| EngineError::InvalidFolder)?;
    Ok(path)
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
    portal_origin(path, &mount, &host)
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
