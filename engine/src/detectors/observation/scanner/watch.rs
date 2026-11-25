//! Inotify batches are invalidation hints. Only freshly read filesystem facts become observations.
use super::{descriptor, ignored, File, Path, StdPath};
use std::{
    collections::BTreeMap,
    ffi::{CString, OsStr},
    io::{ErrorKind, Read},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::ffi::OsStrExt,
    },
    time::Instant,
};

const DATA_EVENTS: u32 = libc::IN_ATTRIB | libc::IN_MODIFY | libc::IN_CLOSE_WRITE;
const ENTRY_EVENTS: u32 =
    libc::IN_CREATE | libc::IN_DELETE | libc::IN_MOVED_FROM | libc::IN_MOVED_TO;
const TREE_EVENTS: u32 = libc::IN_DELETE_SELF | libc::IN_MOVE_SELF;
const ENTRY_FLAGS: u32 = DATA_EVENTS | ENTRY_EVENTS;

pub(super) enum Rebuild {
    Namespace,
    Coverage,
}

pub(super) struct Watches {
    fd: File,
    directories: BTreeMap<i32, Path>,
    pub(super) started: Instant,
    mounts: Vec<u8>,
}

impl Watches {
    pub(super) fn open() -> Option<Self> {
        let mounts = std::fs::read("/proc/self/mountinfo").ok()?;
        let fd = unsafe { libc::inotify_init1(libc::IN_CLOEXEC | libc::IN_NONBLOCK) };
        if fd < 0 {
            return None;
        }
        Some(Self {
            fd: unsafe { File::from_raw_fd(fd) },
            directories: BTreeMap::new(),
            started: Instant::now(),
            mounts,
        })
    }

    pub(super) fn add(&mut self, directory: &File, relative: &StdPath) -> Option<()> {
        let path = CString::new(descriptor(directory).as_os_str().as_bytes()).ok()?;
        let watch = unsafe {
            libc::inotify_add_watch(
                self.fd.as_raw_fd(),
                path.as_ptr(),
                ENTRY_FLAGS | TREE_EVENTS | libc::IN_ONLYDIR,
            )
        };
        if watch < 0
            || self
                .directories
                .insert(watch, relative.to_path_buf())
                .is_some()
        {
            return None;
        }
        Some(())
    }

    pub(super) fn handle(&self) -> Option<File> {
        self.fd.try_clone().ok()
    }

    pub(super) fn mount_coverage_current(&self) -> bool {
        std::fs::read("/proc/self/mountinfo").is_ok_and(|mounts| mounts == self.mounts)
    }

    /// A live stream can never infer absence.
    pub(super) fn dirty(&mut self, limit: usize) -> Result<BTreeMap<Path, bool>, Rebuild> {
        // A mount can hide a watched path without producing a directory event.
        if !self.mount_coverage_current() {
            return Err(Rebuild::Coverage);
        }
        let mut dirty = BTreeMap::new();
        let mut buffer = [0u8; 16 * 1024];
        for _ in 0..128 {
            match self.fd.read(&mut buffer) {
                Ok(0) => return Err(Rebuild::Coverage),
                Ok(n) => self.collect(&buffer[..n], &mut dirty, limit)?,
                Err(error) if error.kind() == ErrorKind::WouldBlock => return Ok(dirty),
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(_) => return Err(Rebuild::Coverage),
            }
        }
        Err(Rebuild::Coverage)
    }

    fn collect(
        &self,
        data: &[u8],
        dirty: &mut BTreeMap<Path, bool>,
        limit: usize,
    ) -> Result<(), Rebuild> {
        let header = std::mem::size_of::<libc::inotify_event>();
        let mut rest = data;
        while !rest.is_empty() {
            let bytes = rest.get(..header).ok_or(Rebuild::Coverage)?;
            // inotify writes native headers with variable-length names.
            let event = unsafe {
                bytes
                    .as_ptr()
                    .cast::<libc::inotify_event>()
                    .read_unaligned()
            };
            if event.mask & (libc::IN_Q_OVERFLOW | libc::IN_UNMOUNT | libc::IN_IGNORED) != 0 {
                return Err(Rebuild::Coverage);
            }
            let directory = self.directories.get(&event.wd).ok_or(Rebuild::Coverage)?;
            let end = header
                .checked_add(event.len as usize)
                .ok_or(Rebuild::Coverage)?;
            let field = rest.get(header..end).ok_or(Rebuild::Coverage)?;
            rest = rest.get(end..).ok_or(Rebuild::Coverage)?;
            let name = OsStr::from_bytes(field.split(|b| *b == 0).next().unwrap_or_default());
            if name.is_empty() {
                return Err(Rebuild::Namespace);
            }
            if ignored(name) || (directory.as_os_str().is_empty() && name == ".drivesync") {
                continue;
            }
            if event.mask & !ENTRY_FLAGS != 0 {
                return Err(Rebuild::Namespace);
            }
            let path = directory.join(name);
            if dirty.len() >= limit && !dirty.contains_key(&path) {
                return Err(Rebuild::Coverage);
            }
            *dirty.entry(path).or_default() |= event.mask & ENTRY_EVENTS != 0;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_overflow_invalidates_all_pending_event_evidence() {
        let watches = Watches::open().unwrap();
        let header: Vec<_> = [
            (-1_i32).to_ne_bytes(),
            libc::IN_Q_OVERFLOW.to_ne_bytes(),
            0_u32.to_ne_bytes(),
            0_u32.to_ne_bytes(),
        ]
        .into_iter()
        .flatten()
        .collect();
        assert!(matches!(
            watches.collect(&header, &mut BTreeMap::new(), 100),
            Err(Rebuild::Coverage)
        ));
    }
}
