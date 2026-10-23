//! Descriptor-relative Linux I/O. No symlink traversal or replacement fallback.
use super::models::Identity;
use crate::EngineError;
use rustix::fs::{self, Mode, OFlags};
use std::{
    ffi::OsStr,
    fs::File,
    os::unix::fs::MetadataExt,
    path::{Component, Path},
};

pub(super) fn io(_: impl std::fmt::Debug) -> EngineError {
    EngineError::DownloadIo
}
pub(super) fn identity(file: &File) -> Result<Identity, EngineError> {
    let m = file.metadata().map_err(io)?;
    Ok(Identity {
        device: m.dev(),
        inode: m.ino(),
    })
}
fn directory(parent: &File, name: &OsStr) -> Result<File, EngineError> {
    fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|e| match e {
        rustix::io::Errno::LOOP | rustix::io::Errno::NOTDIR => EngineError::DownloadConflict,
        _ => io(e),
    })
}
/// Holds an exclusive advisory lock on the authorized root inode for this cycle.
pub(super) struct Root {
    pub file: File,
    path: std::path::PathBuf,
}
impl Root {
    pub fn open(path: &Path) -> Result<Self, EngineError> {
        let file = Self::open_path(path)?;
        fs::flock(&file, fs::FlockOperation::NonBlockingLockExclusive)
            .map_err(|_| EngineError::SyncBusy)?;
        Ok(Self {
            file,
            path: path.into(),
        })
    }
    fn open_path(path: &Path) -> Result<File, EngineError> {
        if !path.is_absolute() {
            return Err(EngineError::InvalidFolder);
        }
        let mut file = File::open("/").map_err(io)?;
        for part in path.components() {
            match part {
                Component::RootDir => {}
                Component::Normal(name) => file = directory(&file, name)?,
                _ => return Err(EngineError::InvalidFolder),
            }
        }
        Ok(file)
    }
    pub fn validate(&self) -> Result<(), EngineError> {
        if identity(&Self::open_path(&self.path)?)? != identity(&self.file)? {
            return Err(EngineError::DownloadConflict);
        }
        Ok(())
    }
    /// Resolves only existing safe directory components. Creation is explicit.
    pub fn parent(&self, relative: &Path) -> Result<(File, std::ffi::OsString), EngineError> {
        let parts: Vec<_> = relative.components().collect();
        if parts.is_empty() || parts.iter().any(|p| !matches!(p, Component::Normal(_))) {
            return Err(EngineError::InvalidPlan);
        }
        let mut parent = self.file.try_clone().map_err(io)?;
        for part in &parts[..parts.len() - 1] {
            parent = directory(&parent, part.as_os_str())?;
        }
        Ok((parent, parts.last().unwrap().as_os_str().into()))
    }
    /// Validates an existing directory without creating anything on the fast path.
    pub fn check_directory(&self, relative: &Path) -> Result<(), EngineError> {
        let (parent, name) = self.parent(relative)?;
        directory(&parent, &name)?;
        self.validate_parent(relative, identity(&parent)?)
    }
    pub fn mkdir(&self, relative: &Path) -> Result<(), EngineError> {
        let (parent, name) = self.parent(relative)?;
        self.validate_parent(relative, identity(&parent)?)?;
        match fs::mkdirat(&parent, &name, Mode::RWXU) {
            Ok(()) => parent.sync_all().map_err(io)?,
            Err(rustix::io::Errno::EXIST) => {}
            Err(e) => return Err(io(e)),
        }
        directory(&parent, &name)?;
        Ok(())
    }
    pub fn validate_parent(&self, relative: &Path, expected: Identity) -> Result<(), EngineError> {
        self.validate()?;
        if identity(&self.parent(relative)?.0)? != expected {
            return Err(EngineError::DownloadConflict);
        }
        Ok(())
    }
}
/// Opens regular files only; NONBLOCK prevents a hostile FIFO from blocking.
pub(super) fn read(parent: &File, name: &OsStr) -> Result<Option<File>, EngineError> {
    match fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => {
            let file = File::from(fd);
            if !file.metadata().map_err(io)?.is_file() {
                return Err(EngineError::DownloadConflict);
            }
            Ok(Some(file))
        }
        Err(rustix::io::Errno::NOENT) => Ok(None),
        Err(rustix::io::Errno::LOOP) => Err(EngineError::DownloadConflict),
        Err(e) => Err(io(e)),
    }
}
pub(super) fn create(parent: &File, name: &str, size: u64) -> Result<File, EngineError> {
    let space = fs::fstatvfs(parent).map_err(io)?;
    if u128::from(space.f_bavail) * u128::from(space.f_frsize) < u128::from(size) {
        return Err(EngineError::DownloadIo);
    }
    fs::openat(
        parent,
        name,
        OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map(File::from)
    .map_err(io)
}
/// Kernel-enforced no-clobber installation; unsupported filesystems fail closed.
pub(super) fn install(parent: &File, temp: &str, name: &OsStr) -> Result<(), EngineError> {
    move_entry(parent, OsStr::new(temp), name)
}
/// Moves an entry without replacing any destination, then syncs the directory.
pub(super) fn move_entry(parent: &File, old: &OsStr, new: &OsStr) -> Result<(), EngineError> {
    rename_entry(parent, old, new)?;
    parent.sync_all().map_err(io)
}
/// Reports syscall success independently so journaled recovery can handle fsync failure.
pub(super) fn rename_entry(parent: &File, old: &OsStr, new: &OsStr) -> Result<(), EngineError> {
    fs::renameat_with(parent, old, parent, new, fs::RenameFlags::NOREPLACE).map_err(|e| {
        if e == rustix::io::Errno::EXIST {
            EngineError::DownloadConflict
        } else {
            io(e)
        }
    })?;
    Ok(())
}

/// Observes an entry without following symlinks, including unexpected file types.
pub(super) fn exists(parent: &File, name: &OsStr) -> Result<bool, EngineError> {
    match fs::statat(parent, name, fs::AtFlags::SYMLINK_NOFOLLOW) {
        Ok(_) => Ok(true),
        Err(rustix::io::Errno::NOENT) => Ok(false),
        Err(e) => Err(io(e)),
    }
}
