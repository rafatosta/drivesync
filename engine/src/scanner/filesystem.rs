//! Descriptor-bound traversal and collection, independent of detector state and notifications.
use crate::{filesystem::metadata, models::*};
use std::{
    ffi::CString,
    fs::{File, Metadata, ReadDir},
    io,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{ffi::OsStrExt, fs::MetadataExt},
    },
    path::{Component, Path},
};
fn open(parent: &File, name: &[u8], directory: bool) -> io::Result<File> {
    let name = CString::new(name).map_err(|_| io::Error::from_raw_os_error(libc::EINVAL))?;
    // SAFETY: owned parent descriptor and NUL-terminated component; no creation or symlink following.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_PATH
                | libc::O_NOFOLLOW
                | libc::O_CLOEXEC
                | if directory { libc::O_DIRECTORY } else { 0 },
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: openat returned a fresh owned descriptor.
    Ok(unsafe { File::from_raw_fd(fd) })
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Stamp {
    dev: u64,
    ino: u64,
    mode: u32,
    modified: (i64, i64),
    changed: (i64, i64),
    size: u64,
}
impl From<&Metadata> for Stamp {
    fn from(m: &Metadata) -> Self {
        Self {
            dev: m.dev(),
            ino: m.ino(),
            mode: m.mode(),
            modified: (m.mtime(), m.mtime_nsec()),
            changed: (m.ctime(), m.ctime_nsec()),
            size: m.size(),
        }
    }
}
fn identity(a: &File, b: &File) -> io::Result<bool> {
    let a = a.metadata()?;
    let b = b.metadata()?;
    Ok((a.dev(), a.ino(), a.mode() & libc::S_IFMT) == (b.dev(), b.ino(), b.mode() & libc::S_IFMT))
}
/// Every ancestry component remains pinned until it has been revalidated.
pub(super) struct Entry {
    files: Vec<File>,
    names: Vec<Vec<u8>>,
}
impl Entry {
    pub fn file(&self) -> &File {
        self.files.last().expect("entry chain contains root")
    }
    pub fn validate(&self) -> io::Result<()> {
        for (i, name) in self.names.iter().enumerate() {
            let actual = open(&self.files[i], name, false)?;
            if !identity(&actual, &self.files[i + 1])? {
                return Err(io::Error::from_raw_os_error(libc::ESTALE));
            }
        }
        Ok(())
    }
    pub fn metadata(&self) -> io::Result<Metadata> {
        self.file().metadata()
    }
    pub fn children(&self) -> io::Result<ReadDir> {
        std::fs::read_dir(format!("/proc/self/fd/{}", self.file().as_raw_fd()))
    }
    pub fn stamp(&self) -> io::Result<Stamp> {
        self.metadata().map(|m| Stamp::from(&m))
    }
}
/// Root inode and absolute ancestry are bound at construction, not inferred from a path prefix.
pub(super) struct Root {
    chain: Entry,
    mount: u64,
}
impl Root {
    pub fn new(path: &Path) -> io::Result<Self> {
        if !path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, Component::ParentDir) || c.as_os_str() == ".drivesync")
        {
            return Err(io::Error::from_raw_os_error(libc::EINVAL));
        }
        let mut chain = Entry {
            files: vec![File::open("/")?],
            names: Vec::new(),
        };
        for c in path.components() {
            if let Component::Normal(c) = c {
                let next = open(chain.file(), c.as_bytes(), true)?;
                chain.names.push(c.as_bytes().to_vec());
                chain.files.push(next);
            }
        }
        chain.validate()?;
        let mount = mount(chain.file())?;
        Ok(Self { chain, mount })
    }
    pub fn validate(&self) -> io::Result<()> {
        self.chain.validate()
    }
    pub fn entry(&self, path: &[u8]) -> io::Result<Entry> {
        self.validate()?;
        let mut chain = Entry {
            files: vec![self.chain.file().try_clone()?],
            names: Vec::new(),
        };
        if !path.is_empty() {
            LocalRelativePath::new(path.to_vec())
                .map_err(|_| io::Error::from_raw_os_error(libc::EINVAL))?;
            for part in path.split(|c| *c == b'/') {
                if part == b".drivesync" {
                    return Err(io::Error::from_raw_os_error(libc::EINVAL));
                }
                let file = open(chain.file(), part, false)?;
                if mount(&file)? != self.mount {
                    return Err(io::Error::from_raw_os_error(libc::EXDEV));
                }
                chain.names.push(part.to_vec());
                chain.files.push(file);
            }
        }
        chain.validate()?;
        Ok(chain)
    }
}
/// Linux mount identity also catches bind mounts on the same device. Unknown identity is not permission to cross.
fn mount(file: &File) -> io::Result<u64> {
    let mut stat = std::mem::MaybeUninit::<libc::statx>::zeroed();
    // SAFETY: live descriptor, literal empty path and valid writable result; no path is followed.
    let result = unsafe {
        libc::statx(
            file.as_raw_fd(),
            c"".as_ptr(),
            libc::AT_EMPTY_PATH | libc::AT_SYMLINK_NOFOLLOW,
            libc::STATX_MNT_ID,
            stat.as_mut_ptr(),
        )
    };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: statx succeeded and the initialized structure is safe to read; check its mask.
    let stat = unsafe { stat.assume_init() };
    if stat.stx_mask & libc::STATX_MNT_ID == 0 {
        return Err(io::Error::from_raw_os_error(libc::EOPNOTSUPP));
    }
    Ok(stat.stx_mnt_id)
}
/// Reads the symlink itself through its O_PATH descriptor, including non-UTF8 or broken targets.
fn target(file: &File) -> io::Result<Vec<u8>> {
    let mut bytes = vec![0u8; 128];
    loop {
        // SAFETY: descriptor and empty path identify the link itself; the full buffer is writable.
        let n = unsafe {
            libc::readlinkat(
                file.as_raw_fd(),
                c"".as_ptr(),
                bytes.as_mut_ptr().cast(),
                bytes.len(),
            )
        };
        if n < 0 {
            return Err(io::Error::last_os_error());
        }
        let n = n as usize;
        if n < bytes.len() {
            bytes.truncate(n);
            return Ok(bytes);
        }
        if bytes.len() >= 65536 {
            return Err(io::Error::from_raw_os_error(libc::EOVERFLOW));
        }
        bytes.resize(bytes.len() * 2, 0);
    }
}
/// Syscall boundary shared by production collection and isolated fault injection.
pub(super) type NativeQuery = fn(&File) -> io::Result<libc::statx>;
/// Result None means the entry changed during collection, never confirmed absence.
pub(super) fn collect(
    entry: &Entry,
    path: LocalRelativePath,
    native: NativeQuery,
) -> io::Result<Option<LocalFile>> {
    entry.validate()?;
    let before = entry.metadata()?;
    let mut file = match native(entry.file()) {
        Ok(stat) => metadata::from_statx(&stat, &before, path),
        Err(error) => metadata::from_fstat(&before, path, &error),
    }
    .map_err(|_| io::Error::from_raw_os_error(libc::EINVAL))?;
    if before.file_type().is_symlink() {
        file.symlink.target = match target(entry.file()) {
            Ok(bytes) => MetadataValue::Known(bytes),
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                MetadataValue::PermissionDenied
            }
            Err(_) => MetadataValue::Unavailable,
        };
    }
    entry.validate()?;
    let after = entry.metadata()?;
    if Stamp::from(&before) != Stamp::from(&after) {
        return Ok(None);
    }
    file.validate()
        .map_err(|_| io::Error::from_raw_os_error(libc::EINVAL))?;
    Ok(Some(file))
}
pub(super) fn reserved(path: &[u8]) -> bool {
    path.split(|c| *c == b'/').any(|c| c == b".drivesync")
}
