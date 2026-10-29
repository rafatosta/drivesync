//! Descriptor-relative local discovery and immutable private upload snapshots.
use super::Intent;
pub(crate) use crate::local::check;
use crate::{
    download::filesystem::{self, Root},
    Cancellation, EngineError,
};
use md5::Md5;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
/// Removes only an incomplete staging file created by this invocation.
struct PartialSnapshot {
    path: PathBuf,
    complete: bool,
}
impl Drop for PartialSnapshot {
    fn drop(&mut self) {
        if !self.complete {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
pub(crate) fn snapshot(
    root: &Root,
    path: &Path,
    stage: &Path,
    cancel: &Cancellation,
) -> Result<(u64, String, String, crate::download::Identity), EngineError> {
    snapshot_with_progress(root, path, stage, cancel, |_| {})
}
/// The progress seam supports deterministic interruption and concurrent-writer tests.
fn snapshot_with_progress(
    root: &Root,
    path: &Path,
    stage: &Path,
    cancel: &Cancellation,
    mut copied: impl FnMut(u64),
) -> Result<(u64, String, String, crate::download::Identity), EngineError> {
    let (parent, name) = root.parent(path)?;
    let parent_id = filesystem::identity(&parent)?;
    root.validate_parent(path, parent_id)?;
    let mut file = filesystem::read(&parent, &name)?.ok_or(EngineError::DownloadConflict)?;
    let before = crate::local::Stamp::read(&file)?;
    let mut target = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(stage)
        .map_err(filesystem::io)?;
    let mut cleanup = PartialSnapshot {
        path: stage.to_path_buf(),
        complete: false,
    };
    let mut sha = Sha256::new();
    let mut md5 = Md5::new();
    let mut size = 0u64;
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        check(cancel)?;
        let n = file.read(&mut buffer).map_err(filesystem::io)?;
        if n == 0 {
            break;
        }
        target.write_all(&buffer[..n]).map_err(filesystem::io)?;
        sha.update(&buffer[..n]);
        md5.update(&buffer[..n]);
        size += n as u64;
        copied(size);
        if size > before.size {
            return Err(EngineError::DownloadConflict);
        }
    }
    target.sync_all().map_err(filesystem::io)?;
    File::open(stage.parent().ok_or(EngineError::InvalidPlan)?)
        .and_then(|p| p.sync_all())
        .map_err(filesystem::io)?;
    root.validate_parent(path, parent_id)?;
    let current = filesystem::read(&parent, &name)?.ok_or(EngineError::DownloadConflict)?;
    if crate::local::Stamp::read(&file)? != before
        || crate::local::Stamp::read(&current)? != before
        || size != before.size
    {
        return Err(EngineError::DownloadConflict);
    }
    cleanup.complete = true;
    Ok((
        size,
        format!("{:x}", md5.finalize()),
        format!("{:x}", sha.finalize()),
        filesystem::identity(&file)?,
    ))
}
/// Revalidates the original source, not only the immutable staging bytes.
/// Returns its signature so chunk boundaries can detect resumed external writes.
pub(crate) fn verify_source(
    root: &Root,
    path: &Path,
    op: &Intent,
    cancel: &Cancellation,
) -> Result<crate::local::Stamp, EngineError> {
    let before = crate::local::stamp(root, path)?;
    let (parent, name) = root.parent(path)?;
    let parent_id = filesystem::identity(&parent)?;
    let mut file = filesystem::read(&parent, &name)?.ok_or(EngineError::DownloadConflict)?;
    if op.source != Some(filesystem::identity(&file)?) {
        return Err(EngineError::DownloadConflict);
    }
    verify_file(&mut file, op, cancel).map_err(|error| {
        if error == EngineError::StateStorage {
            EngineError::DownloadConflict
        } else {
            error
        }
    })?;
    root.validate_parent(path, parent_id)?;
    if crate::local::stamp(root, path)? != before {
        return Err(EngineError::LocalWriting);
    }
    Ok(before)
}
pub(crate) fn verify_stage(
    path: &Path,
    op: &Intent,
    cancel: &Cancellation,
) -> Result<File, EngineError> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(path)
        .map_err(filesystem::io)?;
    verify_file(&mut file, op, cancel)?;
    Ok(file)
}
/// Hashes the opened descriptor, without resolving a pathname a second time.
pub(crate) fn verify_file(
    file: &mut File,
    op: &Intent,
    cancel: &Cancellation,
) -> Result<(), EngineError> {
    let before = crate::local::Stamp::read(file)?;
    if !file.metadata().map_err(filesystem::io)?.is_file() || before.size != op.size {
        return Err(EngineError::StateStorage);
    }
    let mut sha = Sha256::new();
    let mut buffer = vec![0; 1024 * 1024];
    let mut size = 0;
    loop {
        check(cancel)?;
        let limit = op
            .size
            .saturating_sub(size)
            .saturating_add(1)
            .min(buffer.len() as u64) as usize;
        let n = file.read(&mut buffer[..limit]).map_err(filesystem::io)?;
        if n == 0 {
            break;
        }
        size = size
            .checked_add(n as u64)
            .ok_or(EngineError::StateStorage)?;
        if size > op.size {
            return Err(EngineError::StateStorage);
        }
        sha.update(&buffer[..n]);
    }
    if size != op.size || format!("{:x}", sha.finalize()) != op.sha {
        return Err(EngineError::StateStorage);
    }
    if crate::local::Stamp::read(file)? != before {
        return Err(EngineError::StateStorage);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::local::ignored;
    use std::path::Path;

    #[test]
    fn root_identity_marker_is_not_uploaded() {
        assert!(ignored(Path::new(".drivesync")));
        assert!(ignored(Path::new(".drivesync/identity.json")));
        assert!(ignored(Path::new(".drivesync/state.sqlite3")));
    }
    #[test]
    fn n10_write_or_replacement_during_snapshot_discards_only_partial_staging() {
        for replacement in [false, true] {
            let base = std::env::temp_dir().join(format!(
                "drivesync-snapshot-{}",
                oauth2::CsrfToken::new_random().secret()
            ));
            let root_path = base.join("root");
            std::fs::create_dir_all(&root_path).unwrap();
            let bytes = vec![b'a'; 2 * 1024 * 1024];
            let path = root_path.join("file");
            std::fs::write(&path, &bytes).unwrap();
            let stage = base.join("private.snapshot");
            let root = super::Root::open(&root_path).unwrap();
            let mut changed = false;
            let result = super::snapshot_with_progress(
                &root,
                Path::new("file"),
                &stage,
                &crate::Cancellation::new(),
                |_| {
                    if !changed {
                        changed = true;
                        if replacement {
                            std::fs::write(root_path.join("replacement"), &bytes).unwrap();
                            std::fs::rename(root_path.join("replacement"), &path).unwrap();
                        } else {
                            std::fs::write(&path, b"writer resumed").unwrap();
                        }
                    }
                },
            );
            assert_eq!(result, Err(crate::EngineError::DownloadConflict));
            assert!(!stage.exists());
            assert_eq!(
                std::fs::read(&path).unwrap(),
                if replacement {
                    bytes
                } else {
                    b"writer resumed".to_vec()
                }
            );
            std::fs::write(&stage, b"preexisting").unwrap();
            assert!(super::snapshot(
                &root,
                Path::new("file"),
                &stage,
                &crate::Cancellation::new()
            )
            .is_err());
            assert_eq!(std::fs::read(&stage).unwrap(), b"preexisting");
            drop(root);
            std::fs::remove_dir_all(base).unwrap();
        }
    }
}
