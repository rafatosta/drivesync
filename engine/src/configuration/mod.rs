//! Initial folder configuration; no transfers or autostart.
use crate::{AccountId, EngineError};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriveFolder {
    pub id: String,
    pub name: String,
}
impl DriveFolder {
    pub(crate) fn valid(&self) -> bool {
        valid_id(&self.id)
            && !self.name.is_empty()
            && self.name.len() <= 4096
            && !self.name.chars().any(char::is_control)
    }
}
pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 255
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FolderConfiguration {
    pub account_id: AccountId,
    pub remote: DriveFolder,
    pub local: PathBuf,
    pub local_display: PathBuf,
}
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
/// Portal aliases must be compared by their host location, not separate doc IDs.
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
fn portal_origin(path: &Path, mount: &Path, host: &Path) -> Result<PathBuf, EngineError> {
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
pub(crate) fn overlap(a: &Path, b: &Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn path_overlap_uses_components_not_text_prefixes() {
        assert!(overlap(Path::new("/a/b"), Path::new("/a/b/c")));
        assert!(overlap(Path::new("/a/b/c"), Path::new("/a/b")));
        assert!(overlap(Path::new("/a/b"), Path::new("/a/b")));
        assert!(!overlap(Path::new("/a/b"), Path::new("/a/b-other")));
    }
    #[test]
    fn portal_exports_use_host_paths_for_overlap() {
        let parent = portal_origin(
            Path::new("/run/user/1000/doc/aaa/Projects"),
            Path::new("/run/user/1000/doc"),
            Path::new("/home/alex/Projects"),
        )
        .unwrap();
        let child = portal_origin(
            Path::new("/run/user/1000/doc/bbb/Child"),
            Path::new("/run/user/1000/doc"),
            Path::new("/home/alex/Projects/Child"),
        )
        .unwrap();
        assert!(overlap(&parent, &child));
        assert!(portal_origin(
            Path::new("/elsewhere/doc/aaa/Projects"),
            Path::new("/run/user/1000/doc"),
            Path::new("/home/alex/Projects")
        )
        .is_err());
    }
    #[test]
    fn remote_ids_reject_query_injection_and_paths() {
        for id in ["", "../root", "a' or true", "x/y", "x?key=y"] {
            assert!(!valid_id(id));
        }
        assert!(valid_id("root"));
        assert!(valid_id("ab-CD_123"));
    }
}
