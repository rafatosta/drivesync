//! Read-only descriptor-anchored local inventories. A snapshot never creates a sync baseline.
use super::{
    remote::interfaces::CancellationToken, LocalObservation, LocalObservationSource, Observation,
    ObservationCoverage,
};
use crate::models::*;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::Read,
    os::{
        fd::AsRawFd,
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt},
        },
    },
    path::{Component, Path as StdPath, PathBuf},
};
const MAX_ITEMS: usize = 100_000;
const MAX_DEPTH: usize = 128;
/// Safe coverage failures never authorize inferred absence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScanError {
    Unavailable,
    PermissionDenied,
    RootChanged,
    Cancelled,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Bounded diagnostic categories, without raw I/O messages.
pub(crate) enum ScanIssueKind {
    Unreadable,
    Unstable,
    LimitReached,
}
#[derive(Debug, Clone)]
/// One path whose capture could not be trusted.
pub(crate) struct ScanIssue {
    pub path: Path,
    pub kind: ScanIssueKind,
}
/// A scoped read result. Only Complete permits comparison for missing local paths.
pub(crate) struct ScanResult {
    pub entries: BTreeMap<Path, LocalObservation>,
    pub coverage: ObservationCoverage,
    pub issues: Vec<ScanIssue>,
}
/// Pins the configured root and rejects later path substitution.
pub(crate) struct Scanner {
    root: File,
    absolute: PathBuf,
    sync_root_id: SyncRootId,
    max_items: usize,
}
fn io_error(error: std::io::Error) -> ScanError {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        ScanError::PermissionDenied
    } else {
        ScanError::Unavailable
    }
}
fn open_directory(path: &StdPath) -> Result<File, ScanError> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(io_error)
}
fn descriptor(file: &File) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()))
}
fn root(path: &StdPath) -> Result<File, ScanError> {
    if !path.is_absolute() {
        return Err(ScanError::Unavailable);
    }
    let mut dir = open_directory(StdPath::new("/"))?;
    for part in path.components().skip(1) {
        let Component::Normal(name) = part else {
            return Err(ScanError::Unavailable);
        };
        dir = open_directory(&descriptor(&dir).join(name))?;
    }
    Ok(dir)
}
impl Scanner {
    /// Opens each root component without following symlinks, and pins its identity.
    pub(crate) fn open(path: PathBuf, sync_root_id: SyncRootId) -> Result<Self, ScanError> {
        Ok(Self {
            root: root(&path)?,
            absolute: path,
            sync_root_id,
            max_items: MAX_ITEMS,
        })
    }
    /// Raise a soft collection budget on the next complete retry; hard exhaustion remains explicit Partial.
    pub(crate) fn expand_limit(&mut self) -> bool {
        let before = self.max_items;
        self.max_items = self.max_items.saturating_mul(2).min(1_600_000);
        self.max_items > before
    }
    fn directory(&self, path: &StdPath) -> Result<File, ScanError> {
        let mut current = self.root.try_clone().map_err(io_error)?;
        for part in path.components() {
            let Component::Normal(name) = part else {
                return Err(ScanError::Unavailable);
            };
            current = open_directory(&descriptor(&current).join(name))?;
        }
        Ok(current)
    }
    /// Reads native paths and SHA-256 in bounded buffers, checking cancellation and stability.
    pub(crate) fn scan(&self, cancel: &CancellationToken) -> Result<ScanResult, ScanError> {
        if cancel.is_cancelled() {
            return Err(ScanError::Cancelled);
        }
        let current = root(&self.absolute)?;
        let expected = self.root.metadata().map_err(io_error)?;
        let actual = current.metadata().map_err(io_error)?;
        if (expected.dev(), expected.ino()) != (actual.dev(), actual.ino()) {
            return Err(ScanError::RootChanged);
        }
        let mut result = ScanResult {
            entries: BTreeMap::new(),
            coverage: ObservationCoverage::Complete,
            issues: Vec::new(),
        };
        let mut pending = vec![(PathBuf::new(), 0, (expected.dev(), expected.ino()))];
        while let Some((parent, depth, identity)) = pending.pop() {
            let directory = match self.directory(&parent) {
                Ok(directory) => directory,
                Err(_) => {
                    issue(&mut result, parent, ScanIssueKind::Unreadable);
                    continue;
                }
            };
            let actual = directory.metadata().map_err(io_error)?;
            if (actual.dev(), actual.ino()) != identity {
                issue(&mut result, parent, ScanIssueKind::Unstable);
                continue;
            }
            if cancel.is_cancelled() {
                return Err(ScanError::Cancelled);
            }
            let before = directory.metadata().map_err(io_error)?;
            let listing = match std::fs::read_dir(descriptor(&directory)) {
                Ok(listing) => listing,
                Err(_) => {
                    issue(&mut result, parent, ScanIssueKind::Unreadable);
                    continue;
                }
            };
            for entry in listing {
                if cancel.is_cancelled() {
                    return Err(ScanError::Cancelled);
                }
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(_) => {
                        issue(&mut result, parent.clone(), ScanIssueKind::Unreadable);
                        continue;
                    }
                };
                let name = entry.file_name();
                if (parent.as_os_str().is_empty() && name == ".drivesync") || ignored(&name) {
                    continue;
                }
                let path = parent.join(&name);
                if result.entries.len() >= self.max_items {
                    issue(&mut result, path, ScanIssueKind::LimitReached);
                    pending.clear();
                    break;
                }
                let anchored = descriptor(&directory).join(&name);
                let meta = match std::fs::symlink_metadata(&anchored) {
                    Ok(meta) => meta,
                    Err(_) => {
                        issue(&mut result, path, ScanIssueKind::Unreadable);
                        continue;
                    }
                };
                let kind = if meta.is_file() {
                    FileKind::Regular
                } else if meta.is_dir() {
                    FileKind::Directory
                } else if meta.file_type().is_symlink() {
                    FileKind::Symlink
                } else {
                    FileKind::Unsupported
                };
                let mut checksum = Evidence::NotApplicable;
                let mut can_read = Evidence::NotApplicable;
                if kind == FileKind::Regular {
                    match hash(&anchored, &meta, cancel) {
                        Ok(digest) => {
                            checksum = Evidence::Known(digest);
                            can_read = Evidence::Known(true);
                        }
                        Err(ScanError::Cancelled) => return Err(ScanError::Cancelled),
                        Err(ScanError::PermissionDenied) => {
                            checksum = Evidence::PermissionDenied;
                            can_read = Evidence::Known(false);
                            issue(&mut result, path.clone(), ScanIssueKind::Unreadable);
                        }
                        Err(_) => {
                            checksum = Evidence::Unavailable;
                            can_read = Evidence::Unavailable;
                            issue(&mut result, path.clone(), ScanIssueKind::Unstable);
                        }
                    }
                }
                let observed_at = now();
                result.entries.insert(
                    path.clone(),
                    LocalObservation {
                        sync_root_id: self.sync_root_id.clone(),
                        state: FileState::Present,
                        path: Some(path.clone()),
                        previous_path: None,
                        name: name
                            .to_str()
                            .map(|name| Evidence::Known(name.to_owned()))
                            .unwrap_or(Evidence::Unsupported),
                        parent: Evidence::Known(parent.clone()),
                        kind: Evidence::Known(kind),
                        identity: Evidence::Known(LocalIdentity {
                            device_id: meta.dev(),
                            inode: meta.ino(),
                            mount_id: None,
                            birth_ns: crate::models::birth_ns(&meta),
                        }),
                        size: if kind == FileKind::Regular {
                            Evidence::Known(meta.len())
                        } else {
                            Evidence::NotApplicable
                        },
                        modified_at: Evidence::Known(DateTime {
                            unix_millis: meta.mtime() * 1000 + meta.mtime_nsec() / 1_000_000,
                        }),
                        changed_at: Evidence::Known(DateTime {
                            unix_millis: meta.ctime() * 1000 + meta.ctime_nsec() / 1_000_000,
                        }),
                        checksum,
                        capabilities: Evidence::Known(LocalCapabilities {
                            can_read,
                            can_write: access(&anchored, libc::W_OK),
                            // Check the directory reached by our pinned descriptor, not the procfs symlink.
                            can_rename: access(
                                &descriptor(&directory).join("."),
                                libc::W_OK | libc::X_OK,
                            ),
                            can_delete: Evidence::NotCollected,
                        }),
                        observed_at,
                        coverage: ObservationCoverage::Complete,
                        event: Some(ObservationEvent {
                            origin: ObservationOrigin::Scanner,
                            kind: ObservationKind::Snapshot,
                            observed_at,
                            old_path: None,
                            new_path: Some(path.clone()),
                            remote_id: None,
                            move_cookie: None,
                        }),
                    },
                );
                if kind == FileKind::Directory {
                    if depth >= MAX_DEPTH {
                        issue(&mut result, path, ScanIssueKind::LimitReached);
                    } else {
                        // Queue names and observed identities, not open directory descriptors.
                        // Wide trees therefore cannot exhaust the process descriptor limit.
                        pending.push((path, depth + 1, (meta.dev(), meta.ino())));
                    }
                }
            }
            let after = directory.metadata().map_err(io_error)?;
            if stamp(&before) != stamp(&after) {
                issue(&mut result, parent, ScanIssueKind::Unstable);
            }
        }
        let current = root(&self.absolute)?.metadata().map_err(io_error)?;
        if (current.dev(), current.ino()) != (expected.dev(), expected.ino()) {
            return Err(ScanError::RootChanged);
        }
        if result.coverage != ObservationCoverage::Complete {
            for entry in result.entries.values_mut() {
                entry.coverage = result.coverage;
            }
        }
        Ok(result)
    }
}
pub(crate) fn now() -> DateTime {
    crate::orchestrator::timestamp()
}
pub(crate) fn ignored(name: &std::ffi::OsStr) -> bool {
    name.as_bytes().starts_with(b".drivesync-transfer-")
        || crate::projections::private_trash_name(name)
}
fn issue(result: &mut ScanResult, path: Path, kind: ScanIssueKind) {
    result.coverage = ObservationCoverage::Partial;
    if result.issues.len() < 256 {
        result.issues.push(ScanIssue { path, kind });
    }
}
fn stamp(meta: &std::fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        meta.dev(),
        meta.ino(),
        meta.len(),
        meta.mtime(),
        meta.mtime_nsec(),
        meta.ctime(),
        meta.ctime_nsec(),
    )
}
fn hash(
    path: &StdPath,
    expected: &std::fs::Metadata,
    cancel: &CancellationToken,
) -> Result<Checksum, ScanError> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(io_error)?;
    let before = file.metadata().map_err(io_error)?;
    if !before.is_file() || stamp(expected) != stamp(&before) {
        return Err(ScanError::Unavailable);
    }
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        if cancel.is_cancelled() {
            return Err(ScanError::Cancelled);
        }
        let count = file.read(&mut buffer).map_err(io_error)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    let after = file.metadata().map_err(io_error)?;
    let named = std::fs::symlink_metadata(path).map_err(io_error)?;
    if stamp(&before) != stamp(&after) || stamp(&after) != stamp(&named) {
        return Err(ScanError::Unavailable);
    }
    Ok(Checksum {
        algorithm: "sha256".into(),
        value: format!("{:x}", hash.finalize()),
    })
}
fn access(path: &StdPath, mode: i32) -> Evidence<bool> {
    use std::ffi::CString;
    let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
        return Evidence::Unavailable;
    };
    if unsafe {
        libc::faccessat(
            libc::AT_FDCWD,
            path.as_ptr(),
            mode,
            libc::AT_EACCESS | libc::AT_SYMLINK_NOFOLLOW,
        )
    } == 0
    {
        return Evidence::Known(true);
    }
    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::EACCES | libc::EPERM | libc::EROFS) => Evidence::Known(false),
        _ => Evidence::Unavailable,
    }
}

impl ScanResult {
    pub(crate) fn observations(&self, source: LocalObservationSource) -> Vec<Observation> {
        self.entries
            .values()
            .cloned()
            .map(|mut data| {
                if let Some(event) = &mut data.event {
                    event.origin = if source == LocalObservationSource::Scanner {
                        ObservationOrigin::Scanner
                    } else {
                        ObservationOrigin::Local
                    };
                }
                Observation::Local { source, data }
            })
            .collect()
    }
}
#[cfg(test)]
pub(in crate::detectors) mod tests;
