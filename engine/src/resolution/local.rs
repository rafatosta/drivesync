//! Descriptor-relative snapshots and no-clobber cutover; displaced files remain.
use super::{Blob, Intent, ResolutionChoice};
use crate::{
    download::{
        filesystem::{self, Root},
        stream,
    },
    Cancellation, EngineError,
};
use md5::{Digest, Md5};
use sha2::Sha256;
use std::{
    ffi::OsStr,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

pub(super) fn remote_name(intent: &Intent) -> String {
    format!("DriveSync backup conflict-{}-remote.bak", intent.id)
}

pub(super) fn parent(root: &Root, intent: &Intent) -> Result<File, EngineError> {
    root.validate_parent(&intent.plan.origin, intent.parent)?;
    if filesystem::identity(&root.file)? != intent.root {
        return Err(EngineError::DownloadConflict);
    }
    Ok(root.parent(&intent.plan.origin)?.0)
}

/// Hashes a stable regular inode and verifies that the path still refers to it.
pub(super) fn inspect(
    parent: &File,
    name: &OsStr,
    cancel: &Cancellation,
) -> Result<Option<Blob>, EngineError> {
    let Some(mut file) = filesystem::read(parent, name)? else {
        return Ok(None);
    };
    let stamp = crate::local::Stamp::read(&file)?;
    let identity = filesystem::identity(&file)?;
    let mut md5 = Md5::new();
    let mut sha = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0; 65536];
    loop {
        crate::local::check(cancel)?;
        let count = file.read(&mut buffer).map_err(filesystem::io)?;
        if count == 0 {
            break;
        }
        size = size
            .checked_add(count as u64)
            .ok_or(EngineError::DownloadIntegrity)?;
        if size > stamp.size {
            return Err(EngineError::DownloadConflict);
        }
        md5.update(&buffer[..count]);
        sha.update(&buffer[..count]);
    }
    if size != stamp.size
        || crate::local::Stamp::read(&file)? != stamp
        || filesystem::read(parent, name)?
            .as_ref()
            .map(crate::local::Stamp::read)
            .transpose()?
            != Some(stamp)
    {
        return Err(EngineError::DownloadConflict);
    }
    Ok(Some(Blob {
        identity,
        size,
        md5: format!("{:x}", md5.finalize()),
        sha256: format!("{:x}", sha.finalize()),
    }))
}
pub(super) fn matches(
    parent: &File,
    name: &OsStr,
    expected: &Blob,
    cancel: &Cancellation,
) -> Result<bool, EngineError> {
    Ok(inspect(parent, name, cancel)?.as_ref() == Some(expected))
}

pub(super) struct Expected<'a> {
    pub size: u64,
    pub md5: &'a str,
    pub sha256: Option<&'a str>,
}
pub(super) fn capture(
    root: &Root,
    intent: &Intent,
    name: &str,
    input: &mut dyn Read,
    expected: Expected<'_>,
    cancel: &Cancellation,
) -> Result<Blob, EngineError> {
    let Expected {
        size,
        md5,
        sha256: sha,
    } = expected;
    let directory = parent(root, intent)?;
    // Restart may adopt only a complete snapshot satisfying the expected digest.
    if let Some(existing) = inspect(&directory, OsStr::new(name), cancel)? {
        if existing.size == size
            && existing.md5.eq_ignore_ascii_case(md5)
            && sha.is_none_or(|hash| existing.sha256.eq_ignore_ascii_case(hash))
        {
            return Ok(existing);
        }
        return Err(EngineError::DownloadIntegrity);
    }
    let mut file = filesystem::create(&directory, name, size)?;
    let hash = stream::copy(
        input,
        &mut file,
        size,
        md5,
        cancel,
        |_| {},
        || root.validate_parent(&intent.plan.origin, intent.parent),
    )?;
    if sha.is_some_and(|expected| !hash.eq_ignore_ascii_case(expected)) {
        return Err(EngineError::DownloadIntegrity);
    }
    file.sync_all().map_err(filesystem::io)?;
    directory.sync_all().map_err(filesystem::io)?;
    root.validate_parent(&intent.plan.origin, intent.parent)?;
    Ok(Blob {
        identity: filesystem::identity(&file)?,
        size,
        sha256: hash,
        md5: md5.to_ascii_lowercase(),
    })
}

