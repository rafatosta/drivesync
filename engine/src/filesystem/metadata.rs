//! Shared Linux descriptor metadata projection; no detector, inventory or traversal policy.
use crate::models::*;
use std::{
    fs::{File, Metadata},
    io,
    mem::MaybeUninit,
    os::{fd::AsRawFd, unix::fs::MetadataExt},
};
/// Reads only metadata for a held descriptor, without following links or opening content.
pub(crate) fn statx(file: &File) -> io::Result<libc::statx> {
    let mut stat = MaybeUninit::<libc::statx>::zeroed();
    // SAFETY: file remains live, empty path is NUL-terminated, and the output
    // pointer is valid for the initialized statx structure. No pathname follows.
    let result = unsafe {
        libc::statx(
            file.as_raw_fd(),
            c"".as_ptr(),
            libc::AT_EMPTY_PATH | libc::AT_SYMLINK_NOFOLLOW,
            libc::STATX_BASIC_STATS | libc::STATX_BTIME,
            stat.as_mut_ptr(),
        )
    };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: zero initialization covers all fields; successful statx populated
    // supported fields, and callers check the mask before trusting those values.
    Ok(unsafe { stat.assume_init() })
}

/// Maps a requested field missing from a successful response to unavailable,
/// never to a fabricated zero or an unproven assertion of backend support.
fn field<T>(mask: u32, bit: u32, value: T) -> MetadataValue<T> {
    if mask & bit == bit {
        MetadataValue::Known(value)
    } else {
        MetadataValue::Unavailable
    }
}
fn time_field(
    mask: u32,
    bit: u32,
    time: &libc::statx_timestamp,
) -> ModelResult<MetadataValue<LocalTimestamp>> {
    if mask & bit == 0 {
        return Ok(MetadataValue::Unavailable);
    }
    Ok(MetadataValue::Known(LocalTimestamp::new(
        time.tv_sec,
        time.tv_nsec,
    )?))
}
pub(crate) fn from_statx(
    s: &libc::statx,
    before: &Metadata,
    path: LocalRelativePath,
) -> ModelResult<LocalFile> {
    let kind = field(s.stx_mask, libc::STATX_TYPE, entry_kind(s.stx_mode as u32));
    let target = non_link_target(&kind);
    let mut file = LocalFile {
        local_id: None,
        relative_path: path,
        kind,
        mode: field(
            s.stx_mask,
            libc::STATX_TYPE | libc::STATX_MODE,
            LocalFileMode {
                raw_mode: s.stx_mode as u32,
                permission_bits: s.stx_mode & 0o7777,
            },
        ),
        owner: LocalOwner {
            uid: field(s.stx_mask, libc::STATX_UID, s.stx_uid),
            gid: field(s.stx_mask, libc::STATX_GID, s.stx_gid),
        },
        times: LocalTimes {
            modified_at: time_field(s.stx_mask, libc::STATX_MTIME, &s.stx_mtime)?,
            changed_at: time_field(s.stx_mask, libc::STATX_CTIME, &s.stx_ctime)?,
            created_at: time_field(s.stx_mask, libc::STATX_BTIME, &s.stx_btime)?,
        },
        storage: LocalStorage {
            size_bytes: field(s.stx_mask, libc::STATX_SIZE, s.stx_size),
            hard_link_count: field(s.stx_mask, libc::STATX_NLINK, s.stx_nlink as u64),
            inode: field(s.stx_mask, libc::STATX_INO, s.stx_ino),
            filesystem_device: device(before),
        },
        flags: LocalFileFlags {
            statx_attributes: MetadataValue::Known(s.stx_attributes),
            statx_attributes_mask: MetadataValue::Known(s.stx_attributes_mask),
        },
        symlink: LocalSymlink { target },
    };
    // Bind local identity to birth time as well as device/inode; inode reuse or
    // multiple hardlink names must not silently become the same logical entry.
    if let (MetadataValue::Known(created), MetadataValue::Known(inode)) =
        (&file.times.created_at, &file.storage.inode)
    {
        if before.is_dir() || before.is_file() && before.nlink() == 1 {
            file.local_id = Some(LocalFileId(format!(
                "linux:{}:{}:{}:{}",
                before.dev(),
                inode,
                created.unix_seconds,
                created.nanoseconds
            )));
        }
    }
    Ok(file)
}
pub(crate) fn from_fstat(
    m: &Metadata,
    path: LocalRelativePath,
    statx_error: &io::Error,
) -> ModelResult<LocalFile> {
    use MetadataValue::Known;
    let kind = Known(entry_kind(m.mode()));
    let target = non_link_target(&kind);
    Ok(LocalFile {
        local_id: None,
        relative_path: path,
        kind,
        mode: Known(LocalFileMode {
            raw_mode: m.mode(),
            permission_bits: (m.mode() & 0o7777) as u16,
        }),
        owner: LocalOwner {
            uid: Known(m.uid()),
            gid: Known(m.gid()),
        },
        times: LocalTimes {
            modified_at: Known(timestamp(m.mtime(), m.mtime_nsec())?),
            changed_at: Known(timestamp(m.ctime(), m.ctime_nsec())?),
            created_at: absence(statx_error),
        },
        storage: LocalStorage {
            size_bytes: Known(m.size()),
            hard_link_count: Known(m.nlink()),
            inode: Known(m.ino()),
            filesystem_device: device(m),
        },
        flags: LocalFileFlags {
            statx_attributes: absence(statx_error),
            statx_attributes_mask: absence(statx_error),
        },
        symlink: LocalSymlink { target },
    })
}
fn device(m: &Metadata) -> MetadataValue<DeviceNumber> {
    MetadataValue::Known(DeviceNumber {
        major: libc::major(m.dev()),
        minor: libc::minor(m.dev()),
    })
}
fn non_link_target(kind: &MetadataValue<LocalEntryKind>) -> MetadataValue<Vec<u8>> {
    match kind {
        MetadataValue::Known(kind)
            if !matches!(kind, LocalEntryKind::Symlink | LocalEntryKind::Unknown) =>
        {
            MetadataValue::NotApplicable
        }
        _ => MetadataValue::NotCollected,
    }
}
fn timestamp(seconds: i64, nanos: i64) -> ModelResult<LocalTimestamp> {
    let nanos = u32::try_from(nanos).map_err(|_| ModelError::InvalidTimestamp)?;
    LocalTimestamp::new(seconds, nanos)
}
/// A failed query never carries its raw OS text into the observation.
fn absence<T>(error: &io::Error) -> MetadataValue<T> {
    match error.raw_os_error() {
        Some(libc::ENOSYS | libc::EOPNOTSUPP) => MetadataValue::Unsupported,
        Some(libc::EPERM | libc::EACCES) => MetadataValue::PermissionDenied,
        _ => MetadataValue::Unavailable,
    }
}
fn entry_kind(mode: u32) -> LocalEntryKind {
    match mode & libc::S_IFMT {
        libc::S_IFREG => LocalEntryKind::Regular,
        libc::S_IFDIR => LocalEntryKind::Directory,
        libc::S_IFLNK => LocalEntryKind::Symlink,
        libc::S_IFIFO => LocalEntryKind::Fifo,
        libc::S_IFSOCK => LocalEntryKind::Socket,
        libc::S_IFBLK => LocalEntryKind::BlockDevice,
        libc::S_IFCHR => LocalEntryKind::CharacterDevice,
        _ => LocalEntryKind::Other,
    }
}
