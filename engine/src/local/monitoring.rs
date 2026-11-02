//! Bounded descriptor-relative directory discovery for local event monitors.
//!
//! These descriptors are observations only; synchronization revalidates the
//! authorized association independently and never treats them as inventory.
use crate::{
    download::filesystem::{self, Root},
    AuthError, Cancellation, EngineError,
};
use std::{
    fs::File,
    os::{fd::BorrowedFd, unix::ffi::OsStrExt},
    path::{Path, PathBuf},
};

const DIRECTORY_LIMIT: usize = 4096;
const ENTRY_LIMIT: usize = 100_000;

/// A failed discovery or revalidation; none of these means an empty tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonitorError {
    /// The caller requested cancellation.
    Cancelled,
    /// The root or an ancestor is inaccessible.
    Unavailable,
    /// A directory or the root changed while it was being observed.
    Changed,
    /// The configured directory or entry budget was exceeded.
    Limit,
    /// A filesystem operation failed.
    Io,
}

fn classify(error: EngineError) -> MonitorError {
    match error {
        EngineError::Authentication(AuthError::Cancelled) => MonitorError::Cancelled,
        EngineError::LocalAccessDenied | EngineError::RootSuspended(_) => MonitorError::Unavailable,
        EngineError::DownloadConflict | EngineError::InvalidFolder | EngineError::InvalidPlan => {
            MonitorError::Changed
        }
        _ => MonitorError::Io,
    }
}

fn check(cancel: &Cancellation) -> Result<(), MonitorError> {
    super::check(cancel).map_err(classify)
}

fn filesystem_error(error: rustix::io::Errno) -> MonitorError {
    match error {
        rustix::io::Errno::LOOP | rustix::io::Errno::NOTDIR | rustix::io::Errno::NOENT => {
            MonitorError::Changed
        }
        _ => classify(filesystem::io(error)),
    }
}

/// A held directory descriptor and its relative path in the observed root.
///
/// The descriptor anchors the inode, but does not prove that the path still
/// belongs to the root. Revalidate the tree before claiming coverage.
pub struct MonitorDirectory {
    relative: PathBuf,
    file: File,
}

impl MonitorDirectory {
    /// Returns the path relative to the observed root; the root itself is empty.
    pub fn relative(&self) -> &Path {
        &self.relative
    }

    /// Borrows the held directory descriptor for the lifetime of this entry.
    pub fn descriptor(&self) -> BorrowedFd<'_> {
        use std::os::fd::AsFd;
        self.file.as_fd()
    }
}

/// A complete bounded discovery; holding it preserves its directory descriptors.
pub struct MonitorTree {
    root: Root,
    directories: Vec<MonitorDirectory>,
}

impl MonitorTree {
    pub(crate) fn discover(path: &Path, cancel: &Cancellation) -> Result<Self, MonitorError> {
        check(cancel)?;
        let root = Root::observe(path).map_err(classify)?;
        root.validate().map_err(classify)?;
        let root_file = root.file.try_clone().map_err(|_| MonitorError::Io)?;
        let mut directories = vec![MonitorDirectory {
            relative: PathBuf::new(),
            file: root_file,
        }];
        let mut next = 0;
        let mut entries = 0;
        while next < directories.len() {
            check(cancel)?;
            root.validate().map_err(classify)?;
            let directory = &directories[next];
            let relative = directory.relative.clone();
            let mut children = Vec::new();
            for entry in rustix::fs::Dir::read_from(&directory.file).map_err(filesystem_error)? {
                check(cancel)?;
                let entry = entry.map_err(filesystem_error)?;
                let name = std::ffi::OsStr::from_bytes(entry.file_name().to_bytes());
                if name == "." || name == ".." {
                    continue;
                }
                entries += 1;
                if entries > ENTRY_LIMIT {
                    return Err(MonitorError::Limit);
                }
                let path = relative.join(name);
                if super::ignored(&path) || super::temporary(&path) {
                    continue;
                }
                let stat = rustix::fs::statat(
                    &directory.file,
                    name,
                    rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
                )
                .map_err(filesystem_error)?;
                if rustix::fs::FileType::from_raw_mode(stat.st_mode)
                    != rustix::fs::FileType::Directory
                {
                    continue;
                }
                if directories.len() + children.len() >= DIRECTORY_LIMIT {
                    return Err(MonitorError::Limit);
                }
                let file = rustix::fs::openat(
                    &directory.file,
                    name,
                    rustix::fs::OFlags::RDONLY
                        | rustix::fs::OFlags::DIRECTORY
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::empty(),
                )
                .map(File::from)
                .map_err(filesystem_error)?;
                children.push(MonitorDirectory {
                    relative: path,
                    file,
                });
            }
            directories.extend(children);
            next += 1;
        }
        let tree = Self { root, directories };
        tree.validate(cancel)?;
        Ok(tree)
    }

    /// Returns the complete set of discovered directories.
    pub fn directories(&self) -> &[MonitorDirectory] {
        &self.directories
    }

    /// Revalidates root and all held directory identities against their paths.
    ///
    /// Call after installing monitors and off the GTK main thread. A successful
    /// check does not guarantee that the tree cannot change immediately after.
    ///
    /// # Errors
    ///
    /// Returns a typed error on cancellation, I/O failure or changed paths.
    pub fn validate(&self, cancel: &Cancellation) -> Result<(), MonitorError> {
        for directory in &self.directories {
            check(cancel)?;
            self.root.validate().map_err(classify)?;
            if directory.relative.as_os_str().is_empty() {
                continue;
            }
            let (parent, name) = self.root.parent(&directory.relative).map_err(classify)?;
            let current = rustix::fs::openat(
                &parent,
                &name,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map(File::from)
            .map_err(filesystem_error)?;
            let expected = filesystem::identity(&directory.file).map_err(classify)?;
            if filesystem::identity(&current).map_err(classify)? != expected {
                return Err(MonitorError::Changed);
            }
            self.root
                .validate_parent(
                    &directory.relative,
                    filesystem::identity(&parent).map_err(classify)?,
                )
                .map_err(classify)?;
        }
        Ok(())
    }
}