pub(super) fn original(
    root: &Root,
    intent: &Intent,
    cancel: &Cancellation,
) -> Result<Blob, EngineError> {
    let directory = parent(root, intent)?;
    let name = intent
        .plan
        .origin
        .file_name()
        .ok_or(EngineError::InvalidPlan)?;
    let blob = inspect(&directory, name, cancel)?.ok_or(EngineError::DownloadConflict)?;
    if blob.identity.fingerprint(&blob.sha256) != intent.plan.local_fingerprint {
        return Err(EngineError::DownloadConflict);
    }
    Ok(blob)
}
pub(super) fn snapshot_local(
    root: &Root,
    intent: &Intent,
    cancel: &Cancellation,
) -> Result<Blob, EngineError> {
    let blob = original(root, intent, cancel)?;
    let directory = parent(root, intent)?;
    let mut file = filesystem::read(
        &directory,
        intent
            .plan
            .origin
            .file_name()
            .ok_or(EngineError::InvalidPlan)?,
    )?
    .ok_or(EngineError::DownloadConflict)?;
    let saved = capture(
        root,
        intent,
        &intent.temporary("local"),
        &mut file,
        Expected {
            size: blob.size,
            md5: &blob.md5,
            sha256: Some(&blob.sha256),
        },
        cancel,
    )?;
    if original(root, intent, cancel)? != blob {
        return Err(EngineError::DownloadConflict);
    }
    Ok(saved)
}

pub(super) fn snapshot_file(
    root: &Root,
    intent: &Intent,
    remote: bool,
    cancel: &Cancellation,
) -> Result<File, EngineError> {
    let directory = parent(root, intent)?;
    let name = if remote {
        remote_name(intent)
    } else {
        intent.temporary("local")
    };
    let expected = if remote {
        &intent.remote
    } else {
        &intent.local
    };
    if !matches(
        &directory,
        OsStr::new(&name),
        expected.as_ref().ok_or(EngineError::StateStorage)?,
        cancel,
    )? {
        return Err(EngineError::DownloadIntegrity);
    }
    filesystem::read(&directory, OsStr::new(&name))?.ok_or(EngineError::DownloadIntegrity)
}

/// Prepares immutable installation bytes before the LocalPending boundary.
/// The returned inode is persisted before it is exposed at the final name.
pub(super) fn installation(
    root: &Root,
    intent: &Intent,
    copy: bool,
    cancel: &Cancellation,
) -> Result<Blob, EngineError> {
    let remote = copy || intent.plan.choice == ResolutionChoice::KeepRemote;
    let blob = if remote {
        intent.remote.as_ref()
    } else {
        intent.local.as_ref()
    }
    .ok_or(EngineError::StateStorage)?;
    let mut input = snapshot_file(root, intent, remote, cancel)?;
    input.seek(SeekFrom::Start(0)).map_err(filesystem::io)?;
    capture(
        root,
        intent,
        &intent.temporary(if copy { "copy" } else { "result" }),
        &mut input,
        Expected {
            size: blob.size,
            md5: &blob.md5,
            sha256: Some(&blob.sha256),
        },
        cancel,
    )
}

fn final_blob(intent: &Intent, copy: bool) -> Result<Blob, EngineError> {
    let mut blob = if copy {
        intent
            .remote
            .as_ref()
            .ok_or(EngineError::StateStorage)?
            .clone()
    } else {
        intent.winner()?.clone()
    };
    blob.identity = if copy {
        intent.copy_installed
    } else {
        intent.installed
    }
    .ok_or(EngineError::StateStorage)?;
    Ok(blob)
}

pub(super) fn verify_installed(
    root: &Root,
    intent: &Intent,
    copy: bool,
    cancel: &Cancellation,
) -> Result<bool, EngineError> {
    let directory = parent(root, intent)?;
    let name = if copy {
        OsStr::new(
            intent
                .plan
                .copy_name
                .as_ref()
                .ok_or(EngineError::StateStorage)?,
        )
    } else {
        intent
            .plan
            .target
            .file_name()
            .ok_or(EngineError::InvalidPlan)?
    };
    matches(&directory, name, &final_blob(intent, copy)?, cancel)
}

