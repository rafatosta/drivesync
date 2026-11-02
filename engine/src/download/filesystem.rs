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

/// Classifies actual syscall failures without inferring access from POSIX mode bits.
/// ACLs and sandbox policy remain enforced by the kernel; no repair is attempted.
pub(crate) fn io(error: impl Into<std::io::Error>) -> EngineError {
    let error = error.into();
    if error.kind() == std::io::ErrorKind::PermissionDenied
        || error.raw_os_error() == Some(rustix::io::Errno::PERM.raw_os_error())
    {
        EngineError::LocalAccessDenied
    } else if [rustix::io::Errno::NOSPC, rustix::io::Errno::DQUOT]
        .iter()
        .any(|code| error.raw_os_error() == Some(code.raw_os_error()))
    {
        EngineError::LocalStorageFull
    } else {
        EngineError::DownloadIo
    }
}
pub(crate) fn identity(file: &File) -> Result<Identity, EngineError> {
    let m = file.metadata().map_err(io)?;
    Ok(Identity {
        device: m.dev(),
        inode: m.ino(),
    })
}
/// Hashes a regular file through no-follow descriptors and verifies the path
/// still names the same inode when the read completes.
#[cfg(test)]
pub(crate) fn fingerprint(
    root_path: &Path,
    relative: &Path,
    cancel: &crate::Cancellation,
) -> Result<Option<String>, EngineError> {
    let file = Root::open_path(root_path)?;
    let root = Root {
        file,
        path: root_path.into(),
    };
    fingerprint_root(&root, relative, cancel)
}
/// Uses an already validated root without acquiring another advisory lock.
pub(crate) fn fingerprint_root(
    root: &Root,
    relative: &Path,
    cancel: &crate::Cancellation,
) -> Result<Option<String>, EngineError> {
    let (parent, name) = root.parent(relative)?;
    let parent_id = identity(&parent)?;
    let Some(mut file) = read_transfer(&parent, &name)? else {
        return Ok(None);
    };
    let file_id = identity(&file)?;
    let content_fingerprint = super::stream::fingerprint(&mut file, cancel)?;
    root.validate_parent(relative, parent_id)?;
    if identity(&file)? != file_id
        || read_transfer(&parent, &name)?
            .as_ref()
            .map(identity)
            .transpose()?
            != Some(file_id)
    {
        return Err(EngineError::DownloadConflict);
    }
    Ok(Some(file_id.fingerprint(&content_fingerprint)))
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
pub(crate) struct Root {
    pub file: File,
    path: std::path::PathBuf,
}
impl Root {
    /// Returns the authorized path; descriptor validation still guards all I/O.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub fn open(path: &Path) -> Result<Self, EngineError> {
        let file = Self::open_path(path)?;
        fs::flock(&file, fs::FlockOperation::NonBlockingLockExclusive)
            .map_err(|_| EngineError::SyncBusy)?;
        Ok(Self {
            file,
            path: path.into(),
        })
    }
    /// Opens a read-only observation anchor without taking the synchronization lock.
    pub(crate) fn observe(path: &Path) -> Result<Self, EngineError> {
        Ok(Self {
            file: Self::open_path(path)?,
            path: path.into(),
        })
    }
    fn open_path(path: &Path) -> Result<File, EngineError> {
        if !path.is_absolute() {
            return Err(EngineError::InvalidFolder);
        }
        let access = crate::configuration::access_path(path)?;
        let mut file = File::open("/").map_err(io)?;
        for part in access.components() {
            match part {
                Component::RootDir => {}
                Component::Normal(name) => file = directory(&file, name)?,
                _ => return Err(EngineError::InvalidFolder),
            }
        }
        Ok(file)
    }
    pub fn validate(&self) -> Result<(), EngineError> {
        let unavailable = || EngineError::RootSuspended(crate::RootSuspension::LocalUnavailable);
        let current = Self::open_path(&self.path).map_err(|_| unavailable())?;
        if identity(&current).map_err(|_| unavailable())?
            != identity(&self.file).map_err(|_| unavailable())?
        {
            return Err(unavailable());
        }
        fs::accessat(
            &self.file,
            ".",
            fs::Access::READ_OK | fs::Access::WRITE_OK | fs::Access::EXEC_OK,
            fs::AtFlags::EACCESS,
        )
        .map_err(|_| unavailable())?;
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
/// Rejects aliases outside the authorized tree and nonregular descriptors.
/// Link counts can change later, so callers recheck after reading as well.
pub(crate) fn regular(file: &File) -> Result<(), EngineError> {
    let metadata = file.metadata().map_err(io)?;
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err(EngineError::DownloadConflict);
    }
    Ok(())
}

/// Holds a shared flock until the descriptor closes. Contention defers the item.
/// This coordinates cooperative flock users, not POSIX record locks or writers
/// that ignore advisory locks; stamps and hashes remain mandatory.
pub(crate) fn lock_regular(file: &File) -> Result<(), EngineError> {
    regular(file)?;
    #[cfg(test)]
    if UNSUPPORTED_FILE_LOCKS.get() {
        return Err(EngineError::LocalLockUnsupported);
    }
    fs::flock(file, fs::FlockOperation::NonBlockingLockShared).map_err(lock_error)?;
    regular(file)
}

/// Verified transfers can use optimistic reads when file flock is
/// unsupported. Contention and all other failures still defer the operation.
/// Callers must verify stamps/hashes and install without replacing destinations.
/// Upload callers must additionally verify a private snapshot and source before
/// publication. Conflict execution keeps using `lock_regular`/`read`.
pub(crate) fn lock_transfer(file: &File) -> Result<(), EngineError> {
    match lock_regular(file) {
        Err(EngineError::LocalLockUnsupported) => regular(file),
        result => result,
    }
}

#[cfg(test)]
thread_local! {
    static UNSUPPORTED_FILE_LOCKS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Injects only regular-file lock failure on this test thread; root locks and
/// real filesystem/SQLite operations remain active, including during recovery.
#[cfg(test)]
pub(crate) fn with_unsupported_file_locks<T>(run: impl FnOnce() -> T) -> T {
    struct Reset(bool);
    impl Drop for Reset {
        fn drop(&mut self) {
            UNSUPPORTED_FILE_LOCKS.set(self.0);
        }
    }
    let _reset = Reset(UNSUPPORTED_FILE_LOCKS.replace(true));
    run()
}

/// Classifies strict-source lock failures; download policy is applied separately.
fn lock_error(error: rustix::io::Errno) -> EngineError {
    match error {
        rustix::io::Errno::WOULDBLOCK => EngineError::LocalWriting,
        rustix::io::Errno::NOSYS | rustix::io::Errno::OPNOTSUPP => {
            EngineError::LocalLockUnsupported
        }
        _ => io(error),
    }
}

/// Opens single-link regular files only; NONBLOCK prevents FIFO blocking.
/// The shared advisory lock is retained by the returned descriptor.
pub(crate) fn read(parent: &File, name: &OsStr) -> Result<Option<File>, EngineError> {
    read_with(parent, name, lock_regular)
}

/// Opens a verified transfer observation without requiring a regular-file flock
/// capability. This is not an upload authorization or a mutation lease.
pub(crate) fn read_transfer(parent: &File, name: &OsStr) -> Result<Option<File>, EngineError> {
    read_with(parent, name, lock_transfer)
}

fn read_with(
    parent: &File,
    name: &OsStr,
    lock: fn(&File) -> Result<(), EngineError>,
) -> Result<Option<File>, EngineError> {
    match fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => {
            let file = File::from(fd);
            lock(&file)?;
            Ok(Some(file))
        }
        Err(rustix::io::Errno::NOENT) => Ok(None),
        Err(rustix::io::Errno::LOOP) => Err(EngineError::DownloadConflict),
        Err(e) => Err(io(e)),
    }
}
pub(crate) fn create(parent: &File, name: &str, size: u64) -> Result<File, EngineError> {
    let space = fs::fstatvfs(parent).map_err(io)?;
    if u128::from(space.f_bavail) * u128::from(space.f_frsize) < u128::from(size) {
        return Err(EngineError::LocalStorageFull);
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
pub(crate) fn move_entry(parent: &File, old: &OsStr, new: &OsStr) -> Result<(), EngineError> {
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
pub(crate) fn exists(parent: &File, name: &OsStr) -> Result<bool, EngineError> {
    match fs::statat(parent, name, fs::AtFlags::SYMLINK_NOFOLLOW) {
        Ok(_) => Ok(true),
        Err(rustix::io::Errno::NOENT) => Ok(false),
        Err(e) => Err(io(e)),
    }
}

#[cfg(test)]
mod capacity_tests {
    use super::*;

    #[test]
    fn only_verified_reads_allow_unsupported_flock_and_never_special_files() {
        let file = std::fs::File::open("/dev/null").unwrap();
        with_unsupported_file_locks(|| {
            assert_eq!(lock_transfer(&file), Err(EngineError::DownloadConflict));
            let base = std::env::temp_dir().join(format!(
                "drivesync-lock-policy-{}",
                oauth2::CsrfToken::new_random().secret()
            ));
            std::fs::create_dir(&base).unwrap();
            std::fs::write(base.join("file"), b"preserve").unwrap();
            let parent = File::open(&base).unwrap();
            assert!(matches!(
                read(&parent, OsStr::new("file")),
                Err(EngineError::LocalLockUnsupported)
            ));
            assert!(read_transfer(&parent, OsStr::new("file"))
                .unwrap()
                .is_some());
            assert_eq!(std::fs::read(base.join("file")).unwrap(), b"preserve");
            std::fs::remove_dir_all(base).unwrap();
        });
    }

    #[test]
    fn unsupported_file_locks_fail_closed_with_a_distinct_diagnosis() {
        for code in [rustix::io::Errno::NOSYS, rustix::io::Errno::OPNOTSUPP] {
            assert_eq!(lock_error(code), EngineError::LocalLockUnsupported);
        }
        assert_eq!(
            lock_error(rustix::io::Errno::WOULDBLOCK),
            EngineError::LocalWriting
        );
        assert_eq!(
            lock_error(rustix::io::Errno::ACCESS),
            EngineError::LocalAccessDenied
        );
        assert_eq!(lock_error(rustix::io::Errno::IO), EngineError::DownloadIo);
    }

    #[test]
    fn e02_classifies_capacity_failures_separately_from_permissions_and_io() {
        for code in [rustix::io::Errno::NOSPC, rustix::io::Errno::DQUOT] {
            assert_eq!(io(code), EngineError::LocalStorageFull);
        }
        assert_eq!(
            io(rustix::io::Errno::ACCESS),
            EngineError::LocalAccessDenied
        );
        assert_eq!(io(rustix::io::Errno::IO), EngineError::DownloadIo);
    }
}
