//! Local GNOME link entries for browser-based Google Workspace documents.
use super::filesystem::{self, Root};
use crate::{remote::valid_workspace_link, Cancellation, EngineError};
use rustix::fs::AtFlags;
use std::{
    ffi::OsStr,
    fs::File,
    io::{Read, Write},
    os::unix::fs::PermissionsExt,
    path::Path,
};

/// Creates a verified `.desktop` link without replacing an existing local entry.
/// Writes through an exclusive temporary file, rechecks the authorized root,
/// and installs with no-clobber rename. An identical existing link is a no-op;
/// divergent files, symlinks, and other entry types return a conflict.
pub(super) fn ensure(
    root: &Root,
    path: &Path,
    remote_name: &str,
    mime_type: &str,
    url: &str,
    cancel: &Cancellation,
) -> Result<(), EngineError> {
    let contents = entry(remote_name, mime_type, url)?;
    let (parent, name) = root.parent(path)?;
    let parent_id = filesystem::identity(&parent)?;
    root.validate_parent(path, parent_id)?;
    if matches_existing(root, &parent, &name, path, parent_id, &contents)? {
        return Ok(());
    }
    if filesystem::exists(&parent, &name)? {
        return Err(EngineError::DownloadConflict);
    }
    if cancel.is_cancelled() {
        return Err(EngineError::Authentication(crate::AuthError::Cancelled));
    }

    let temporary = format!(
        ".drivesync-link-{}.part",
        oauth2::CsrfToken::new_random().secret()
    );
    let result = (|| {
        let mut staged = filesystem::create(&parent, &temporary, contents.len() as u64)?;
        staged.write_all(&contents).map_err(filesystem::io)?;
        staged
            .set_permissions(std::fs::Permissions::from_mode(0o700))
            .map_err(filesystem::io)?;
        staged.sync_all().map_err(filesystem::io)?;
        parent.sync_all().map_err(filesystem::io)?;
        let staged_id = filesystem::identity(&staged)?;
        let mut check = filesystem::read(&parent, OsStr::new(&temporary))?
            .ok_or(EngineError::DownloadConflict)?;
        if filesystem::identity(&check)? != staged_id || !same_bytes(&mut check, &contents)? {
            return Err(EngineError::DownloadIntegrity);
        }
        root.validate_parent(path, parent_id)?;
        if cancel.is_cancelled() {
            return Err(EngineError::Authentication(crate::AuthError::Cancelled));
        }
        match filesystem::install(&parent, &temporary, &name) {
            Ok(()) => Ok(()),
            Err(EngineError::DownloadConflict)
                if matches_existing(root, &parent, &name, path, parent_id, &contents)? =>
            {
                Ok(())
            }
            Err(error) => Err(error),
        }
    })();
    let _ = rustix::fs::unlinkat(&parent, temporary.as_str(), AtFlags::empty());
    result
}

fn entry(name: &str, mime_type: &str, url: &str) -> Result<Vec<u8>, EngineError> {
    let mime_path = match mime_type {
        "application/vnd.google-apps.document" => Some("/document/"),
        "application/vnd.google-apps.spreadsheet" => Some("/spreadsheets/"),
        "application/vnd.google-apps.presentation" => Some("/presentation/"),
        "application/vnd.google-apps.shortcut" => [
            "/document/",
            "/spreadsheets/",
            "/presentation/",
        ]
        .into_iter()
        .find(|path| valid_workspace_link(url, Some(path))),
        _ => None,
    };
    if !valid_workspace_link(url, mime_path) {
        return Err(EngineError::InvalidPlan);
    }
    let name = escape(name);
    let url = escape(url);
    Ok(format!(
        "[Desktop Entry]\nVersion=1.5\nType=Link\nName={name}\nURL={url}\n"
    )
    .into_bytes())
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn matches_existing(
    root: &Root,
    parent: &File,
    name: &OsStr,
    path: &Path,
    parent_id: super::models::Identity,
    expected: &[u8],
) -> Result<bool, EngineError> {
    let Some(mut current) = filesystem::read(parent, name)? else {
        return Ok(false);
    };
    let current_id = filesystem::identity(&current)?;
    if !same_bytes(&mut current, expected)? {
        return Ok(false);
    }
    root.validate_parent(path, parent_id)?;
    let Some(reopened) = filesystem::read(parent, name)? else {
        return Ok(false);
    };
    Ok(filesystem::identity(&reopened)? == current_id)
}

fn same_bytes(file: &mut File, expected: &[u8]) -> Result<bool, EngineError> {
    if file.metadata().map_err(filesystem::io)?.len() != expected.len() as u64 {
        return Ok(false);
    }
    let mut actual = Vec::with_capacity(expected.len());
    file.take(expected.len() as u64 + 1)
        .read_to_end(&mut actual)
        .map_err(filesystem::io)?;
    Ok(actual == expected)
}
