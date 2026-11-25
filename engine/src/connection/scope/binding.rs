//! Persistent admission proof, independent of the runtime identity held by the writer lease.
use super::{directory, directory_handle, unavailable};
use crate::{
    connection::{synchronization, EngineResult},
    models::{LocalIdentity, SyncRootId},
    SynchronizationErrorKind,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Identity {
    Btrfs {
        filesystem: [u8; 16],
        subvolume: u64,
        inode: u64,
        birth_ns: u128,
    },
    Native(LocalIdentity),
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    scope: String,
    token: [u8; 32],
    identity: Identity,
}

#[repr(C)]
struct FilesystemInfo {
    max_id: u64,
    num_devices: u64,
    fsid: [u8; 16],
    remaining: [u8; 992],
}

#[repr(C)]
struct InodeLookup {
    tree_id: u64,
    object_id: u64,
    name: [u8; 4080],
}

fn identity(root: &File, runtime: &LocalIdentity) -> EngineResult<Identity> {
    let mut stat = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // A successful fstatfs initializes the output buffer.
    if unsafe { libc::fstatfs(root.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
        return Err(unavailable());
    }
    if unsafe { stat.assume_init() }.f_type != libc::BTRFS_SUPER_MAGIC {
        return Ok(Identity::Native(runtime.clone()));
    }

    // Linux UAPI: FS_INFO returns the filesystem UUID. INO_LOOKUP with tree 0
    // and object BTRFS_FIRST_FREE_OBJECTID returns the descriptor's subvolume.
    let mut fs_info = FilesystemInfo {
        max_id: 0,
        num_devices: 0,
        fsid: [0; 16],
        remaining: [0; 992],
    };
    let mut lookup = InodeLookup {
        tree_id: 0,
        object_id: 256,
        name: [0; 4080],
    };
    let fd = root.as_raw_fd();
    if unsafe { libc::ioctl(fd, libc::_IOR::<FilesystemInfo>(0x94, 31), &mut fs_info) } != 0
        || unsafe { libc::ioctl(fd, libc::_IOWR::<InodeLookup>(0x94, 18), &mut lookup) } != 0
        || fs_info.fsid == [0; 16]
        || lookup.tree_id == 0
    {
        return Err(unavailable());
    }
    Ok(Identity::Btrfs {
        filesystem: fs_info.fsid,
        subvolume: lookup.tree_id,
        inode: runtime.inode,
        birth_ns: runtime.birth_ns.ok_or_else(unavailable)?,
    })
}

fn changed() -> crate::EngineError {
    synchronization::error(SynchronizationErrorKind::LocalRootIdentityChanged)
}

fn fd_path(directory: &File, name: &str) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd())).join(name)
}

fn read(path: &Path) -> EngineResult<Option<Vec<u8>>> {
    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(changed()),
    };
    let meta = file.metadata().map_err(|_| changed())?;
    if !meta.is_file()
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.mode() & 0o077 != 0
        || meta.nlink() != 1
    {
        return Err(changed());
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(4097)
        .read_to_end(&mut bytes)
        .map_err(|_| changed())?;
    if bytes.len() > 4096 {
        return Err(changed());
    }
    Ok(Some(bytes))
}

fn random() -> io::Result<[u8; 32]> {
    let mut token = [0; 32];
    File::open("/dev/urandom")?.read_exact(&mut token)?;
    Ok(token)
}

fn publish(directory: &File, target: &Path, binding: &Binding) -> EngineResult<()> {
    let bytes = serde_json::to_vec(binding).map_err(|_| unavailable())?;
    let random = random().map_err(|_| unavailable())?;
    let name: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
    let temp = fd_path(directory, &format!(".binding-{name}"));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)
        .map_err(|_| unavailable())?;
    let result = (|| -> io::Result<()> {
        file.write_all(&bytes)?;
        file.sync_all()?;
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        let from = CString::new(temp.as_os_str().as_bytes())?;
        let to = CString::new(target.as_os_str().as_bytes())?;
        if unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                from.as_ptr(),
                libc::AT_FDCWD,
                to.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
        directory.sync_all()
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result.map_err(|_| unavailable())
}

pub(super) fn verify(
    root: &File,
    runtime: &LocalIdentity,
    storage: &Path,
    scope: &SyncRootId,
    has_inventory: bool,
) -> EngineResult<()> {
    if !storage.exists() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(storage)
            .map_err(|_| unavailable())?;
    }
    let storage = directory_handle(storage)?;
    let saved_path = fd_path(&storage, &format!("scope-{}.json", scope.0));
    let saved: Option<Binding> = read(&saved_path)?
        .map(|bytes| serde_json::from_slice(&bytes).map_err(|_| changed()))
        .transpose()?;
    if saved.is_none() && has_inventory {
        return Err(unavailable());
    }
    let identity = identity(root, runtime)?;
    if saved
        .as_ref()
        .is_some_and(|binding| binding.scope != scope.0 || binding.identity != identity)
    {
        return Err(changed());
    }

    let metadata = fd_path(root, ".drivesync");
    if saved.is_none() {
        match fs::DirBuilder::new().mode(0o700).create(&metadata) {
            Ok(()) => root.sync_all().map_err(|_| unavailable())?,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(unavailable()),
        }
    }
    let metadata = directory(&metadata).map_err(|_| changed())?;
    let attributes = metadata.metadata().map_err(|_| changed())?;
    if attributes.uid() != unsafe { libc::geteuid() } || attributes.mode() & 0o077 != 0 {
        return Err(changed());
    }
    let marker_path = fd_path(&metadata, &format!("root-{}.json", scope.0));
    let marker: Option<Binding> = read(&marker_path)?
        .map(|bytes| serde_json::from_slice(&bytes).map_err(|_| changed()))
        .transpose()?;
    if let Some(binding) = saved {
        return if marker.as_ref() == Some(&binding) {
            Ok(())
        } else {
            Err(changed())
        };
    }

    let marker = match marker {
        Some(marker) if marker.scope == scope.0 && marker.identity == identity => marker,
        Some(_) => return Err(changed()),
        None => {
            let marker = Binding {
                scope: scope.0.clone(),
                token: random().map_err(|_| unavailable())?,
                identity,
            };
            publish(&metadata, &marker_path, &marker)?;
            marker
        }
    };
    publish(&storage, &saved_path, &marker)
}