/// A writer retaining the displaced inode may continue editing it. Keep those
/// bytes, but do not confirm a decision that no longer matches its preservation.
pub(super) fn verify_preserved(
    root: &Root,
    intent: &Intent,
    cancel: &Cancellation,
) -> Result<(), EngineError> {
    let directory = parent(root, intent)?;
    let old = inspect(&directory, OsStr::new(&intent.backup()), cancel)?
        .ok_or(EngineError::DownloadConflict)?;
    if old.identity.fingerprint(&old.sha256) != intent.plan.local_fingerprint
        || !matches(
            &directory,
            OsStr::new(&remote_name(intent)),
            intent.remote.as_ref().ok_or(EngineError::StateStorage)?,
            cancel,
        )?
    {
        return Err(EngineError::DownloadConflict);
    }
    Ok(())
}

/// Cutover never follows links, replaces a destination, or deletes the displaced
/// inode. A concurrent writer remains in the verified backup or original path.
pub(super) fn install(
    root: &Root,
    intent: &Intent,
    copy: bool,
    cancel: &Cancellation,
) -> Result<(), EngineError> {
    let directory = parent(root, intent)?;
    if verify_installed(root, intent, copy, cancel)? {
        if !copy {
            verify_preserved(root, intent, cancel)?;
        }
        directory.sync_all().map_err(filesystem::io)?;
        return Ok(());
    }
    let target = if copy {
        OsStr::new(
            intent
                .plan
                .copy_name
                .as_ref()
                .ok_or(EngineError::StateStorage)?,
        )
    } else {
        intent
            .plan
            .target
            .file_name()
            .ok_or(EngineError::InvalidPlan)?
    };
    let temporary = intent.temporary(if copy { "copy" } else { "result" });
    if (copy || intent.plan.origin != intent.plan.target) && filesystem::exists(&directory, target)?
    {
        return Err(EngineError::DownloadConflict);
    }
    if !matches(
        &directory,
        OsStr::new(&temporary),
        &final_blob(intent, copy)?,
        cancel,
    )? {
        return Err(EngineError::DownloadIntegrity);
    }
    if !copy {
        let origin = intent
            .plan
            .origin
            .file_name()
            .ok_or(EngineError::InvalidPlan)?;
        let backup = intent.backup();
        if filesystem::exists(&directory, OsStr::new(&backup))? {
            let saved = inspect(&directory, OsStr::new(&backup), cancel)?
                .ok_or(EngineError::DownloadConflict)?;
            if saved.identity.fingerprint(&saved.sha256) != intent.plan.local_fingerprint {
                return Err(EngineError::DownloadConflict);
            }
            if filesystem::exists(&directory, origin)? {
                return Err(EngineError::DownloadConflict);
            }
        } else {
            original(root, intent, cancel)?;
            crate::local::check(cancel)?;
            filesystem::move_entry(&directory, origin, OsStr::new(&backup))?;
            let saved = inspect(&directory, OsStr::new(&backup), &Cancellation::new())?
                .ok_or(EngineError::DownloadConflict)?;
            if saved.identity.fingerprint(&saved.sha256) != intent.plan.local_fingerprint {
                return Err(EngineError::DownloadConflict);
            }
        }
    }
    // Once the origin has moved, complete or retain recoverable evidence even
    // if cancellation arrives. Revalidation follows the irreversible syscall.
    root.validate_parent(&intent.plan.target, intent.parent)?;
    filesystem::move_entry(&directory, OsStr::new(&temporary), target)?;
    if !verify_installed(root, intent, copy, &Cancellation::new())? {
        return Err(EngineError::DownloadConflict);
    }
    root.validate_parent(&intent.plan.target, intent.parent)
}

pub(super) fn target_available(
    root: &Root,
    origin: &Path,
    target: &Path,
) -> Result<(), EngineError> {
    if origin == target {
        return Ok(());
    }
    let (parent, name) = root.parent(target)?;
    if filesystem::exists(&parent, &name)? {
        return Err(EngineError::DownloadConflict);
    }
    Ok(())
}
