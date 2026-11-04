//! Directory-only inotify adapter with bounded volatile delivery.
//!
//! Watches are installed through pinned directory descriptors. Resolution of
//! descendants rejects links and mount crossings. Path checks detect races but
//! do not freeze directory topology; every notice remains an activity hint.
use super::*;
use rustix::fs::{inotify, openat2, Mode, OFlags, ResolveFlags, CWD};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fs::File;
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::Instant;

type Identity = (u64, u64);
struct Watch {
    path: PathBuf,
    identity: Identity,
}
struct PendingMove {
    path: PathBuf,
    directory: bool,
    since: Instant,
}

/// An independent, nonblocking observer for one account/root scope.
///
/// Call [`Self::poll`] regularly. Dropping this value releases all watches.
/// No worker thread, content read, persistent queue or automatic recovery is
/// hidden inside this API. Consumers own transferred batches and must handle
/// recheck notices before treating their own state as current.
pub struct LocalWatcher {
    scope: LocalScope,
    options: LocalWatchOptions,
    root: File,
    root_identity: Identity,
    source: OwnedFd,
    watches: HashMap<i32, Watch>,
    retired: HashSet<i32>,
    moves: BTreeMap<u32, PendingMove>,
    queue: VecDeque<LocalNotice>,
    queue_lost: bool,
    state: LocalWatchState,
}

impl LocalWatcher {
    /// Opens the root and installs watches without reading file contents.
    ///
    /// Existing entries are not reported as creations. A startup recheck is
    /// always delivered; partial traversal is explicitly incomplete.
    /// # Errors
    /// Returns invalid input or unavailable if root/openat2/inotify is denied
    /// or unsupported. Descendant failures are delivered as coverage gaps.
    pub fn start(scope: LocalScope, options: LocalWatchOptions) -> Result<Self, LocalWatchError> {
        if !options.valid() {
            return Err(LocalWatchError::InvalidInput);
        }
        let root = open_root(scope.root()).map_err(|_| LocalWatchError::Unavailable)?;
        let root_identity = identity(&root).map_err(|_| LocalWatchError::Unavailable)?;
        let source = inotify::init(inotify::CreateFlags::CLOEXEC | inotify::CreateFlags::NONBLOCK)
            .map_err(|_| LocalWatchError::Unavailable)?;
        let mut watcher = Self {
            scope,
            options,
            root,
            root_identity,
            source,
            watches: HashMap::new(),
            retired: HashSet::new(),
            moves: BTreeMap::new(),
            queue: VecDeque::new(),
            queue_lost: false,
            state: LocalWatchState::Initializing,
        };
        watcher.gap(PathBuf::new(), LocalCoverageGap::Startup);
        // Linux filesystem magic values from include/uapi/linux/magic.h.
        // Known local backends still have inotify blind spots; all other backends
        // require explicit investigation rather than implied notification coverage.
        let known_local = rustix::fs::fstatfs(&watcher.root).is_ok_and(|fs| {
            matches!(
                fs.f_type as u64,
                0xef53 | 0x9123683e | 0x58465342 | 0x01021994 | 0x794c7630
            )
        });
        if !known_local {
            watcher.gap(PathBuf::new(), LocalCoverageGap::FilesystemUnverified);
        }
        watcher.install_tree(Path::new(""));
        if watcher.state == LocalWatchState::Initializing {
            watcher.state = LocalWatchState::Observing;
        }
        watcher.check_root();
        Ok(watcher)
    }

    /// Current state; incomplete is sticky and stopped requires a new session.
    pub fn state(&self) -> LocalWatchState {
        self.state
    }

