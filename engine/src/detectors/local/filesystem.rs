//! Descriptor-relative, no-follow Linux navigation adapted from the archived engine.
//! No file content is opened; all descriptors are O_PATH and owned through RAII.
use super::models::*;
use crate::models::LocalRelativePath;
use std::{
    ffi::{CStr, CString},
    fs::{File, Metadata},
    io,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{ffi::OsStrExt, fs::MetadataExt},
    },
    path::{Component, Path},
};

/// Maps actual syscall failures to fixed typed diagnostics without exposing paths.
pub(super) fn failure(error: io::Error, root: bool) -> LocalDetectorFailure {
    use LocalDetectorError::*;
    let kind = match error.raw_os_error() {
        Some(libc::EACCES | libc::EPERM) => PermissionDenied,
        Some(libc::EMFILE | libc::ENFILE | libc::ENOMEM | libc::ENAMETOOLONG) => ResourceLimit,
        _ if root => RootUnavailable,
        Some(libc::ENOENT | libc::ENOTDIR | libc::ELOOP | libc::ESTALE) => PartialObservation,
        _ => Internal,
    };
    LocalDetectorFailure::new(kind)
}

/// Opens a single component relative to a held directory, never following links.
pub(super) fn open_at(parent: &File, name: &CStr, directory: bool) -> io::Result<File> {
    let flags = libc::O_PATH
        | libc::O_NOFOLLOW
        | libc::O_CLOEXEC
        | if directory { libc::O_DIRECTORY } else { 0 };
    // SAFETY: parent is live, name is NUL-terminated, and flags do not create files.
    let fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: openat returned a new descriptor whose ownership is transferred once.
    Ok(unsafe { File::from_raw_fd(fd) })
}

/// Identity evidence for revalidating a held descriptor against its path.
pub(super) fn same_entry(left: &Metadata, right: &Metadata) -> bool {
    (left.dev(), left.ino(), left.mode() & libc::S_IFMT)
        == (right.dev(), right.ino(), right.mode() & libc::S_IFMT)
}

/// A held directory and the component that must still bind it to its parent.
struct Binding {
    file: File,
    name: CString,
    absolute: bool,
}

/// Keeps every traversed directory alive until its bindings have been rechecked.
pub(super) struct DirectoryChain {
    bindings: Vec<Binding>,
}
impl DirectoryChain {
    /// Opens an absolute root one no-follow component at a time, then revalidates it.
    pub(super) fn open_root(path: &Path, cancel: &LocalCancellation) -> LocalDetectorResult<Self> {
        cancel.check()?;
        // SAFETY: this literal is NUL-terminated; O_PATH opens no file contents.
        let fd = unsafe {
            libc::open(
                c"/".as_ptr(),
                libc::O_PATH | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(failure(io::Error::last_os_error(), true));
        }
        // SAFETY: fd is newly owned and valid after the success check above.
        let base = unsafe { File::from_raw_fd(fd) };
        let mut chain = Self {
            bindings: vec![Binding {
                file: base,
                name: CString::new("/").unwrap(),
                absolute: true,
            }],
        };
        for component in path.components() {
            match component {
                Component::RootDir | Component::CurDir => continue,
                Component::Normal(name) => chain.push(
                    CString::new(name.as_bytes())
                        .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::InvalidScope))?,
                    true,
                    cancel,
                )?,
                _ => return Err(LocalDetectorFailure::new(LocalDetectorError::InvalidScope)),
            }
        }
        chain.validate(cancel)?;
        Ok(chain)
    }
    fn push(
        &mut self,
        name: CString,
        absolute: bool,
        cancel: &LocalCancellation,
    ) -> LocalDetectorResult<()> {
        cancel.check()?;
        let file = open_at(self.last(), &name, true).map_err(|e| failure(e, absolute))?;
        self.bindings.push(Binding {
            file,
            name,
            absolute,
        });
        Ok(())
    }
    /// Borrows the final anchored directory; the chain always contains `/`.
    pub(super) fn last(&self) -> &File {
        &self.bindings.last().unwrap().file
    }
    /// Opens relative parents and the terminal entry, allowing only a final symlink.
    pub(super) fn open_entry(
        &mut self,
        path: &LocalRelativePath,
        cancel: &LocalCancellation,
    ) -> LocalDetectorResult<Option<(File, CString)>> {
        let parts: Vec<_> = path.as_bytes().split(|byte| *byte == b'/').collect();
        for part in &parts[..parts.len() - 1] {
            let name = CString::new(*part)
                .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::InvalidScope))?;
            cancel.check()?;
            match open_at(self.last(), &name, true) {
                Ok(file) => self.bindings.push(Binding {
                    file,
                    name,
                    absolute: false,
                }),
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(failure(error, false)),
            }
        }
        let name = CString::new(*parts.last().unwrap())
            .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::InvalidScope))?;
        cancel.check()?;
        match open_at(self.last(), &name, false) {
            Ok(file) => Ok(Some((file, name))),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(failure(error, false)),
        }
    }
    /// Rechecks every absolute/relative parent binding without assuming ancestry from fd alone.
    pub(super) fn validate(&self, cancel: &LocalCancellation) -> LocalDetectorResult<()> {
        for pair in self.bindings.windows(2) {
            cancel.check()?;
            let current = open_at(&pair[0].file, &pair[1].name, true)
                .map_err(|e| failure(e, pair[1].absolute))?;
            cancel.check()?;
            let actual = current
                .metadata()
                .map_err(|e| failure(e, pair[1].absolute))?;
            cancel.check()?;
            let expected = pair[1]
                .file
                .metadata()
                .map_err(|e| failure(e, pair[1].absolute))?;
            if !same_entry(&actual, &expected) {
                return Err(LocalDetectorFailure::new(if pair[1].absolute {
                    LocalDetectorError::RootUnavailable
                } else {
                    LocalDetectorError::PartialObservation
                }));
            }
        }
        Ok(())
    }
}

