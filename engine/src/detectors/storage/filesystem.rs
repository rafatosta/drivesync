//! Private app-data directories and no-follow queue file creation.
use super::models::*;
use std::{
    ffi::{CString, OsStr},
    fs::File,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{ffi::OsStrExt, fs::MetadataExt},
    },
    path::{Component, Path},
};
fn error() -> QueueFailure {
    QueueFailure::new(QueueError::Unavailable)
}
fn open_at(parent: &File, name: &OsStr) -> QueueResult<File> {
    let name = CString::new(name.as_bytes()).map_err(|_| error())?;
    // SAFETY: owned parent and NUL-terminated relative component remain live.
    let mut fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ENOENT) {
        // SAFETY: same live parent and component; mode creates an owner-private directory.
        if unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) } < 0
            && std::io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST)
        {
            return Err(error());
        }
        parent.sync_all().map_err(|_| error())?;
        // SAFETY: transfer one newly opened directory descriptor below.
        fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
    }
    if fd < 0 {
        return Err(error());
    }
    // SAFETY: a successful unique descriptor is transferred once.
    Ok(unsafe { File::from_raw_fd(fd) })
}
pub(super) fn directory(path: &Path) -> QueueResult<File> {
    if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(error());
    }
    let mut parent = File::open("/").map_err(|_| error())?;
    for component in path.components() {
        if let Component::Normal(name) = component {
            parent = open_at(&parent, name)?;
        }
    }
    let metadata = parent.metadata().map_err(|_| error())?;
    // SAFETY: geteuid has no arguments or memory effects.
    if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o077 != 0 {
        return Err(error());
    }
    Ok(parent)
}
pub(super) fn database(parent: &File) -> QueueResult<(File, bool)> {
    let name = c"events.sqlite3";
    // SAFETY: parent stays live and the literal is NUL-terminated.
    let mut fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDWR | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_CREAT | libc::O_EXCL,
            0o600,
        )
    };
    let created = fd >= 0;
    if !created && std::io::Error::last_os_error().raw_os_error() == Some(libc::EEXIST) {
        // SAFETY: existing files are opened without following a final symlink.
        fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                libc::O_PATH | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
    }
    if fd < 0 {
        return Err(error());
    }
    // SAFETY: successful descriptor is transferred once.
    let file = unsafe { File::from_raw_fd(fd) };
    let metadata = file.metadata().map_err(|_| error())?;
    // SAFETY: geteuid has no pointer arguments.
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err(error());
    }
    let file = if created {
        file
    } else {
        // Reopen our already verified regular inode, never an untrusted device or symlink.
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/proc/self/fd/{}", file.as_raw_fd()))
            .map_err(|_| error())?
    };
    if created {
        file.sync_all().map_err(|_| error())?;
        parent.sync_all().map_err(|_| error())?;
    }
    Ok((file, created))
}
