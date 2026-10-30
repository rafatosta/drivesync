//! Explicit, non-destructive replacement of invalid scoped state stores.
//!
//! A durable selector switches filenames only after replacement stores validate.
//! Originals and SQLite sidecars never move. The caller holds association and
//! physical-root locks throughout, including selector publication and fsync.
use super::{failed, CheckpointScope, Checkpoints, Reconciliations, Store, Uploads};
use crate::{AuthError, Cancellation, EngineError, FolderConfiguration};
use rustix::fs::{self, Mode, OFlags};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const SELECTOR: &str = "recovery.json";
const REQUIRED: &str = "recovery-required";
const NAMES: [&str; 4] = [
    "state.sqlite3",
    "changes.sqlite3",
    "reconciliation.sqlite3",
    "uploads.sqlite3",
];

/// Local recovery outcome, not confirmation of synchronization or remote effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateRecovery {
    /// Number of missing or invalid stores replaced. Healthy stores are retained.
    pub rebuilt_stores: usize,
    /// Lost upload intents require explicit reconciliation before any new upload.
    pub uploads_blocked: bool,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Selector {
    format: u32,
    stores: BTreeMap<String, String>,
    uploads_blocked: bool,
}

fn directory(path: &Path) -> Result<File, EngineError> {
    fs::open(
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(failed)
}

/// Reads bounded regular metadata without following a symlink or waiting on a FIFO.
fn selector(dir: &File) -> Result<Selector, EngineError> {
    let file = match fs::openat(
        dir,
        SELECTOR,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => File::from(fd),
        Err(rustix::io::Errno::NOENT) => {
            if recovery_required(dir)? {
                return Err(EngineError::StateStorage);
            }
            return Ok(Selector {
                format: 1,
                ..Selector::default()
            });
        }
        Err(e) => return Err(failed(e)),
    };
    if !file.metadata().map_err(failed)?.is_file() {
        return Err(EngineError::StateStorage);
    }
    let mut bytes = Vec::new();
    file.take(4097).read_to_end(&mut bytes).map_err(failed)?;
    if bytes.len() > 4096 {
        return Err(EngineError::StateStorage);
    }
    let value: Selector = serde_json::from_slice(&bytes).map_err(failed)?;
    if value.format != 1
        || value.stores.iter().any(|(key, name)| {
            !NAMES.contains(&key.as_str())
                || !name.starts_with("recovered-")
                || !name.ends_with(key)
                || name.len() > 160
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        })
    {
        return Err(EngineError::StateStorage);
    }
    Ok(value)
}

/// A durable marker prevents loss of a selector from selecting stale originals.
fn recovery_required(dir: &File) -> Result<bool, EngineError> {
    match fs::openat(
        dir,
        REQUIRED,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => {
            if File::from(fd).metadata().map_err(failed)?.is_file() {
                Ok(true)
            } else {
                Err(EngineError::StateStorage)
            }
        }
        Err(rustix::io::Errno::NOENT) => Ok(false),
        Err(error) => Err(failed(error)),
    }
}

/// Explicit recovery may replace a damaged selector, but cannot infer which old
/// generation was current. Preserve its bytes and rebuild all stores as unknown.
fn recover_selector(dir: &File) -> Result<(Selector, bool), EngineError> {
    match selector(dir) {
        Ok(value) => Ok((value, false)),
        Err(_) => {
            match fs::openat(
                dir,
                SELECTOR,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            ) {
                Ok(fd) => {
                    let mut input = File::from(fd);
                    if !input.metadata().map_err(failed)?.is_file() {
                        return Err(EngineError::StateStorage);
                    }
                    let name = format!(
                        "selector-evidence-{}.json",
                        oauth2::CsrfToken::new_random().secret()
                    );
                    let mut output = fs::openat(
                        dir,
                        name.as_str(),
                        OFlags::WRONLY
                            | OFlags::CREATE
                            | OFlags::EXCL
                            | OFlags::NOFOLLOW
                            | OFlags::CLOEXEC,
                        Mode::RUSR | Mode::WUSR,
                    )
                    .map(File::from)
                    .map_err(failed)?;
                    std::io::copy(&mut input, &mut output).map_err(failed)?;
                    output.sync_all().map_err(failed)?;
                    dir.sync_all().map_err(failed)?;
                }
                Err(rustix::io::Errno::NOENT) if recovery_required(dir)? => {}
                _ => return Err(EngineError::StateStorage),
            }
            Ok((
                Selector {
                    format: 1,
                    uploads_blocked: true,
                    ..Selector::default()
                },
                true,
            ))
        }
    }
}

/// Resolves only known stores; malformed selectors fail closed, never fall back.
pub(crate) fn resolve(parent: &Path, name: &str) -> Result<PathBuf, EngineError> {
    if !NAMES.contains(&name) {
        return Err(EngineError::StateStorage);
    }
    let value = selector(&directory(parent)?)?;
    if let Some(selected) = value.stores.get(name) {
        let file = fs::openat(
            &directory(parent)?,
            selected.as_str(),
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map(File::from)
        .map_err(failed)?;
        if !file.metadata().map_err(failed)?.is_file() {
            return Err(EngineError::StateStorage);
        }
        Ok(parent.join(selected))
    } else {
        Ok(parent.join(name))
    }
}

pub(crate) fn uploads_blocked(parent: &Path) -> Result<bool, EngineError> {
    Ok(selector(&directory(parent)?)?.uploads_blocked)
}

/// Validates both SQLite structure and scoped serialized evidence before reuse.
fn validate(
    path: &Path,
    name: &str,
    folder: &FolderConfiguration,
    client: &str,
) -> Result<(), EngineError> {
    let scope = CheckpointScope {
        client,
        account: &folder.account_id,
        remote: &folder.remote.id,
        local: &folder.local,
    };
    match name {
        "state.sqlite3" => {
            let store = Store::open(path)?;
            store.load(client, &folder.account_id, &folder.remote.id, &folder.local)?;
            store.confirmed_bases(client, &folder.account_id, &folder.remote.id, &folder.local)?;
        }
        "changes.sqlite3" => {
            Checkpoints::open(path)?.load(&scope)?;
        }
        "reconciliation.sqlite3" => {
            Reconciliations::open(path)?.load(&scope)?;
        }
        "uploads.sqlite3" => {
            Uploads::open(path, client, folder)?.validate_evidence()?;
        }
        _ => return Err(EngineError::StateStorage),
    }
    Ok(())
}

/// SQLite may consume a hot or malformed sidecar even while opening a corrupt
/// database. Inspect private copies so explicit recovery leaves originals intact.
/// The root lock ensures cooperating workers have closed their connections.
fn inspect(
    dir: &File,
    parent: &Path,
    active: &str,
    name: &str,
    folder: &FolderConfiguration,
    client: &str,
) -> Result<Result<(), EngineError>, EngineError> {
    let inspection = format!("inspection-{}", oauth2::CsrfToken::new_random().secret());
    fs::mkdirat(dir, inspection.as_str(), Mode::RWXU).map_err(failed)?;
    let copy_dir = directory(&parent.join(&inspection))?;
    for suffix in ["", "-journal", "-wal", "-shm"] {
        let source = format!("{active}{suffix}");
        let mut input = match fs::openat(
            dir,
            source.as_str(),
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => File::from(fd),
            Err(rustix::io::Errno::NOENT) if !suffix.is_empty() => continue,
            Err(e) => return Err(failed(e)),
        };
        if !input.metadata().map_err(failed)?.is_file() {
            return Err(EngineError::StateStorage);
        }
        let target = format!("{name}{suffix}");
        let mut output = fs::openat(
            &copy_dir,
            target.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map(File::from)
        .map_err(failed)?;
        std::io::copy(&mut input, &mut output).map_err(failed)?;
        output.sync_all().map_err(failed)?;
    }
    copy_dir.sync_all().map_err(failed)?;
    dir.sync_all().map_err(failed)?;
    let result = validate(&parent.join(&inspection).join(name), name, folder, client);
    // Only disposable copies created by this invocation are removed. Originals
    // and replacement candidates are never part of this cleanup.
    drop(copy_dir);
    std::fs::remove_dir_all(parent.join(inspection)).map_err(failed)?;
    dir.sync_all().map_err(failed)?;
    Ok(result)
}

/// Builds empty replacements only where evidence cannot be reused. No salvage of
/// arbitrary rows and no conversion of observations into confirmed installations.
/// Cancellation leaves the old selector active and candidates retained. SQLite
/// performs its own hot-journal rollback; application code never removes sidecars.
pub(crate) fn rebuild(
    folder: &FolderConfiguration,
    client: &str,
    cancel: &Cancellation,
) -> Result<StateRecovery, EngineError> {
    crate::configuration::identity::verify(folder, client)?;
    let parent = folder.local.join(".drivesync");
    let dir = directory(&parent)?;
    let (mut value, unknown_selection) = recover_selector(&dir)?;
    let mut rebuilt = 0;
    for name in NAMES {
        cancelled(cancel)?;
        let active = value.stores.get(name).map(String::as_str).unwrap_or(name);
        // Refuse unsafe or unreadable objects instead of hiding access failures.
        let missing = match fs::openat(
            &dir,
            active,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => {
                if !File::from(fd).metadata().map_err(failed)?.is_file() {
                    return Err(EngineError::StateStorage);
                }
                false
            }
            Err(rustix::io::Errno::NOENT) => true,
            Err(e) => return Err(failed(e)),
        };
        if !unknown_selection
            && !missing
            && inspect(&dir, &parent, active, name, folder, client)?.is_ok()
        {
            continue;
        }
        let replacement = format!(
            "recovered-{}-{name}",
            oauth2::CsrfToken::new_random().secret()
        );
        let replacement_path = parent.join(&replacement);
        validate(&replacement_path, name, folder, client)?;
        File::open(&replacement_path)
            .and_then(|f| f.sync_all())
            .map_err(failed)?;
        value.stores.insert(name.into(), replacement);
        if name == "uploads.sqlite3" {
            // Even a missing intent store may hide an already completed remote create.
            value.uploads_blocked = true;
        }
        rebuilt += 1;
    }
    if rebuilt > 0 {
        cancelled(cancel)?;
        publish(&dir, &value, cancel)?;
    }
    Ok(StateRecovery {
        rebuilt_stores: rebuilt,
        uploads_blocked: value.uploads_blocked,
    })
}

fn cancelled(cancel: &Cancellation) -> Result<(), EngineError> {
    if cancel.is_cancelled() {
        Err(EngineError::Authentication(AuthError::Cancelled))
    } else {
        Ok(())
    }
}

/// Candidates are durable before the atomic selector rename. A failed directory
/// sync returns failure even if the selector is already visible; retry revalidates
/// the selected stores. Unpublished files are preserved rather than auto-cleaned.
fn publish(dir: &File, value: &Selector, cancel: &Cancellation) -> Result<(), EngineError> {
    let name = format!("recovery-{}.tmp", oauth2::CsrfToken::new_random().secret());
    let mut file = fs::openat(
        dir,
        name.as_str(),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map(File::from)
    .map_err(failed)?;
    file.write_all(&serde_json::to_vec(value).map_err(failed)?)
        .map_err(failed)?;
    file.sync_all().map_err(failed)?;
    dir.sync_all().map_err(failed)?;
    cancelled(cancel)?;
    // Publish the no-fallback marker first. A crash before selector rename is
    // recoverable explicitly, but ordinary reads cannot revive stale originals.
    match fs::openat(
        dir,
        REQUIRED,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    ) {
        Ok(fd) => File::from(fd).sync_all().map_err(failed)?,
        Err(rustix::io::Errno::EXIST) if recovery_required(dir)? => {}
        Err(error) => return Err(failed(error)),
    }
    dir.sync_all().map_err(failed)?;
    fs::renameat(dir, name.as_str(), dir, SELECTOR).map_err(failed)?;
    dir.sync_all().map_err(failed)
}

#[cfg(test)]
mod tests;
