//! Bounded hashing of an already-open regular file.
//!
//! This primitive does not resolve paths or establish membership in a root.
//! A scanner must open the descriptor safely and verify that the path still
//! identifies the observed file before publishing an inventory.

use super::LocalMetadata;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, Metadata},
    io::{Read, Seek, SeekFrom},
    os::unix::fs::MetadataExt,
};

/// A content digest and the metadata observed during a stable read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HashedLocalFile {
    /// Metadata observed before and after the content read.
    pub metadata: LocalMetadata,
    /// SHA-256 digest of the bytes read.
    pub sha256: [u8; 32],
}

/// A file could not be safely hashed; no digest is returned.
#[derive(Debug)]
pub enum LocalHashError {
    /// The descriptor does not refer to a regular file.
    NotRegularFile,
    /// The configured maximum number of content bytes would be exceeded.
    LimitExceeded,
    /// Identity, size or relevant metadata changed while reading.
    Changed,
    /// An I/O operation failed.
    Io(std::io::Error),
}

impl std::fmt::Display for LocalHashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotRegularFile => f.write_str("The descriptor is not a regular file."),
            Self::LimitExceeded => f.write_str("The file exceeds the configured read limit."),
            Self::Changed => f.write_str("The file changed while being read."),
            Self::Io(_) => f.write_str("The file could not be read."),
        }
    }
}

impl std::error::Error for LocalHashError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for LocalHashError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Hashes an already-open regular file from its beginning, up to `max_bytes`.
///
/// The caller must open the descriptor without following links and establish
/// root membership separately. The consumed descriptor's file offset changes;
/// reading may update access time. Comparing metadata before and after detects
/// many concurrent modifications but is not a filesystem snapshot or proof
/// that the file's directory entry still refers to it.
///
/// # Errors
///
/// Returns [`LocalHashError`] for non-regular files, resource limits, changed
/// metadata, or read/seek errors. A failure never returns a partial digest.
pub fn hash_local_regular_file(
    file: File,
    max_bytes: u64,
) -> Result<HashedLocalFile, LocalHashError> {
    hash_with(file, max_bytes, || {})
}

fn hash_with(
    mut file: File,
    max_bytes: u64,
    after_read: impl FnOnce(),
) -> Result<HashedLocalFile, LocalHashError> {
    let before = file.metadata()?;
    if !before.is_file() {
        return Err(LocalHashError::NotRegularFile);
    }
    if before.len() > max_bytes {
        return Err(LocalHashError::LimitExceeded);
    }
    file.seek(SeekFrom::Start(0))?;
    let mut digest = Sha256::new();
    let mut bytes_read = 0_u64;
    let mut buffer = [0_u8; 65536];
    loop {
        let remaining = max_bytes - bytes_read;
        if remaining == 0 {
            let mut probe = [0_u8; 1];
            if file.read(&mut probe)? != 0 {
                return Err(LocalHashError::LimitExceeded);
            }
            break;
        }
        let len = remaining.min(buffer.len() as u64) as usize;
        let count = file.read(&mut buffer[..len])?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
        bytes_read += count as u64;
    }
    after_read();
    let after = file.metadata()?;
    if bytes_read != before.len() || !same_version(&before, &after) {
        return Err(LocalHashError::Changed);
    }
    Ok(HashedLocalFile {
        metadata: local_metadata(&before),
        sha256: digest.finalize().into(),
    })
}

fn same_version(first: &Metadata, second: &Metadata) -> bool {
    first.dev() == second.dev()
        && first.ino() == second.ino()
        && first.mode() == second.mode()
        && first.uid() == second.uid()
        && first.gid() == second.gid()
        && first.nlink() == second.nlink()
        && first.len() == second.len()
        && first.mtime() == second.mtime()
        && first.mtime_nsec() == second.mtime_nsec()
        && first.ctime() == second.ctime()
        && first.ctime_nsec() == second.ctime_nsec()
}

fn local_metadata(metadata: &Metadata) -> LocalMetadata {
    LocalMetadata {
        device: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode(),
        size: metadata.len(),
        modified_ns: i128::from(metadata.mtime()) * 1_000_000_000
            + i128::from(metadata.mtime_nsec()),
        changed_ns: i128::from(metadata.ctime()) * 1_000_000_000
            + i128::from(metadata.ctime_nsec()),
    }
}

#[cfg(test)]
mod tests;
