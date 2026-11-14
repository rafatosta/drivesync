//! Descriptor-bound private files; never follows root or database symlinks.
use super::{failure, InventoryError, InventoryResult, OpenInventoryRequest};
use std::{
    fs::{self, File, OpenOptions},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
        },
    },
    path::{Component, Path},
};
fn io(error: std::io::Error) -> super::InventoryFailure {
    failure(if error.kind() == std::io::ErrorKind::PermissionDenied {
        InventoryError::PermissionDenied
    } else {
        InventoryError::StorageUnavailable
    })
}
fn directory(path: &Path) -> InventoryResult<File> {
    if !path.is_absolute() {
        return Err(failure(InventoryError::InvalidArgument));
    }
    let mut current = File::open("/").map_err(io)?;
    for component in path.components() {
        match component {
            Component::RootDir => continue,
            Component::Normal(name) => {
                let name = std::ffi::CString::new(name.as_bytes())
                    .map_err(|_| failure(InventoryError::InvalidArgument))?;
                // SAFETY: valid descriptor and NUL-terminated component; returned fd is owned.
                let fd = unsafe {
                    libc::openat(
                        current.as_raw_fd(),
                        name.as_ptr(),
                        libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_RDONLY,
                    )
                };
                if fd < 0 {
                    return Err(io(std::io::Error::last_os_error()));
                }
                // SAFETY: openat returned a fresh owned descriptor.
                current = unsafe { File::from_raw_fd(fd) };
            }
            _ => return Err(failure(InventoryError::InvalidArgument)),
        }
    }
    Ok(current)
}
pub(super) fn open(request: &OpenInventoryRequest) -> InventoryResult<(File, File, bool)> {
    open_with_access(request, false)
}
/// Pins and validates an existing database without creating files or requiring write access.
pub(super) fn open_readonly(request: &OpenInventoryRequest) -> InventoryResult<(File, File, bool)> {
    if request.create_if_missing {
        return Err(failure(InventoryError::InvalidArgument));
    }
    open_with_access(request, true)
}
fn open_with_access(
    request: &OpenInventoryRequest,
    readonly: bool,
) -> InventoryResult<(File, File, bool)> {
    let root = directory(&request.local_root)?;
    let pinned = format!("/proc/self/fd/{}", root.as_raw_fd());
    let reserved = Path::new(&pinned).join(".drivesync");
    let exists = reserved
        .join("inventory.sqlite3")
        .symlink_metadata()
        .is_ok();
    if !exists {
        if !request.create_if_missing {
            return Err(failure(InventoryError::NotFound));
        }
        for entry in fs::read_dir(&pinned).map_err(io)? {
            if entry.map_err(io)?.file_name() != ".drivesync" {
                return Err(failure(InventoryError::AlreadyExists));
            }
        }
        match fs::DirBuilder::new().mode(0o700).create(&reserved) {
            Ok(()) => {
                root.sync_all().map_err(io)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(io(error)),
        }
    }
    let name = c".drivesync";
    // SAFETY: root descriptor and literal component are valid; no symlink is followed.
    let fd = unsafe {
        libc::openat(
            root.as_raw_fd(),
            name.as_ptr(),
            libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_RDONLY,
        )
    };
    if fd < 0 {
        return Err(io(std::io::Error::last_os_error()));
    }
    // SAFETY: openat returned a fresh owned directory descriptor.
    let dir = unsafe { File::from_raw_fd(fd) };
    let metadata = dir.metadata().map_err(io)?;
    // SAFETY: geteuid has no pointer arguments or side effects.
    let owner = unsafe { libc::geteuid() };
    if metadata.uid() != owner || metadata.mode() & 0o077 != 0 {
        return Err(failure(InventoryError::PermissionDenied));
    }
    let path = format!("/proc/self/fd/{}/inventory.sqlite3", dir.as_raw_fd());
    let open_existing = || {
        OpenOptions::new()
            .read(true)
            .write(!readonly)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(&path)
    };
    let (file, created) = if request.create_if_missing {
        match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)
        {
            Ok(file) => (file, true),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                (open_existing().map_err(io)?, false)
            }
            Err(error) => return Err(io(error)),
        }
    } else {
        (open_existing().map_err(io)?, false)
    };
    let metadata = file.metadata().map_err(io)?;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != owner
        || metadata.mode() & 0o077 != 0
    {
        return Err(failure(InventoryError::PermissionDenied));
    }
    // SAFETY: flock uses this owned regular-file descriptor and no pointers.
    if unsafe {
        libc::flock(
            file.as_raw_fd(),
            (if readonly {
                libc::LOCK_SH
            } else {
                libc::LOCK_EX
            }) | libc::LOCK_NB,
        )
    } != 0
    {
        return Err(io(std::io::Error::last_os_error()));
    }
    Ok((dir, file, created))
}
