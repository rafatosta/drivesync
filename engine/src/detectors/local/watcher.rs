//! Private Linux notifications. Raw hints never escape to the orchestrator.
use super::{collector::FilesystemMetadataCollector, filesystem, models::*};
use crate::models::LocalRelativePath;
use std::{
    collections::{BTreeSet, HashMap, VecDeque},
    ffi::CString,
    fs::File,
    io,
    mem::size_of,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::ffi::OsStrExt,
    },
    sync::Arc,
};
const MASK: u32 = libc::IN_CREATE
    | libc::IN_DELETE
    | libc::IN_MODIFY
    | libc::IN_CLOSE_WRITE
    | libc::IN_ATTRIB
    | libc::IN_MOVED_FROM
    | libc::IN_MOVED_TO
    | libc::IN_DELETE_SELF
    | libc::IN_MOVE_SELF
    | libc::IN_UNMOUNT
    | libc::IN_ONLYDIR;
/// Finite native coverage budgets; exceeding one is never complete coverage.
#[derive(Clone, Copy)]
pub(super) struct Limits {
    pub directories: usize,
    pub entries: usize,
    pub path_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            directories: 4096,
            entries: 100_000,
            path_bytes: 8 * 1024 * 1024,
        }
    }
}
/// Independent stop wakeup; no incoming filesystem event is needed to terminate.
pub(super) struct Wake(File);
impl Wake {
    pub(super) fn new() -> LocalDetectorResult<Self> {
        // SAFETY: eventfd has no pointers and returns a new owned fd.
        let fd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
        if fd < 0 {
            return Err(watch_error(io::Error::last_os_error()));
        }
        // SAFETY: a successful eventfd result is transferred exactly once.
        Ok(Self(unsafe { File::from_raw_fd(fd) }))
    }
    pub(super) fn signal(&self) {
        let value = 1u64;
        // SAFETY: the descriptor is live and exactly eight initialized bytes are readable.
        unsafe {
            libc::write(
                self.0.as_raw_fd(),
                (&value as *const u64).cast(),
                size_of::<u64>(),
            );
        }
    }
}
struct Node {
    file: File,
    path: Option<LocalRelativePath>,
    observes: bool,
    guards: BTreeSet<Vec<u8>>,
}
/// Kernel hints, bounded by a native read and later coalesced before constructing File.
#[derive(Default)]
pub(super) struct Hints {
    pub paths: BTreeSet<LocalRelativePath>,
    pub structural: bool,
    pub recheck: bool,
    pub overflow: bool,
}
impl Hints {
    pub(super) fn any(&self) -> bool {
        !self.paths.is_empty() || self.structural || self.recheck
    }
}
/// Owned kernel watches, directory anchors and original root identity.
pub(super) struct Watches {
    fd: File,
    nodes: HashMap<i32, Node>,
    root: Arc<FilesystemMetadataCollector>,
    directories: usize,
    path_bytes: usize,
}
impl Watches {
    /// Captures root and ancestry before asynchronous recursive coverage verification.
    pub(super) fn root(
        root: Arc<FilesystemMetadataCollector>,
        cancel: &LocalCancellation,
    ) -> LocalDetectorResult<Self> {
        cancel.check()?;
        // SAFETY: initialization has no pointer arguments and returns one owned fd.
        let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        if fd < 0 {
            return Err(watch_error(io::Error::last_os_error()));
        }
        // SAFETY: the successful new descriptor is wrapped once.
        let mut this = Self {
            fd: unsafe { File::from_raw_fd(fd) },
            nodes: HashMap::new(),
            root,
            directories: 0,
            path_bytes: 0,
        };
        let chain = this.root.root_chain(cancel)?;
        for (parent, name) in chain.ancestor_guards() {
            this.add(parent, None, false, Some(name.to_bytes().to_vec()), cancel)?;
        }
        this.add(chain.last(), None, true, None, cancel)?;
        chain.validate(cancel)?;
        Ok(this)
    }
    fn add(
        &mut self,
        file: &File,
        path: Option<LocalRelativePath>,
        observes: bool,
        guard: Option<Vec<u8>>,
        cancel: &LocalCancellation,
    ) -> LocalDetectorResult<()> {
        cancel.check()?;
        let proc_path = CString::new(format!("/proc/self/fd/{}", file.as_raw_fd())).unwrap();
        // SAFETY: the controlled proc path resolves our live fd, not an untrusted
        // symlink. inotify attaches to the pinned inode before this borrow ends.
        let wd = unsafe { libc::inotify_add_watch(self.fd.as_raw_fd(), proc_path.as_ptr(), MASK) };
        if wd < 0 {
            return Err(watch_error(io::Error::last_os_error()));
        }
        if let Some(node) = self.nodes.get_mut(&wd) {
            if observes && node.observes && node.path != path {
                return Err(LocalDetectorFailure::new(
                    LocalDetectorError::PartialObservation,
                ));
            }
            if observes && !node.observes {
                node.path = path;
                node.observes = true;
                self.directories += 1;
            }
            if let Some(guard) = guard {
                node.guards.insert(guard);
            }
            return Ok(());
        }
        let mut guards = BTreeSet::new();
        if let Some(guard) = guard {
            guards.insert(guard);
        }
        self.path_bytes += path.as_ref().map_or(0, |p| p.as_bytes().len());
        self.directories += usize::from(observes);
        self.nodes.insert(
            wd,
            Node {
                file: file
                    .try_clone()
                    .map_err(|e| filesystem::failure(e, false))?,
                path,
                observes,
                guards,
            },
        );
        Ok(())
    }
    fn validate_directory(
        &self,
        file: &File,
        path: Option<&LocalRelativePath>,
        cancel: &LocalCancellation,
    ) -> LocalDetectorResult<()> {
        let mut chain = self.root.root_chain(cancel)?;
        let current = match path {
            None => chain
                .last()
                .try_clone()
                .map_err(|e| filesystem::failure(e, true))?,
            Some(path) => {
                chain
                    .open_entry(path, cancel)?
                    .ok_or_else(|| {
                        LocalDetectorFailure::new(LocalDetectorError::PartialObservation)
                    })?
                    .0
            }
        };
        cancel.check()?;
        let expected = file.metadata().map_err(|e| filesystem::failure(e, false))?;
        cancel.check()?;
        if !filesystem::same_entry(
            &expected,
            &current
                .metadata()
                .map_err(|e| filesystem::failure(e, false))?,
        ) {
            return Err(LocalDetectorFailure::new(
                LocalDetectorError::PartialObservation,
            ));
        }
        chain.validate(cancel)
    }
    /// Installs child directory watches; no Inventory or independent Scanner is called.
    pub(super) fn discover(
        &mut self,
        cancel: &LocalCancellation,
        limits: Limits,
    ) -> LocalDetectorResult<()> {
        let chain = self.root.root_chain(cancel)?;
        let mut pending = VecDeque::new();
        pending.push_back((
            chain
                .last()
                .try_clone()
                .map_err(|e| filesystem::failure(e, true))?,
            None,
        ));
        let mut entries = 0;
        while let Some((directory, path)) = pending.pop_front() {
            cancel.check()?;
            self.validate_directory(&directory, path.as_ref(), cancel)?;
            let listing = std::fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))
                .map_err(|e| filesystem::failure(e, false))?;
            for entry in listing {
                cancel.check()?;
                let name = entry
                    .map_err(|e| filesystem::failure(e, false))?
                    .file_name();
                if name == ".drivesync" {
                    continue;
                }
                entries += 1;
                if entries > limits.entries {
                    return Err(LocalDetectorFailure::new(LocalDetectorError::ResourceLimit));
                }
                let name_c = CString::new(name.as_bytes())
                    .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::InvalidScope))?;
                let child = filesystem::open_at(&directory, &name_c, false)
                    .map_err(|e| filesystem::failure(e, false))?;
                cancel.check()?;
                if !child
                    .metadata()
                    .map_err(|e| filesystem::failure(e, false))?
                    .is_dir()
                {
                    continue;
                }
                let mut bytes = path
                    .as_ref()
                    .map_or_else(Vec::new, |p: &LocalRelativePath| p.as_bytes().to_vec());
                if !bytes.is_empty() {
                    bytes.push(b'/');
                }
                bytes.extend_from_slice(name.as_bytes());
                if self.directories >= limits.directories
                    || self.path_bytes.saturating_add(bytes.len()) > limits.path_bytes
                {
                    return Err(LocalDetectorFailure::new(LocalDetectorError::ResourceLimit));
                }
                let relative = LocalRelativePath::new(bytes)
                    .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::InvalidScope))?;
                self.validate_directory(&child, Some(&relative), cancel)?;
                self.add(&child, Some(relative.clone()), true, None, cancel)?;
                pending.push_back((child, Some(relative)));
            }
        }
        for node in self.nodes.values().filter(|n| n.observes) {
            self.validate_directory(&node.file, node.path.as_ref(), cancel)?;
        }
        Ok(())
    }
    /// Blocks for kernel traffic or stop; poll timeouts never scan or emit events.
    pub(super) fn wait(
        &self,
        wake: &Wake,
        cancel: &LocalCancellation,
        timeout_ms: i32,
    ) -> LocalDetectorResult<Hints> {
        cancel.check()?;
        let mut polls = [
            libc::pollfd {
                fd: self.fd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: wake.0.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // SAFETY: both initialized pollfd entries and owned descriptors stay live.
        let ready =
            unsafe { libc::poll(polls.as_mut_ptr(), polls.len() as libc::nfds_t, timeout_ms) };
        cancel.check()?;
        if ready < 0 {
            let e = io::Error::last_os_error();
            if e.kind() == io::ErrorKind::Interrupted {
                return Ok(Hints::default());
            }
            return Err(watch_error(e));
        }
        if ready == 0 {
            return Ok(Hints::default());
        }
        if polls[0].revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
            return Err(LocalDetectorFailure::new(
                LocalDetectorError::WatcherUnavailable,
            ));
        }
        if polls[0].revents & libc::POLLIN == 0 {
            return Ok(Hints::default());
        }
        let mut bytes = [0u8; 65536];
        // SAFETY: the initialized buffer is writable for exactly its given length.
        let count =
            unsafe { libc::read(self.fd.as_raw_fd(), bytes.as_mut_ptr().cast(), bytes.len()) };
        if count < 0 {
            let e = io::Error::last_os_error();
            if matches!(
                e.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) {
                return Ok(Hints::default());
            }
            return Err(watch_error(e));
        }
        let mut hints = Hints::default();
        let mut offset = 0;
        let count = count as usize;
        while offset + size_of::<libc::inotify_event>() <= count {
            // SAFETY: the header fits and read_unaligned accepts byte-buffer alignment.
            let event = unsafe {
                bytes
                    .as_ptr()
                    .add(offset)
                    .cast::<libc::inotify_event>()
                    .read_unaligned()
            };
            let start = offset + size_of::<libc::inotify_event>();
            let end = start
                .checked_add(event.len as usize)
                .filter(|end| *end <= count)
                .ok_or_else(|| LocalDetectorFailure::new(LocalDetectorError::Internal))?;
            offset = end;
            if event.mask & libc::IN_Q_OVERFLOW != 0 {
                hints.overflow = true;
                hints.recheck = true;
                continue;
            }
            let Some(node) = self.nodes.get(&event.wd) else {
                hints.recheck = true;
                continue;
            };
            let name = &bytes[start..end];
            let name = &name[..name.iter().position(|b| *b == 0).unwrap_or(name.len())];
            if name == b".drivesync" {
                continue;
            }
            if node.guards.contains(name) || (!node.guards.is_empty() && name.is_empty()) {
                hints.structural = true;
                hints.recheck = true;
            }
            if !node.observes {
                continue;
            }
            if self
                .validate_directory(&node.file, node.path.as_ref(), cancel)
                .is_err()
            {
                hints.structural = true;
                hints.recheck = true;
                continue;
            }
            let mut path = node
                .path
                .as_ref()
                .map_or_else(Vec::new, |p| p.as_bytes().to_vec());
            if !name.is_empty() {
                if !path.is_empty() {
                    path.push(b'/');
                }
                path.extend_from_slice(name);
            }
            if path.is_empty() {
                hints.recheck = true;
            } else {
                hints
                    .paths
                    .insert(LocalRelativePath::new(path).map_err(|_| {
                        LocalDetectorFailure::new(LocalDetectorError::InvalidScope)
                    })?);
            }
            if event.mask
                & (libc::IN_CREATE
                    | libc::IN_DELETE
                    | libc::IN_MOVED_FROM
                    | libc::IN_MOVED_TO
                    | libc::IN_DELETE_SELF
                    | libc::IN_MOVE_SELF
                    | libc::IN_UNMOUNT
                    | libc::IN_IGNORED)
                != 0
                || (event.mask & libc::IN_ISDIR != 0 && event.mask & libc::IN_ATTRIB != 0)
            {
                hints.structural = true;
                hints.recheck = true;
            }
        }
        if hints.any() {
            self.root.root_chain(cancel)?;
        }
        Ok(hints)
    }
}
fn watch_error(error: io::Error) -> LocalDetectorFailure {
    let kind = match error.raw_os_error() {
        Some(libc::EACCES | libc::EPERM) => LocalDetectorError::PermissionDenied,
        Some(libc::ENOSPC | libc::EMFILE | libc::ENFILE | libc::ENOMEM) => {
            LocalDetectorError::ResourceLimit
        }
        _ => LocalDetectorError::WatcherUnavailable,
    };
    LocalDetectorFailure::new(kind)
}
