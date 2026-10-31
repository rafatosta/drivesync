//! Independent effect markers prevent a lost operation database becoming empty work.
use super::{execution::Context, Intent, Stage};
use crate::{
    download::filesystem::{self, Root},
    EngineError,
};
use rustix::fs::{self, Mode, OFlags};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Read, Write},
};

const PREFIX: &str = ".drivesync-resolution-";
#[derive(PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Marker {
    format: u32,
    client: String,
    account: crate::AccountId,
    remote: String,
    root: crate::download::Identity,
    operation: String,
    file: String,
}
fn expected(ctx: &Context<'_>, intent: &Intent) -> Marker {
    Marker {
        format: 1,
        client: ctx.client.into(),
        account: ctx.scope.account_id.clone(),
        remote: ctx.scope.remote_root.clone(),
        root: intent.root,
        operation: intent.id.clone(),
        file: intent.plan.remote.item.id.clone(),
    }
}
fn name(id: &str) -> String {
    format!("{PREFIX}{id}.guard")
}
fn read(root: &Root, name: &str) -> Result<Option<Marker>, EngineError> {
    let file = match fs::openat(
        &root.file,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => File::from(fd),
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(_) => return Err(EngineError::StateStorage),
    };
    if !file.metadata().map_err(filesystem::io)?.is_file() {
        return Err(EngineError::StateStorage);
    }
    let mut bytes = Vec::new();
    file.take(16385)
        .read_to_end(&mut bytes)
        .map_err(filesystem::io)?;
    if bytes.len() > 16384 {
        return Err(EngineError::StateStorage);
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| EngineError::StateStorage)
}
/// This marker is synchronized before the first remote or local cutover effect.
/// A partial/corrupt marker fails closed instead of being silently recreated.
pub(super) fn ensure(ctx: &Context<'_>, intent: &Intent) -> Result<(), EngineError> {
    ctx.root.validate()?;
    if filesystem::identity(&ctx.root.file)? != intent.root {
        return Err(EngineError::DownloadConflict);
    }
    let marker = expected(ctx, intent);
    let name = name(&intent.id);
    if let Some(old) = read(ctx.root, &name)? {
        return if old == marker {
            Ok(())
        } else {
            Err(EngineError::StateStorage)
        };
    }
    let mut file = File::from(
        fs::openat(
            &ctx.root.file,
            name.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(|_| EngineError::StateStorage)?,
    );
    file.write_all(&serde_json::to_vec(&marker).map_err(|_| EngineError::StateStorage)?)
        .map_err(filesystem::io)?;
    file.sync_all().map_err(filesystem::io)?;
    ctx.root.file.sync_all().map_err(filesystem::io)?;
    ctx.root.validate()
}

/// Retirement preserves marker bytes under a completed name; it never deletes
/// an unknown file or treats a failed directory fsync as an absent rename.
pub(super) fn retire(ctx: &Context<'_>, intent: &Intent) -> Result<(), EngineError> {
    if intent.stage != Stage::Confirmed {
        return Err(EngineError::InvalidPlan);
    }
    let name = name(&intent.id);
    let Some(marker) = read(ctx.root, &name)? else {
        return Ok(());
    };
    if marker != expected(ctx, intent) {
        return Err(EngineError::StateStorage);
    }
    let done = format!("{PREFIX}{}.done", intent.id);
    match fs::renameat_with(
        &ctx.root.file,
        name.as_str(),
        &ctx.root.file,
        done.as_str(),
        fs::RenameFlags::NOREPLACE,
    ) {
        Ok(()) => {}
        Err(rustix::io::Errno::EXIST) => {
            if read(ctx.root, &done)? != Some(marker) {
                return Err(EngineError::StateStorage);
            }
        }
        Err(_) => return Err(EngineError::StateStorage),
    }
    ctx.root.file.sync_all().map_err(filesystem::io)?;
    ctx.root.validate()
}

/// Validate all independent active markers before planning any effects. Losing
/// SQLite after a remote request must not allow a fresh operation or download.
pub(crate) fn check(ctx: &Context<'_>) -> Result<(), EngineError> {
    ctx.root.validate()?;
    if ctx.scope.local_root != ctx.root.path() {
        return Err(EngineError::InvalidPlan);
    }
    let intents = ctx.store.resolutions(ctx.client, ctx.scope)?;
    let entries = fs::Dir::read_from(&ctx.root.file).map_err(|_| EngineError::StateStorage)?;
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|_| EngineError::StateStorage)?;
        let Ok(filename) = entry.file_name().to_str() else {
            continue;
        };
        if !filename.starts_with(PREFIX) || !filename.ends_with(".guard") {
            continue;
        }
        names.push(filename.to_owned());
        if names.len() > 100_000 {
            return Err(EngineError::StateStorage);
        }
    }
    for filename in names {
        let marker = read(ctx.root, &filename)?.ok_or(EngineError::StateStorage)?;
        let intent = intents
            .iter()
            .find(|intent| name(&intent.id) == filename)
            .ok_or(EngineError::StateStorage)?;
        if marker != expected(ctx, intent)
            || marker.root != filesystem::identity(&ctx.root.file)?
            || intent.stage < Stage::CopyPending
        {
            return Err(EngineError::StateStorage);
        }
        if intent.stage == Stage::Confirmed {
            retire(ctx, intent)?;
        }
    }
    ctx.root.validate()
}
