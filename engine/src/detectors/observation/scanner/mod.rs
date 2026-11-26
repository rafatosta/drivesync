//! Read-only descriptor-anchored local inventories. A snapshot never creates a sync baseline.
pub(super) mod activity;
mod watch;

use super::{remote::interfaces::CancellationToken, LocalObservation, ObservationCoverage};
use crate::models::*;
use sha2::{Digest, Sha256};
use std::{
    cell::{Cell, RefCell},
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
    time::{Duration, Instant},
};
const MAX_ITEMS: usize = 100_000;
const MAX_DEPTH: usize = 128;
const HASH_AUDIT_INTERVAL: Duration = Duration::from_secs(30);
type FileStamp = (u64, u64, u64, i64, i64, i64, i64);

#[derive(Clone)]
struct CachedHash {
    stamp: FileStamp,
    birth_ns: Option<u128>,
    digest: Checksum,
    checked: Instant,
}
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
    progress: Option<activity::Reporter>,
    traversal: Cell<activity::Traversal>,
    root: File,
    absolute: PathBuf,
    sync_root_id: SyncRootId,
    max_items: usize,
    hashes: RefCell<BTreeMap<Path, CachedHash>>,
    watcher: RefCell<Option<watch::Watches>>,
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
            progress: None,
            traversal: Cell::default(),
            root: root(&path)?,
            absolute: path,
            sync_root_id,
            max_items: MAX_ITEMS,
            hashes: RefCell::default(),
            watcher: RefCell::default(),
        })
    }
    pub(super) fn set_progress(&mut self, progress: Option<activity::Reporter>) {
        self.progress = progress;
    }

    fn emit(&self, event: activity::Event<'_>) {
        if let Some(report) = &self.progress {
            report(event);
        }
    }

    fn count(&self, update: impl FnOnce(&mut activity::Traversal)) {
        if self.progress.is_some() {
            let mut counts = self.traversal.get();
            update(&mut counts);
            self.traversal.set(counts);
        }
    }

    fn report(&self, path: &StdPath) {
        self.emit(activity::Event::Position(path, self.traversal.get()));
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
    pub(super) fn event_fd(&self) -> Option<File> {
        self.watcher.borrow().as_ref()?.handle()
    }

    /// Recheck data changes from a covered native watch set, or rebuild on coverage loss.
    pub(crate) fn poll(
        &self,
        previous: Option<&BTreeMap<Path, LocalObservation>>,
        cancel: &CancellationToken,
    ) -> Result<ScanResult, ScanError> {
        if cancel.is_cancelled() {
            self.watcher.take();
            self.hashes.take();
            return Err(ScanError::Cancelled);
        }
        self.traversal.set(activity::Traversal::default());
        let result = (|| {
            if let Some(previous) = previous {
                if let Some(scan) = self.scan_dirty(previous, cancel)? {
                    return Ok(scan);
                }
            }
            self.scan(cancel)
        })();
        if let Ok(scan) = &result {
            self.count(|counts| counts.snapshot = scan.entries.len() as u64);
        }
        self.emit(activity::Event::Final(
            self.traversal.get(),
            result
                .as_ref()
                .map(|scan| scan.coverage)
                .map_err(|error| *error),
        ));
        result
    }

    fn check_root(&self) -> Result<std::fs::Metadata, ScanError> {
        let actual = root(&self.absolute)?.metadata().map_err(io_error)?;
        let expected = self.root.metadata().map_err(io_error)?;
        if (expected.dev(), expected.ino()) != (actual.dev(), actual.ino()) {
            return Err(ScanError::RootChanged);
        }
        Ok(expected)
    }

    fn scan_dirty(
        &self,
        previous: &BTreeMap<Path, LocalObservation>,
        cancel: &CancellationToken,
    ) -> Result<Option<ScanResult>, ScanError> {
        let dirty = self
            .watcher
            .borrow_mut()
            .as_mut()
            .filter(|watcher| watcher.started.elapsed() < HASH_AUDIT_INTERVAL)
            .map(|watcher| watcher.dirty(self.max_items));
        let dirty = match dirty {
            Some(Ok(dirty)) => dirty,
            Some(Err(watch::Rebuild::Coverage)) => {
                self.hashes.take();
                self.emit(activity::Event::Rebuild(activity::Rebuild::Coverage));
                return Ok(None);
            }
            Some(Err(watch::Rebuild::Namespace)) => {
                self.emit(activity::Event::Rebuild(activity::Rebuild::Namespace));
                return Ok(None);
            }
            None => {
                self.emit(activity::Event::Rebuild(activity::Rebuild::NoCurrentWatch));
                return Ok(None);
            }
        };
        let _sample = crate::performance::Sample::begin("local_incremental_scan");
        self.check_root()?;
        let mut entries = previous.clone();
        let mut changed_hashes = BTreeMap::new();
        for (path, namespace_changed) in dirty {
            self.count(|counts| counts.visited += 1);
            if !self.recheck_path(
                previous,
                &mut entries,
                &mut changed_hashes,
                &path,
                namespace_changed,
                cancel,
            )? {
                self.emit(activity::Event::Rebuild(activity::Rebuild::Namespace));
                return Ok(None);
            }
            self.count(|counts| counts.snapshot = entries.len() as u64);
        }
        self.check_root()?;
        if entries.len() > self.max_items {
            return Ok(None);
        }
        if !self
            .watcher
            .borrow()
            .as_ref()
            .is_some_and(watch::Watches::mount_coverage_current)
        {
            self.hashes.take();
            return Ok(None);
        }
        let mut hashes = self.hashes.borrow_mut();
        for (path, hash) in changed_hashes {
            if let Some(hash) = hash {
                hashes.insert(path, hash);
            } else {
                hashes.remove(&path);
            }
        }
        Ok(Some(ScanResult {
            entries,
            coverage: ObservationCoverage::Complete,
            issues: Vec::new(),
        }))
    }

    fn recheck_path(
        &self,
        previous: &BTreeMap<Path, LocalObservation>,
        entries: &mut BTreeMap<Path, LocalObservation>,
        changed_hashes: &mut BTreeMap<Path, Option<CachedHash>>,
        path: &StdPath,
        namespace_changed: bool,
        cancel: &CancellationToken,
    ) -> Result<bool, ScanError> {
        let parent = path.parent().unwrap();
        let Ok(directory) = self.directory(parent) else {
            return Ok(false);
        };
        let before = directory.metadata().map_err(io_error)?;
        if !parent.as_os_str().is_empty()
            && !previous
                .get(parent)
                .is_some_and(|entry| entry.identity == Evidence::Known(local_identity(&before)))
        {
            return Ok(false);
        }
        let old = previous.get(path);
        if old.is_some_and(|entry| entry.kind != Evidence::Known(FileKind::Regular)) {
            return Ok(false);
        }
        let anchored = descriptor(&directory).join(path.file_name().unwrap());
        match std::fs::symlink_metadata(&anchored) {
            Ok(meta) if meta.is_file() && meta.nlink() == 1 && meta.dev() == before.dev() => {
                match self.collect_entry(&directory, parent, path, &meta, None, cancel)? {
                    (entry, Some(hash), None)
                        if namespace_changed
                            || old.is_some_and(|old| old.identity == entry.identity) =>
                    {
                        changed_hashes.insert(path.to_path_buf(), Some(hash));
                        entries.insert(path.to_path_buf(), entry);
                    }
                    _ => return Ok(false),
                }
            }
            Err(error) if namespace_changed && error.kind() == std::io::ErrorKind::NotFound => {
                changed_hashes.insert(path.to_path_buf(), None);
                entries.remove(path);
            }
            _ => return Ok(false),
        }
        if namespace_changed {
            if let Some(grandparent) = parent.parent() {
                let Ok(grandparent_fd) = self.directory(grandparent) else {
                    return Ok(false);
                };
                let (entry, _, _) = self.collect_entry(
                    &grandparent_fd,
                    grandparent,
                    parent,
                    &before,
                    None,
                    cancel,
                )?;
                entries.insert(parent.to_path_buf(), entry);
            }
        }
        let after = directory.metadata().map_err(io_error)?;
        Ok(stamp(&before) == stamp(&after))
    }

    /// Reads native paths and SHA-256 in bounded buffers, checking cancellation and stability.
    pub(crate) fn scan(&self, cancel: &CancellationToken) -> Result<ScanResult, ScanError> {
        let _sample = crate::performance::Sample::begin("local_full_scan");
        let previous = self.hashes.take();
        let mut hashes = BTreeMap::new();
        if cancel.is_cancelled() {
            return Err(ScanError::Cancelled);
        }
        self.watcher.take();
        let expected = self.check_root()?;
        let mut watcher = cacheable(&self.root).then(watch::Watches::open).flatten();
        let mut result = ScanResult {
            entries: BTreeMap::new(),
            coverage: ObservationCoverage::Complete,
            issues: Vec::new(),
        };
        let mut pending = vec![(PathBuf::new(), 0, (expected.dev(), expected.ino()))];
        while let Some((parent, depth, identity)) = pending.pop() {
            self.report(&parent);
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
            if actual.dev() != expected.dev()
                || watcher
                    .as_mut()
                    .is_some_and(|watcher| watcher.add(&directory, &parent).is_none())
            {
                watcher = None;
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
                self.count(|counts| counts.visited += 1);
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
                let (entry, verified, problem) = self.collect_entry(
                    &directory,
                    &parent,
                    &path,
                    &meta,
                    previous.get(&path),
                    cancel,
                )?;
                let is_directory = entry.kind == Evidence::Known(FileKind::Directory);
                if meta.is_file() && (meta.nlink() != 1 || meta.dev() != expected.dev()) {
                    watcher = None;
                }
                if let Some(verified) = verified {
                    hashes.insert(path.clone(), verified);
                }
                if let Some(problem) = problem {
                    issue(&mut result, path.clone(), problem);
                }
                result.entries.insert(path.clone(), entry);
                self.count(|counts| counts.snapshot = result.entries.len() as u64);
                if is_directory {
                    if depth >= MAX_DEPTH {
                        issue(&mut result, path, ScanIssueKind::LimitReached);
                    } else {
                        // Queue names and observed identities, not open directory descriptors.
                        // Wide trees therefore cannot exhaust the process descriptor limit.
                        pending.push((path, depth + 1, (meta.dev(), meta.ino())));
                    }
                }
            }
            self.count(|counts| counts.directories += 1);
            let after = directory.metadata().map_err(io_error)?;
            if stamp(&before) != stamp(&after) {
                issue(&mut result, parent, ScanIssueKind::Unstable);
            }
        }
        self.check_root()?;
        if result.coverage != ObservationCoverage::Complete {
            for entry in result.entries.values_mut() {
                entry.coverage = result.coverage;
            }
        } else {
            self.hashes.replace(hashes);
            self.watcher.replace(watcher);
        }
        Ok(result)
    }

    fn collect_entry(
        &self,
        directory: &File,
        parent: &StdPath,
        path: &StdPath,
        meta: &std::fs::Metadata,
        previous: Option<&CachedHash>,
        cancel: &CancellationToken,
    ) -> Result<(LocalObservation, Option<CachedHash>, Option<ScanIssueKind>), ScanError> {
        self.count(|counts| counts.selected += 1);
        self.report(path);
        let name = path.file_name().ok_or(ScanError::Unavailable)?;
        let anchored = descriptor(directory).join(name);
        let kind = if meta.is_file() {
            FileKind::Regular
        } else if meta.is_dir() {
            FileKind::Directory
        } else if meta.file_type().is_symlink() {
            FileKind::Symlink
        } else {
            FileKind::Unsupported
        };
        let mut verified_hash = None;
        let mut problem = None;
        let mut checksum = Evidence::NotApplicable;
        let mut can_read = Evidence::NotApplicable;
        if kind == FileKind::Regular {
            match hash(&anchored, meta, previous, cancel, |event| match event {
                HashProgress::Reused => self.count(|counts| counts.hashes_reused += 1),
                HashProgress::Start(total) => {
                    self.emit(activity::Event::ChecksumStart { path, total })
                }
                HashProgress::Bytes(bytes) => self.emit(activity::Event::ChecksumBytes(bytes)),
                HashProgress::End(result) => self.emit(activity::Event::ChecksumEnd(result)),
            }) {
                Ok(verified) => {
                    checksum = Evidence::Known(verified.digest.clone());
                    verified_hash = Some(verified);
                    can_read = Evidence::Known(true);
                }
                Err(ScanError::Cancelled) => return Err(ScanError::Cancelled),
                Err(ScanError::PermissionDenied) => {
                    checksum = Evidence::PermissionDenied;
                    can_read = Evidence::Known(false);
                    problem = Some(ScanIssueKind::Unreadable);
                }
                Err(_) => {
                    checksum = Evidence::Unavailable;
                    can_read = Evidence::Unavailable;
                    problem = Some(ScanIssueKind::Unstable);
                }
            }
        }
        let observed_at = now();
        self.count(|counts| counts.completed += 1);
        Ok((
            LocalObservation {
                sync_root_id: self.sync_root_id.clone(),
                state: FileState::Present,
                path: Some(path.to_path_buf()),
                previous_path: None,
                name: name
                    .to_str()
                    .map(|name| Evidence::Known(name.to_owned()))
                    .unwrap_or(Evidence::Unsupported),
                parent: Evidence::Known(parent.to_path_buf()),
                kind: Evidence::Known(kind),
                identity: Evidence::Known(local_identity(meta)),
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
                    can_rename: access(&descriptor(directory).join("."), libc::W_OK | libc::X_OK),
                    can_delete: Evidence::NotCollected,
                }),
                observed_at,
                coverage: ObservationCoverage::Complete,
                event: Some(ObservationEvent {
                    origin: ObservationOrigin::Scanner,
                    kind: ObservationKind::Snapshot,
                    observed_at,
                    old_path: None,
                    new_path: Some(path.to_path_buf()),
                    remote_id: None,
                    move_cookie: None,
                }),
            },
            verified_hash,
            problem,
        ))
    }
}
pub(crate) fn now() -> DateTime {
    crate::orchestrator::timestamp()
}
pub(crate) fn ignored(name: &std::ffi::OsStr) -> bool {
    name.as_bytes().starts_with(b".drivesync-transfer-")
        || crate::projections::private_trash_name(name)
        || crate::projections::libreoffice_lock_name(name)
}
fn issue(result: &mut ScanResult, path: Path, kind: ScanIssueKind) {
    result.coverage = ObservationCoverage::Partial;
    if result.issues.len() < 256 {
        result.issues.push(ScanIssue { path, kind });
    }
}
fn local_identity(meta: &std::fs::Metadata) -> LocalIdentity {
    LocalIdentity {
        device_id: meta.dev(),
        inode: meta.ino(),
        mount_id: None,
        birth_ns: crate::models::birth_ns(meta),
    }
}
fn stamp(meta: &std::fs::Metadata) -> FileStamp {
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
enum HashProgress {
    Reused,
    Start(u64),
    Bytes(u64),
    End(Result<(), ScanError>),
}

fn hash(
    path: &StdPath,
    expected: &std::fs::Metadata,
    previous: Option<&CachedHash>,
    cancel: &CancellationToken,
    progress: impl Fn(HashProgress),
) -> Result<CachedHash, ScanError> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(io_error)?;
    let before = file.metadata().map_err(io_error)?;
    if !before.is_file() || stamp(expected) != stamp(&before) {
        return Err(ScanError::Unavailable);
    }
    let fingerprint = stamp(&before);
    let birth_ns = crate::models::birth_ns(&before);
    let verified = match previous.filter(|cache| {
        cache.stamp == fingerprint
            && cache.birth_ns == birth_ns
            && cache.checked.elapsed() < HASH_AUDIT_INTERVAL
            && cacheable(&file)
    }) {
        Some(cached) => {
            let _sample = crate::performance::Sample::begin("local_sha256_reuse");
            progress(HashProgress::Reused);
            cached.clone()
        }
        None => CachedHash {
            stamp: fingerprint,
            birth_ns,
            digest: if before.len() >= 1024 * 1024 {
                let total = before.len();
                progress(HashProgress::Start(total));
                let result = digest(&mut file, cancel, |bytes| {
                    progress(HashProgress::Bytes(bytes))
                });
                progress(HashProgress::End(
                    result.as_ref().map(|_| ()).map_err(|error| *error),
                ));
                result?
            } else {
                digest(&mut file, cancel, |_| {})?
            },
            checked: Instant::now(),
        },
    };
    if cancel.is_cancelled() {
        return Err(ScanError::Cancelled);
    }
    let after = file.metadata().map_err(io_error)?;
    let named = std::fs::symlink_metadata(path).map_err(io_error)?;
    if fingerprint != stamp(&after)
        || fingerprint != stamp(&named)
        || birth_ns != crate::models::birth_ns(&after)
        || birth_ns != crate::models::birth_ns(&named)
    {
        return Err(ScanError::Unavailable);
    }
    Ok(verified)
}

fn cacheable(file: &File) -> bool {
    let mut stat = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // FUSE and network mounts may cache or synthesize their inode timestamps.
    if unsafe { libc::fstatfs(file.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
        return false;
    }
    matches!(
        unsafe { stat.assume_init() }.f_type,
        libc::EXT4_SUPER_MAGIC
            | libc::BTRFS_SUPER_MAGIC
            | libc::XFS_SUPER_MAGIC
            | libc::TMPFS_MAGIC
    )
}

fn digest(
    file: &mut File,
    cancel: &CancellationToken,
    progress: impl Fn(u64),
) -> Result<Checksum, ScanError> {
    let _sample = crate::performance::Sample::begin("local_sha256");
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    let mut bytes = 0;
    loop {
        if cancel.is_cancelled() {
            return Err(ScanError::Cancelled);
        }
        let count = file.read(&mut buffer).map_err(io_error)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
        bytes += count as u64;
        progress(bytes);
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

#[cfg(test)]
pub(in crate::detectors) mod tests;
