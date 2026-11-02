//! Managed, read-only HTML redirects for supported Workspace documents.
//!
//! Uses the existing scoped download journal. Ready evidence precedes publication;
//! updates retain the previous inode before exclusive installation. No remote write,
//! legacy migration, content export, or inference of remote deletion is performed.
use super::{
    filesystem::{self, Root},
    models::{Identity, Operation, Phase, Previous},
    stream,
};
use crate::{state::Store, Cancellation, DownloadPlan, EngineError, InventoryEntry};
use sha2::{Digest, Sha256};
use std::{ffi::OsStr, fs::File, io::Write, os::unix::fs::PermissionsExt, path::Path};

/// Stable content marker also protects renamed/copied redirects from binary upload.
pub(crate) const MARKER: &str = "<!-- DriveSync Workspace link -->";

/// Generates inert markup using only an independently validated Google editor URL.
/// Both attribute and text contexts escape HTML metacharacters. No JavaScript,
/// executable command, local credential, or remote document content is embedded.
pub(crate) fn entry(item: &InventoryEntry) -> Result<Vec<u8>, EngineError> {
    let url = item
        .web_view_link
        .as_deref()
        .ok_or(EngineError::InvalidPlan)?;
    let path = match item.mime_type.as_str() {
        "application/vnd.google-apps.document" => Some("/document/"),
        "application/vnd.google-apps.spreadsheet" => Some("/spreadsheets/"),
        "application/vnd.google-apps.presentation" => Some("/presentation/"),
        "application/vnd.google-apps.shortcut" => {
            ["/document/", "/spreadsheets/", "/presentation/"]
                .into_iter()
                .find(|path| crate::remote::valid_workspace_link(url, Some(path)))
        }
        _ => None,
    };
    if !crate::remote::valid_workspace_link(url, path) {
        return Err(EngineError::InvalidPlan);
    }
    let name = escape(&item.name);
    let url = escape(url);
    Ok(format!("<!doctype html>\n{MARKER}\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta http-equiv=\"refresh\" content=\"0;url={url}\">\n<title>Open {name}</title>\n</head>\n<body>\n<a href=\"{url}\">Open {name} in Google Drive</a>\n<p>Browser link only. Offline editing, export and conversion are not supported.</p>\n</body>\n</html>\n").into_bytes())
}
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Scoped dependencies and journal snapshot retained under the worker root lock.
pub(super) struct Context<'a> {
    pub root: &'a Root,
    pub store: &'a Store,
    pub client: &'a str,
    pub scope: &'a DownloadPlan,
    pub cancel: &'a Cancellation,
    pub operations: &'a [Operation],
}

/// Executes under the root lock. Missing/edited/replaced managed links block
/// updates. Remote renames publish at the new path while retaining the old link;
/// same-path URL/title updates retain a named backup. Ordinary HTML is never adopted.
/// Ready records survive cancellation, persistence failure and process restart.
pub(super) fn ensure(
    context: Context<'_>,
    path: &Path,
    item: &InventoryEntry,
) -> Result<(), EngineError> {
    let Context {
        root,
        store,
        client,
        scope,
        cancel,
        operations,
    } = context;
    let contents = entry(item)?;
    let mut latest = None;
    for mut op in operations
        .iter()
        .filter(|op| op.item.id == item.id)
        .cloned()
    {
        if op.item.kind != crate::InventoryKind::GoogleWorkspaceLink {
            return Err(EngineError::DownloadConflict);
        }
        if matches!(op.phase, Phase::Ready { .. } | Phase::UpdateReady { .. }) {
            commit(root, &mut op, store, client, scope, cancel)?;
        }
        if matches!(op.phase, Phase::Installed { .. }) {
            latest = Some(op);
        }
    }
    let (parent, name) = root.parent(path)?;
    let parent_id = filesystem::identity(&parent)?;
    root.validate_parent(path, parent_id)?;
    let mut previous = None;
    if let Some(mut old) = latest {
        let Phase::Installed {
            identity,
            ref sha256,
        } = old.phase
        else {
            unreachable!()
        };
        let (old_parent, old_name) = root.parent(&old.relative)?;
        if old.root != filesystem::identity(&root.file)?
            || old.parent != filesystem::identity(&old_parent)?
            || !intact(&old_parent, &old_name, identity, &entry(&old.item)?, cancel)?
        {
            return Err(EngineError::DownloadConflict);
        }
        root.validate_parent(&old.relative, old.parent)?;
        if old.relative == path {
            if entry(&old.item)? == contents {
                if old.item != *item {
                    old.item = item.clone();
                    store.put_operation(client, scope, &old)?;
                }
                return Ok(());
            }
            previous = Some(Previous {
                identity,
                sha256: sha256.clone(),
                size: entry(&old.item)?.len() as u64,
                version: old.item.version.unwrap_or(1),
            });
        }
    }
    if previous.is_none() && filesystem::exists(&parent, &name)? {
        return Err(EngineError::DownloadConflict);
    }
    stream::cancelled(cancel)?;
    let id = oauth2::CsrfToken::new_random().secret().clone();
    let temporary = format!(".drivesync-link-{id}.part");
    let mut staged = filesystem::create(&parent, &temporary, contents.len() as u64)?;
    staged.write_all(&contents).map_err(filesystem::io)?;
    staged
        .set_permissions(std::fs::Permissions::from_mode(0o444))
        .map_err(filesystem::io)?;
    staged.sync_all().map_err(filesystem::io)?;
    parent.sync_all().map_err(filesystem::io)?;
    let identity = filesystem::identity(&staged)?;
    let sha256 = format!("{:x}", Sha256::digest(&contents));
    let phase = match previous {
        Some(previous) => Phase::UpdateReady {
            identity,
            sha256,
            previous,
            backup: format!("DriveSync backup {id}.bak"),
        },
        None => Phase::Ready { identity, sha256 },
    };
    let mut op = Operation {
        id,
        item: item.clone(),
        relative: path.into(),
        temporary,
        root: filesystem::identity(&root.file)?,
        parent: parent_id,
        phase,
    };
    store.put_operation(client, scope, &op)?;
    commit(root, &mut op, store, client, scope, cancel)
}