/// Rechecks the terminal entry's identity; disappearance is not confirmed deletion.
pub(super) fn entry_still_bound(
    chain: &DirectoryChain,
    name: &CStr,
    expected: &Metadata,
    cancel: &LocalCancellation,
) -> LocalDetectorResult<bool> {
    cancel.check()?;
    let current = match open_at(chain.last(), name, false) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(failure(error, false)),
    };
    cancel.check()?;
    Ok(same_entry(
        &current.metadata().map_err(|e| failure(e, false))?,
        expected,
    ))
}

/// Selected mutation-sensitive evidence; deliberately excludes read-generated atime.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Stamp {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    size: u64,
    links: u64,
    mtime: (i64, i64),
    ctime: (i64, i64),
}
impl From<&Metadata> for Stamp {
    fn from(m: &Metadata) -> Self {
        Self {
            device: m.dev(),
            inode: m.ino(),
            mode: m.mode(),
            uid: m.uid(),
            gid: m.gid(),
            size: m.size(),
            links: m.nlink(),
            mtime: (m.mtime(), m.mtime_nsec()),
            ctime: (m.ctime(), m.ctime_nsec()),
        }
    }
}

/// Reads the literal target through the O_PATH symlink fd, never through its destination.
pub(super) fn read_link(
    file: &File,
    cancel: &LocalCancellation,
) -> LocalDetectorResult<io::Result<Vec<u8>>> {
    read_link_with(cancel, |bytes| {
        // SAFETY: file is live; the initialized buffer is writable for its full length.
        // Empty path names the held symlink itself with Linux readlinkat semantics.
        let count = unsafe {
            libc::readlinkat(
                file.as_raw_fd(),
                c"".as_ptr(),
                bytes.as_mut_ptr().cast(),
                bytes.len(),
            )
        };
        if count < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(count as usize)
        }
    })
}

/// Bounded buffer growth isolated from the syscall for deterministic truncation tests.
pub(super) fn read_link_with(
    cancel: &LocalCancellation,
    mut read: impl FnMut(&mut [u8]) -> io::Result<usize>,
) -> LocalDetectorResult<io::Result<Vec<u8>>> {
    const MAX_TARGET_BYTES: usize = 64 * 1024;
    let mut bytes = vec![0u8; 128];
    loop {
        cancel.check()?;
        let count = match read(&mut bytes) {
            Ok(count) => count,
            Err(error) => return Ok(Err(error)),
        };
        cancel.check()?;
        if count < bytes.len() {
            bytes.truncate(count);
            return Ok(Ok(bytes));
        }
        if bytes.len() == MAX_TARGET_BYTES {
            return Ok(Err(io::Error::from_raw_os_error(libc::EOVERFLOW)));
        }
        bytes.resize((bytes.len() * 2).min(MAX_TARGET_BYTES), 0);
    }
}
