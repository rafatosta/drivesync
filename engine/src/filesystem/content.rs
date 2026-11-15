//! Read-only, descriptor-relative SHA-256 measurements bound to an observed file.
use crate::models::*;
use sha2::{Digest, Sha256};
use std::{
    ffi::CString,
    fs::File,
    io::{self, Read},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt},
        },
    },
    path::Path,
};

/// Refuses symlink traversal and stale/unstable observations. No file is written.
/// Missing evidence or an unstable writer yields None, never an invented checksum.
pub(crate) fn sha256(root: &Path, observed: &LocalFile) -> Option<String> {
    fn read(root: &Path, o: &LocalFile) -> io::Result<String> {
        let mut directory = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(root)?;
        let parts = o
            .relative_path
            .as_bytes()
            .split(|b| *b == b'/')
            .collect::<Vec<_>>();
        let mut file = None;
        for (i, part) in parts.iter().enumerate() {
            let name = CString::new(*part).map_err(|_| io::ErrorKind::InvalidInput)?;
            let flags = libc::O_RDONLY
                | libc::O_NOFOLLOW
                | libc::O_CLOEXEC
                | libc::O_NONBLOCK
                | if i + 1 < parts.len() {
                    libc::O_DIRECTORY
                } else {
                    0
                };
            // SAFETY: directory is held throughout openat; name is NUL terminated.
            let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: a successful openat returns an owned, newly allocated fd.
            let opened = unsafe { File::from_raw_fd(fd) };
            if i + 1 == parts.len() {
                file = Some(opened)
            } else {
                directory = opened;
            }
        }
        let mut file = file.ok_or(io::ErrorKind::InvalidInput)?;
        let before = file.metadata()?;
        if !before.is_file() {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let same_time = |t: &MetadataValue<LocalTimestamp>, s: i64, n: i64| matches!(t,MetadataValue::Known(v) if v.unix_seconds==s && i64::from(v.nanoseconds)==n);
        if o.storage.inode != MetadataValue::Known(before.ino())
            || o.storage.size_bytes != MetadataValue::Known(before.len())
            || !same_time(&o.times.modified_at, before.mtime(), before.mtime_nsec())
            || !same_time(&o.times.changed_at, before.ctime(), before.ctime_nsec())
        {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let device = MetadataValue::Known(DeviceNumber {
            major: libc::major(before.dev()),
            minor: libc::minor(before.dev()),
        });
        if device != o.storage.filesystem_device {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let mut hash = Sha256::new();
        let mut buffer = [0; 65536];
        let mut count = 0u64;
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            count += n as u64;
            if count > before.len() {
                return Err(io::ErrorKind::InvalidData.into());
            }
            hash.update(&buffer[..n]);
        }
        let after = file.metadata()?;
        if count != before.len()
            || before.len() != after.len()
            || before.mtime() != after.mtime()
            || before.mtime_nsec() != after.mtime_nsec()
            || before.ctime() != after.ctime()
            || before.ctime_nsec() != after.ctime_nsec()
        {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let path = root.join(std::ffi::OsStr::from_bytes(o.relative_path.as_bytes()));
        let current = std::fs::symlink_metadata(path)?;
        if current.dev() != after.dev() || current.ino() != after.ino() {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(format!("sha256:{:x}", hash.finalize()))
    }
    read(root, observed).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hashing_refuses_changed_snapshots_and_symlinks_without_writes() {
        let root = std::env::temp_dir().join(format!("content-evidence-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("file");
        std::fs::write(&path, b"original").unwrap();
        let file = File::open(&path).unwrap();
        let observed = crate::filesystem::metadata::from_statx(
            &crate::filesystem::metadata::statx(&file).unwrap(),
            &file.metadata().unwrap(),
            LocalRelativePath::new(b"file".to_vec()).unwrap(),
        )
        .unwrap();
        assert!(sha256(&root, &observed).is_some());
        std::fs::write(&path, b"different").unwrap();
        assert!(sha256(&root, &observed).is_none());
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink("/etc/passwd", &path).unwrap();
        assert!(sha256(&root, &observed).is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
}
