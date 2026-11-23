//! Linux writer exclusion and durable scope/root identity; no synchronization effects occur here.
use super::*;
use crate::models::{LocalIdentity, SyncRootId};
use crate::SynchronizationErrorKind;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        linux::net::SocketAddrExt,
        unix::{
            fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
            net::{SocketAddr, UnixListener},
        },
    },
    path::{Component, Path},
};
/// Held for the entire source/worker lifetime, including uncertain effect recovery.
pub(super) struct WriterLease {
    _directories: Vec<File>,
    _provider: UnixListener,
    pub(super) identity: LocalIdentity,
}
fn unavailable() -> EngineError {
    synchronization::error(SynchronizationErrorKind::LocalRootUnavailable)
}
fn in_use() -> EngineError {
    synchronization::error(SynchronizationErrorKind::RootInUse)
}
fn directory(path: &Path) -> EngineResult<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| unavailable())
}
fn lock(file: &File, mode: i32) -> EngineResult<()> {
    if unsafe { libc::flock(file.as_raw_fd(), mode | libc::LOCK_NB) } == 0 {
        return Ok(());
    }
    if std::io::Error::last_os_error().kind() == std::io::ErrorKind::WouldBlock {
        Err(in_use())
    } else {
        Err(unavailable())
    }
}
impl WriterLease {
    pub(super) fn acquire(root: &Path, provider: &str, subject: &str) -> EngineResult<Self> {
        let mut directories = vec![];
        let mut parent = directory(Path::new("/"))?;
        let parts: Vec<_> = root.components().skip(1).collect();
        if parts.is_empty() {
            return Err(unavailable());
        }
        for part in parts {
            lock(&parent, libc::LOCK_SH)?;
            let Component::Normal(name) = part else {
                return Err(unavailable());
            };
            let child = directory(
                &std::path::PathBuf::from(format!("/proc/self/fd/{}", parent.as_raw_fd()))
                    .join(name),
            )?;
            directories.push(parent);
            parent = child;
        }
        lock(&parent, libc::LOCK_EX)?;
        let meta = parent.metadata().map_err(|_| unavailable())?;
        let identity = LocalIdentity {
            device_id: meta.dev(),
            inode: meta.ino(),
            mount_id: None,
            birth_ns: crate::models::birth_ns(&meta),
        };
        if identity.birth_ns.is_none() {
            return Err(unavailable());
        }
        directories.push(parent);
        // Abstract sockets are shared by the manifest's shared network namespace, including Flatpak.
        // Framing prevents ambiguous provider/subject concatenation; no account metadata is exposed.
        let mut hash = Sha256::new();
        for bytes in [provider.as_bytes(), subject.as_bytes()] {
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
        let name = format!(
            "drivesync-writer-{}-{:x}",
            unsafe { libc::geteuid() },
            hash.finalize()
        );
        let address = SocketAddr::from_abstract_name(name.as_bytes()).map_err(|_| unavailable())?;
        let provider = UnixListener::bind_addr(&address).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                in_use()
            } else {
                unavailable()
            }
        })?;
        Ok(Self {
            _directories: directories,
            _provider: provider,
            identity,
        })
    }
    /// First publication is create-only. Truncation, unsafe ownership and root replacement refuse startup.
    pub(super) fn bind(
        &self,
        directory: &Path,
        scope: &SyncRootId,
        has_inventory: bool,
    ) -> EngineResult<()> {
        if !directory.exists() {
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(directory)
                .map_err(|_| unavailable())?;
        }
        let root = directory_handle(directory)?;
        let path = std::path::PathBuf::from(format!("/proc/self/fd/{}", root.as_raw_fd()))
            .join(format!("scope-{}.json", scope.0));
        if has_inventory && !path.try_exists().map_err(|_| unavailable())? {
            // Historical replicas cannot prove which physical root originally owned their slots.
            // Refuse rather than bootstrap identity onto a potentially replaced tree.
            return Err(unavailable());
        }
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(&serde_json::to_vec(&self.identity).map_err(|_| unavailable())?)
                    .map_err(|_| unavailable())?;
                file.sync_all().map_err(|_| unavailable())?;
                root.sync_all().map_err(|_| unavailable())?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let mut file = OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_NOFOLLOW)
                    .open(path)
                    .map_err(|_| unavailable())?;
                let meta = file.metadata().map_err(|_| unavailable())?;
                if !meta.is_file()
                    || meta.uid() != unsafe { libc::geteuid() }
                    || meta.mode() & 0o077 != 0
                    || meta.nlink() != 1
                {
                    return Err(unavailable());
                }
                let mut bytes = vec![];
                (&mut file)
                    .take(4097)
                    .read_to_end(&mut bytes)
                    .map_err(|_| unavailable())?;
                if bytes.len() > 4096
                    || serde_json::from_slice::<LocalIdentity>(&bytes).map_err(|_| unavailable())?
                        != self.identity
                {
                    return Err(unavailable());
                }
            }
            Err(_) => return Err(unavailable()),
        }
        Ok(())
    }
}
fn directory_handle(path: &Path) -> EngineResult<File> {
    let mut parent = directory(Path::new("/"))?;
    for p in path.components().skip(1) {
        let Component::Normal(name) = p else {
            return Err(unavailable());
        };
        parent = directory(
            &std::path::PathBuf::from(format!("/proc/self/fd/{}", parent.as_raw_fd())).join(name),
        )?;
    }
    let m = parent.metadata().map_err(|_| unavailable())?;
    if m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o077 != 0 {
        return Err(unavailable());
    }
    Ok(parent)
}