    /// Collects a bounded number of kernel buffers and transfers queued notices.
    ///
    /// The final reserved application-overflow notice follows retained notices.
    /// Cookie matches may span polls; unmatched sources expire on later polls.
    /// This is volatile delivery, not a durable acknowledgement protocol.
    pub fn poll(&mut self) -> LocalBatch {
        if self.state != LocalWatchState::Stopped {
            self.check_root();
        }
        if self.state != LocalWatchState::Stopped {
            // Duplicating the descriptor allows processing without borrowing self.
            match rustix::io::dup(&self.source) {
                Ok(fd) => {
                    let mut buffer = [MaybeUninit::uninit(); 4096];
                    let mut reader = inotify::Reader::new(fd, &mut buffer);
                    let mut reads = 0;
                    loop {
                        if reader.is_buffer_empty() {
                            if reads == self.options.max_reads_per_poll {
                                break;
                            }
                            reads += 1;
                        }
                        match reader.next() {
                            Ok(event) => {
                                let wd = event.wd();
                                let flags = event.events();
                                let cookie = event.cookie();
                                let name = event.file_name().map(|n| n.to_bytes().to_vec());
                                self.process(wd, flags, cookie, name.as_deref());
                            }
                            Err(rustix::io::Errno::AGAIN) => break,
                            Err(rustix::io::Errno::INTR) => continue,
                            Err(_) => {
                                self.stop(LocalCoverageGap::SourceFailed);
                                break;
                            }
                        }
                        if self.state == LocalWatchState::Stopped {
                            break;
                        }
                    }
                }
                Err(_) => self.stop(LocalCoverageGap::SourceFailed),
            }
            self.expire_moves();
            self.check_root();
        }
        let mut notices: Vec<_> = self.queue.drain(..).collect();
        if std::mem::take(&mut self.queue_lost) {
            notices.push(LocalNotice {
                path: PathBuf::new(),
                is_directory: Some(true),
                activity: LocalActivity::Recheck(LocalCoverageGap::ApplicationOverflow),
            });
        }
        LocalBatch {
            scope: self.scope.clone(),
            state: self.state,
            notices,
        }
    }

    fn emit(&mut self, notice: LocalNotice) {
        if self.queue.len() < self.options.max_queued {
            self.queue.push_back(notice);
        } else {
            self.queue_lost = true;
            if self.state != LocalWatchState::Stopped {
                self.state = LocalWatchState::Incomplete;
            }
        }
    }
    fn gap(&mut self, path: PathBuf, reason: LocalCoverageGap) {
        if !matches!(reason, LocalCoverageGap::Startup) && self.state != LocalWatchState::Stopped {
            self.state = LocalWatchState::Incomplete;
        }
        let is_directory = path.as_os_str().is_empty().then_some(true);
        self.emit(LocalNotice {
            path,
            is_directory,
            activity: LocalActivity::Recheck(reason),
        });
    }
    fn stop(&mut self, reason: LocalCoverageGap) {
        self.state = LocalWatchState::Stopped;
        self.gap(PathBuf::new(), reason);
        self.moves.clear();
        // Closing watches also bounds the queued IGNORED events; stopped never reads again.
        for wd in self.watches.keys() {
            let _ = inotify::remove_watch(&self.source, *wd);
        }
        self.watches.clear();
        self.retired.clear();
    }
    /// Removed IDs are retained only until IGNORED arrives. If it was lost,
    /// stop at the same finite limit as active watches rather than growing forever.
    fn retire(&mut self, wd: i32) -> bool {
        let _ = inotify::remove_watch(&self.source, wd);
        if self.retired.len() >= self.options.max_watches {
            self.stop(LocalCoverageGap::ResourceLimit);
            return false;
        }
        self.retired.insert(wd);
        true
    }
    fn check_root(&mut self) {
        if self.state == LocalWatchState::Stopped {
            return;
        }
        let current = open_root(self.scope.root()).and_then(|f| identity(&f));
        if current.ok() != Some(self.root_identity)
            || identity(&self.root).ok() != Some(self.root_identity)
        {
            self.stop(LocalCoverageGap::RootChanged);
        }
    }
    fn open_directory(&self, path: &Path) -> std::io::Result<File> {
        let path = if path.as_os_str().is_empty() {
            Path::new(".")
        } else {
            path
        };
        openat2(
            &self.root,
            path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
        )
        .map(File::from)
        .map_err(Into::into)
    }
    fn watch_matches(&self, watch: &Watch) -> bool {
        self.open_directory(&watch.path)
            .and_then(|f| identity(&f))
            .ok()
            == Some(watch.identity)
    }