/// Verifies both bytes and inode through no-follow descriptors; permissions are
/// checked rather than repaired so an already modified link remains user data.
fn intact(
    parent: &File,
    name: &OsStr,
    expected: Identity,
    bytes: &[u8],
    cancel: &Cancellation,
) -> Result<bool, EngineError> {
    let Some(mut file) = filesystem::read_transfer(parent, name)? else {
        return Ok(false);
    };
    if filesystem::identity(&file)? != expected
        || file
            .metadata()
            .map_err(filesystem::io)?
            .permissions()
            .mode()
            & 0o777
            != 0o444
    {
        return Ok(false);
    }
    let sha = format!("{:x}", Sha256::digest(bytes));
    if !stream::verify(&mut file, bytes.len() as u64, &sha, cancel)? {
        return Ok(false);
    }
    Ok(filesystem::read_transfer(parent, name)?
        .as_ref()
        .map(filesystem::identity)
        .transpose()?
        == Some(expected))
}

/// Finishes only the recorded ready inode. Moving the prior entry to a durable
/// backup before no-clobber publication preserves concurrent edits at either path.
fn commit(
    root: &Root,
    op: &mut Operation,
    store: &Store,
    client: &str,
    scope: &DownloadPlan,
    cancel: &Cancellation,
) -> Result<(), EngineError> {
    stream::cancelled(cancel)?;
    let contents = entry(&op.item)?;
    let (new_id, sha, previous) = match &op.phase {
        Phase::Ready { identity, sha256 } => (*identity, sha256.clone(), None),
        Phase::UpdateReady {
            identity,
            sha256,
            previous,
            backup,
        } => (
            *identity,
            sha256.clone(),
            Some((previous.clone(), backup.clone())),
        ),
        _ => return Err(EngineError::InvalidPlan),
    };
    if sha != format!("{:x}", Sha256::digest(&contents)) {
        return Err(EngineError::StateStorage);
    }
    let (parent, name) = root.parent(&op.relative)?;
    if op.root != filesystem::identity(&root.file)? || op.parent != filesystem::identity(&parent)? {
        return Err(EngineError::DownloadConflict);
    }
    root.validate_parent(&op.relative, op.parent)?;
    if !intact(&parent, &name, new_id, &contents, cancel)? {
        if !intact(
            &parent,
            OsStr::new(&op.temporary),
            new_id,
            &contents,
            cancel,
        )? {
            return Err(EngineError::DownloadConflict);
        }
        if let Some((old, backup)) = previous {
            if filesystem::exists(&parent, &name)? {
                if !matches_prior(&parent, &name, &old, cancel)? {
                    return Err(EngineError::DownloadConflict);
                }
                filesystem::move_entry(&parent, &name, OsStr::new(&backup))?;
            }
            if !matches_prior(&parent, OsStr::new(&backup), &old, cancel)? {
                return Err(EngineError::DownloadConflict);
            }
        }
        root.validate_parent(&op.relative, op.parent)?;
        stream::cancelled(cancel)?;
        filesystem::install(&parent, &op.temporary, &name)?;
        if !intact(&parent, &name, new_id, &contents, cancel)? {
            return Err(EngineError::DownloadConflict);
        }
    }
    parent.sync_all().map_err(filesystem::io)?;
    root.validate_parent(&op.relative, op.parent)?;
    op.phase = Phase::Installed {
        identity: new_id,
        sha256: sha,
    };
    store.put_operation(client, scope, op)
}
fn matches_prior(
    parent: &File,
    name: &OsStr,
    old: &Previous,
    cancel: &Cancellation,
) -> Result<bool, EngineError> {
    let Some(mut file) = filesystem::read_transfer(parent, name)? else {
        return Ok(false);
    };
    Ok(filesystem::identity(&file)? == old.identity
        && stream::verify(&mut file, old.size, &old.sha256, cancel)?)
}