#[cfg(test)]
mod tests {
    use super::*;
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    fn fixture() -> (std::path::PathBuf, String) {
        let key = format!(
            "stage11-leases-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        let root = std::env::temp_dir().join(&key);
        std::fs::create_dir(&root).unwrap();
        for name in ["a", "a/child", "b"] {
            std::fs::create_dir(root.join(name)).unwrap();
        }
        (root, key)
    }
    #[test]
    fn writer_excludes_nested_alias_and_same_provider_but_allows_independent_accounts() {
        let _serial = SERIAL.lock().unwrap();
        let (base, subject) = fixture();
        let first = WriterLease::acquire(&base.join("a"), "test", &subject).unwrap();
        assert!(WriterLease::acquire(&base.join("a/child"), "test", "child").is_err());
        assert!(WriterLease::acquire(&base.join("a/child/.."), "test", "alias").is_err());
        assert!(WriterLease::acquire(&base.join("a/./"), "test", "same-inode-alias").is_err());
        assert!(WriterLease::acquire(&base.join("b"), "test", &subject).is_err());
        let other =
            WriterLease::acquire(&base.join("b"), "test", &format!("{subject}-other")).unwrap();
        std::os::unix::fs::symlink(base.join("a"), base.join("link")).unwrap();
        assert!(WriterLease::acquire(&base.join("link"), "test", "symlink").is_err());
        drop(other);
        drop(first);
        let child = WriterLease::acquire(&base.join("a/child"), "test", &subject).unwrap();
        assert!(WriterLease::acquire(&base.join("a"), "test", "parent").is_err());
        drop(child);
        std::fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn same_scope_reopens_but_replaced_root_is_refused_without_touching_either_tree() {
        let _serial = SERIAL.lock().unwrap();
        let (base, subject) = fixture();
        let storage = base.join("storage");
        let scope = SyncRootId("test-scope".into());
        let first = WriterLease::acquire(&base.join("a"), "test", &subject).unwrap();
        first.bind(&storage, &scope, false).unwrap();
        drop(first);
        let reopened = WriterLease::acquire(&base.join("a"), "test", &subject).unwrap();
        reopened.bind(&storage, &scope, false).unwrap();
        drop(reopened);
        std::fs::write(base.join("a/original"), b"preserved").unwrap();
        std::fs::rename(base.join("a"), base.join("preserved-root")).unwrap();
        std::fs::create_dir(base.join("a")).unwrap();
        let changed = WriterLease::acquire(&base.join("a"), "test", &subject).unwrap();
        assert!(changed.bind(&storage, &scope, false).is_err());
        drop(changed);
        assert_eq!(
            std::fs::read(base.join("preserved-root/original")).unwrap(),
            b"preserved"
        );
        assert_eq!(std::fs::read_dir(base.join("a")).unwrap().count(), 0);
        std::fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn lease_probe() {
        let Ok(path) = std::env::var("DRIVESYNC_LEASE_PROBE_ROOT") else {
            return;
        };
        assert!(WriterLease::acquire(
            Path::new(&path),
            "test",
            &std::env::var("DRIVESYNC_LEASE_SUBJECT").unwrap()
        )
        .is_err());
    }
    #[test]
    #[ignore = "must run in a separate test process: fork can temporarily inherit other parallel account leases until exec"]
    fn another_process_cannot_write_the_same_provider_account_from_a_different_root() {
        let _serial = SERIAL.lock().unwrap();
        let (base, subject) = fixture();
        let lease = WriterLease::acquire(&base.join("a"), "test", &subject).unwrap();
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "connection::scope::tests::lease_probe",
                "--nocapture",
            ])
            .env("DRIVESYNC_LEASE_PROBE_ROOT", base.join("b"))
            .env("DRIVESYNC_LEASE_SUBJECT", &subject)
            .status()
            .unwrap();
        assert!(result.success());
        drop(lease);
        std::fs::remove_dir_all(base).unwrap();
    }
}