    /// Each traversal is bounded by directory/watch and examined-entry limits.
    /// Watch first, enumerate second; attachment races still require a recheck.
    fn install_tree(&mut self, start: &Path) {
        let mut pending = VecDeque::from([start.to_path_buf()]);
        let mut examined = 0;
        while let Some(path) = pending.pop_front() {
            if self.state == LocalWatchState::Stopped {
                return;
            }
            if self.watches.values().any(|w| w.path == path) {
                continue;
            }
            if self.watches.len() >= self.options.max_watches {
                self.gap(path, LocalCoverageGap::ResourceLimit);
                return;
            }
            let directory = match self.open_directory(&path) {
                Ok(f) => f,
                Err(_) => {
                    self.gap(path, LocalCoverageGap::Inaccessible);
                    continue;
                }
            };
            let Ok(id) = identity(&directory) else {
                self.gap(path, LocalCoverageGap::Inaccessible);
                continue;
            };
            let flags = inotify::WatchFlags::CREATE
                | inotify::WatchFlags::DELETE
                | inotify::WatchFlags::MODIFY
                | inotify::WatchFlags::CLOSE_WRITE
                | inotify::WatchFlags::ATTRIB
                | inotify::WatchFlags::MOVE
                | inotify::WatchFlags::MOVE_SELF
                | inotify::WatchFlags::DELETE_SELF
                | inotify::WatchFlags::ONLYDIR
                | inotify::WatchFlags::MASK_CREATE;
            // Follow only our own pinned /proc descriptor, never a user symlink.
            let pinned = PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()));
            let wd = match inotify::add_watch(&self.source, &pinned, flags) {
                Ok(wd) => wd,
                Err(_) => {
                    self.gap(path, LocalCoverageGap::Inaccessible);
                    continue;
                }
            };
            if self.retired.contains(&wd) || self.watches.contains_key(&wd) {
                self.stop(LocalCoverageGap::WatchLost);
                return;
            }
            let watch = Watch {
                path: path.clone(),
                identity: id,
            };
            if !self.watch_matches(&watch) {
                self.retire(wd);
                self.gap(path, LocalCoverageGap::PathChanged);
                continue;
            }
            self.watches.insert(wd, watch);
            let entries = match std::fs::read_dir(&pinned) {
                Ok(entries) => entries,
                Err(_) => {
                    self.gap(path, LocalCoverageGap::Inaccessible);
                    continue;
                }
            };
            for entry in entries {
                examined += 1;
                if examined > self.options.max_scan_entries {
                    self.gap(path.clone(), LocalCoverageGap::ResourceLimit);
                    return;
                }
                match entry.and_then(|e| Ok((e.file_name(), e.file_type()?))) {
                    Ok((name, kind)) if kind.is_dir() => {
                        if pending.len() + self.watches.len() >= self.options.max_watches {
                            self.gap(path.clone(), LocalCoverageGap::ResourceLimit);
                        } else {
                            pending.push_back(path.join(name));
                        }
                    }
                    Ok(_) => {}
                    Err(_) => self.gap(path.clone(), LocalCoverageGap::Inaccessible),
                }
            }
            if !self.watches.get(&wd).is_some_and(|w| self.watch_matches(w)) {
                self.gap(path, LocalCoverageGap::PathChanged);
            }
        }
    }

    fn remove_tree(&mut self, path: &Path) {
        let ids: Vec<_> = self
            .watches
            .iter()
            .filter(|(_, w)| w.path.starts_with(path))
            .map(|(wd, _)| *wd)
            .collect();
        for wd in ids {
            self.watches.remove(&wd);
            if !self.retire(wd) {
                break;
            }
        }
    }
    fn rebase(&mut self, from: &Path, to: &Path) {
        // Replacement may remove an already-watched destination subtree.
        self.remove_tree(to);
        for watch in self.watches.values_mut() {
            if let Ok(suffix) = watch.path.strip_prefix(from) {
                watch.path = to.join(suffix);
            }
        }
    }
    fn expire_moves(&mut self) {
        let now = Instant::now();
        let expired: Vec<_> = self
            .moves
            .iter()
            .filter(|(_, m)| now.duration_since(m.since) >= self.options.move_timeout)
            .map(|(cookie, _)| *cookie)
            .collect();
        for cookie in expired {
            let movement = self.moves.remove(&cookie).expect("existing move");
            if movement.directory {
                self.remove_tree(&movement.path);
            }
            self.emit(LocalNotice {
                path: movement.path,
                is_directory: movement.directory.then_some(true),
                activity: LocalActivity::MovedOut,
            });
        }
    }

    fn process(&mut self, wd: i32, flags: inotify::ReadFlags, cookie: u32, name: Option<&[u8]>) {
        use inotify::ReadFlags as F;
        if flags.contains(F::QUEUE_OVERFLOW) {
            self.moves.clear();
            self.gap(PathBuf::new(), LocalCoverageGap::KernelOverflow);
            return;
        }
        if self.retired.contains(&wd) {
            if flags.contains(F::IGNORED) {
                self.retired.remove(&wd);
            }
            return;
        }
        let Some(watch) = self.watches.get(&wd) else {
            self.gap(PathBuf::new(), LocalCoverageGap::WatchLost);
            return;
        };
        let base = watch.path.clone();
        if flags.intersects(F::IGNORED | F::UNMOUNT | F::DELETE_SELF) {
            if base.as_os_str().is_empty() {
                self.stop(LocalCoverageGap::RootChanged);
            } else {
                self.remove_tree(&base);
                // IGNORED itself has already acknowledged this removed ID.
                if flags.contains(F::IGNORED) {
                    self.retired.remove(&wd);
                }
                self.gap(base, LocalCoverageGap::WatchLost);
            }
            return;
        }
        if base.as_os_str().is_empty() && flags.contains(F::MOVE_SELF) {
            self.stop(LocalCoverageGap::RootChanged);
            return;
        }
        if self
            .moves
            .values()
            .any(|m| m.directory && base.starts_with(&m.path))
        {
            self.gap(base, LocalCoverageGap::MoveUncertain);
            return;
        }
        if !self.watch_matches(watch) {
            self.remove_tree(&base);
            self.gap(base, LocalCoverageGap::PathChanged);
            return;
        }
        if flags.contains(F::MOVE_SELF) {
            return;
        }
        let path = match name {
            Some(bytes)
                if !bytes.is_empty()
                    && !bytes.contains(&b'/')
                    && bytes != b"."
                    && bytes != b".." =>
            {
                base.join(std::ffi::OsStr::from_bytes(bytes))
            }
            None => base,
            _ => {
                self.gap(base, LocalCoverageGap::PathChanged);
                return;
            }
        };
        let directory = flags.contains(F::ISDIR) || name.is_none();
        let kind = directory.then_some(true);
        if flags.contains(F::MOVED_FROM) {
            if cookie == 0
                || self.moves.len() >= self.options.max_pending_moves
                || self.moves.contains_key(&cookie)
            {
                if directory {
                    self.remove_tree(&path);
                }
                self.gap(path, LocalCoverageGap::ResourceLimit);
                return;
            }
            self.moves.insert(
                cookie,
                PendingMove {
                    path,
                    directory,
                    since: Instant::now(),
                },
            );
            return;
        }
        if flags.contains(F::MOVED_TO) {
            let movement = self.moves.remove(&cookie);
            let activity = match movement {
                Some(m) if m.directory == directory => {
                    if directory {
                        self.rebase(&m.path, &path);
                    }
                    LocalActivity::Moved { from: m.path }
                }
                Some(m) => {
                    if m.directory {
                        self.remove_tree(&m.path);
                    }
                    self.gap(m.path, LocalCoverageGap::MoveUncertain);
                    LocalActivity::MovedIn
                }
                None => LocalActivity::MovedIn,
            };
            self.emit(LocalNotice {
                path: path.clone(),
                is_directory: kind,
                activity,
            });
            if directory {
                self.gap(path.clone(), LocalCoverageGap::SubtreeAttached);
                self.install_tree(&path);
            }
            return;
        }
        for (flag, activity) in [
            (F::CREATE, LocalActivity::Created),
            (F::DELETE, LocalActivity::Removed),
            (F::MODIFY, LocalActivity::Written),
            (F::CLOSE_WRITE, LocalActivity::ClosedAfterWrite),
            (F::ATTRIB, LocalActivity::MetadataChanged),
        ] {
            if flags.contains(flag) {
                self.emit(LocalNotice {
                    path: path.clone(),
                    is_directory: kind,
                    activity,
                });
            }
        }
        if directory && flags.contains(F::DELETE) {
            self.remove_tree(&path);
        }
        if directory && flags.contains(F::CREATE) {
            self.gap(path.clone(), LocalCoverageGap::SubtreeAttached);
            self.install_tree(&path);
        }
    }
}

fn identity(file: &File) -> std::io::Result<Identity> {
    let metadata = file.metadata()?;
    Ok((metadata.dev(), metadata.ino()))
}
fn open_root(path: &Path) -> std::io::Result<File> {
    openat2(
        CWD,
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS,
    )
    .map(File::from)
    .map_err(Into::into)
}

#[cfg(test)]
mod tests;
