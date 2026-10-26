//! Descriptor-relative local discovery and immutable private upload snapshots.
use super::Intent;
use crate::{
    download::filesystem::{self, Root},
    AuthError, Cancellation, EngineError,
};
use md5::Md5;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
};
pub(crate) fn check(cancel: &Cancellation) -> Result<(), EngineError> {
    if cancel.is_cancelled() {
        Err(EngineError::Authentication(AuthError::Cancelled))
    } else {
        Ok(())
    }
}
pub(crate) fn ignored(path: &Path) -> bool {
    path.components().any(|c| {
        c.as_os_str().to_str().is_none_or(|s| {
            s == ".drivesync"
                || s.starts_with(".drivesync-")
                || s.ends_with(".dslink")
                || (s.starts_with("DriveSync backup ") && s.ends_with(".bak"))
        })
    })
}
pub(crate) fn list(
    root: &Root,
    cancel: &Cancellation,
) -> Result<Vec<(PathBuf, bool)>, EngineError> {
    let mut result = Vec::new();
    let mut pending = vec![PathBuf::new()];
    while let Some(path) = pending.pop() {
        check(cancel)?;
        root.validate()?;
        let dir = if path.as_os_str().is_empty() {
            root.file.try_clone().map_err(filesystem::io)?
        } else {
            let (p, n) = root.parent(&path)?;
            rustix::fs::openat(
                &p,
                &n,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::NOFOLLOW,
                rustix::fs::Mode::empty(),
            )
            .map(File::from)
            .map_err(filesystem::io)?
        };
        for entry in rustix::fs::Dir::read_from(&dir).map_err(filesystem::io)? {
            check(cancel)?;
            let entry = entry.map_err(filesystem::io)?;
            let name = std::ffi::OsStr::from_bytes(entry.file_name().to_bytes());
            if name == "." || name == ".." {
                continue;
            }
            let child = path.join(name);
            if ignored(&child) {
                continue;
            }
            let stat = rustix::fs::statat(&dir, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
                .map_err(filesystem::io)?;
            let kind = rustix::fs::FileType::from_raw_mode(stat.st_mode);
            let directory = kind == rustix::fs::FileType::Directory;
            if directory {
                pending.push(child.clone());
            }
            // Special files remain visible to the executor, which rejects them
            // through the regular-file nofollow reader without blocking on FIFOs.
            result.push((child, directory));
            if result.len() > 100000 {
                return Err(EngineError::InventoryLimit);
            }
        }
    }
    result.sort_by(|a, b| {
        a.0.components()
            .count()
            .cmp(&b.0.components().count())
            .then_with(|| a.0.cmp(&b.0))
    });
    Ok(result)
}
type Stamp = (u64, u64, u64, i64, i64, i64, i64);
fn stamp(file: &File) -> Result<Stamp, EngineError> {
    let m = file.metadata().map_err(filesystem::io)?;
    Ok((
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    ))
}
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
    let (parent, name) = root.parent(path)?;
    let parent_id = filesystem::identity(&parent)?;
    root.validate_parent(path, parent_id)?;
    let mut file = filesystem::read(&parent, &name)?.ok_or(EngineError::DownloadConflict)?;
    let before = stamp(&file)?;
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
        if size > before.2 {
            return Err(EngineError::DownloadConflict);
        }
    }
    target.sync_all().map_err(filesystem::io)?;
    File::open(stage.parent().ok_or(EngineError::InvalidPlan)?)
        .and_then(|p| p.sync_all())
        .map_err(filesystem::io)?;
    root.validate_parent(path, parent_id)?;
    let current = filesystem::read(&parent, &name)?.ok_or(EngineError::DownloadConflict)?;
    if stamp(&file)? != before || stamp(&current)? != before || size != before.2 {
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
    let before = stamp(file)?;
    if !file.metadata().map_err(filesystem::io)?.is_file() {
        return Err(EngineError::StateStorage);
    }
    let mut sha = Sha256::new();
    let mut buffer = vec![0; 1024 * 1024];
    let mut size = 0;
    loop {
        check(cancel)?;
        let n = file.read(&mut buffer).map_err(filesystem::io)?;
        if n == 0 {
            break;
        }
        size += n as u64;
        sha.update(&buffer[..n]);
    }
    if size != op.size || format!("{:x}", sha.finalize()) != op.sha {
        return Err(EngineError::StateStorage);
    }
    if stamp(file)? != before {
        return Err(EngineError::StateStorage);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ignored;
    use std::path::Path;

    #[test]
    fn root_identity_marker_is_not_uploaded() {
        assert!(ignored(Path::new(".drivesync")));
        assert!(ignored(Path::new(".drivesync/identity.json")));
        assert!(ignored(Path::new(".drivesync/state.sqlite3")));
    }
}
