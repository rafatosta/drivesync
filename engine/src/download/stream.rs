//! Bounded streaming with provider MD5 and local SHA-256 evidence.
use super::filesystem::io;
use crate::{AuthError, Cancellation, EngineError};
use md5::{Digest, Md5};
use sha2::Sha256;
use std::{
    fs::File,
    io::{Read, Write},
    os::unix::fs::MetadataExt,
};

pub(super) fn cancelled(cancel: &Cancellation) -> Result<(), EngineError> {
    if cancel.is_cancelled() {
        Err(EngineError::Authentication(AuthError::Cancelled))
    } else {
        Ok(())
    }
}
fn signature(metadata: &std::fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    )
}
/// Reads at most expected+1 bytes; writes only bounded verified-size prefixes.
/// The caller owns staging and does not expose it as the final file on failure.
pub(crate) fn copy(
    input: &mut dyn Read,
    output: &mut dyn Write,
    expected: u64,
    md5: &str,
    cancel: &Cancellation,
    mut progress: impl FnMut(u64),
    mut check: impl FnMut() -> Result<(), EngineError>,
) -> Result<String, EngineError> {
    let mut buffer = [0u8; 65536];
    let mut bytes = 0u64;
    let mut provider = Md5::new();
    let mut local = Sha256::new();
    loop {
        cancelled(cancel)?;
        let limit = expected
            .saturating_sub(bytes)
            .saturating_add(1)
            .min(buffer.len() as u64) as usize;
        let n = input
            .read(&mut buffer[..limit])
            .map_err(|_| EngineError::Authentication(AuthError::Network))?;
        cancelled(cancel)?;
        if n == 0 {
            break;
        }
        bytes = bytes
            .checked_add(n as u64)
            .ok_or(EngineError::DownloadIntegrity)?;
        if bytes > expected {
            return Err(EngineError::DownloadIntegrity);
        }
        check()?;
        output.write_all(&buffer[..n]).map_err(io)?;
        provider.update(&buffer[..n]);
        local.update(&buffer[..n]);
        progress(bytes);
    }
    if bytes != expected || !format!("{:x}", provider.finalize()).eq_ignore_ascii_case(md5) {
        return Err(EngineError::DownloadIntegrity);
    }
    Ok(format!("{:x}", local.finalize()))
}
/// Hashes one open regular file and rejects changes observed while it is read.
pub(crate) fn fingerprint(file: &mut File, cancel: &Cancellation) -> Result<String, EngineError> {
    let before = file.metadata().map_err(io)?;
    if !before.is_file() {
        return Err(EngineError::DownloadConflict);
    }
    let mut hash = Sha256::new();
    let mut bytes = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        cancelled(cancel)?;
        let n = file.read(&mut buffer).map_err(io)?;
        cancelled(cancel)?;
        if n == 0 {
            break;
        }
        bytes = bytes
            .checked_add(n as u64)
            .ok_or(EngineError::DownloadConflict)?;
        if bytes > before.len() {
            return Err(EngineError::DownloadConflict);
        }
        hash.update(&buffer[..n]);
    }
    let after = file.metadata().map_err(io)?;
    if bytes != before.len() || signature(&before) != signature(&after) {
        return Err(EngineError::DownloadConflict);
    }
    Ok(format!("{:x}", hash.finalize()))
}
/// Fingerprints the open regular file and rejects metadata changes during reading.
pub(crate) fn verify(
    file: &mut File,
    size: u64,
    sha256: &str,
    cancel: &Cancellation,
) -> Result<bool, EngineError> {
    let before = file.metadata().map_err(io)?;
    if !before.is_file() || before.len() != size {
        return Ok(false);
    }
    let mut remaining = size;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    while remaining > 0 {
        cancelled(cancel)?;
        let limit = remaining.min(buffer.len() as u64) as usize;
        let n = file.read(&mut buffer[..limit]).map_err(io)?;
        if n == 0 {
            return Ok(false);
        }
        remaining -= n as u64;
        hash.update(&buffer[..n]);
    }
    let after = file.metadata().map_err(io)?;
    Ok(signature(&before) == signature(&after) && format!("{:x}", hash.finalize()) == sha256)
}
