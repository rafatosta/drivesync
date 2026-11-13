//! Schema-aware metadata adapter. Detector control never enumerates LocalFile fields.
use super::{filesystem, models::*};
use crate::models::*;
use std::{
    fs::{File, Metadata},
    io,
    mem::MaybeUninit,
    os::{fd::AsRawFd, unix::fs::MetadataExt},
};

/// Thin syscall boundary for isolated failures; navigation and collection stay real.
pub(super) trait MetadataReader {
    /// Reads descriptor-bound statx data, preserving the kernel's field masks.
    fn statx(&self, file: &File) -> io::Result<libc::statx>;
    /// Reads literal bytes from the held symlink, never its destination.
    fn read_link(
        &self,
        file: &File,
        cancel: &LocalCancellation,
    ) -> LocalDetectorResult<io::Result<Vec<u8>>> {
        filesystem::read_link(file, cancel)
    }
}

/// Real Linux syscall implementation, without configurable external endpoints.
pub(super) struct LinuxMetadataReader;
impl MetadataReader for LinuxMetadataReader {
    fn statx(&self, file: &File) -> io::Result<libc::statx> {
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
}

/// Builds only the selected metadata profile, retaining precise absence states.
pub(super) fn collect(
    entry: &File,
    before: &Metadata,
    path: LocalRelativePath,
    cancel: &LocalCancellation,
    reader: &impl MetadataReader,
) -> LocalDetectorResult<(LocalFile, LocalScanCoverage)> {
    cancel.check()?;
    let native = reader.statx(entry);
    cancel.check()?;
    let mut file = match native {
        Ok(stat) => from_statx(&stat, before, path)?,
        Err(error) => from_fstat(before, path, &error)?,
    };
    // fstat on the same held fd is reliable evidence for deciding whether to
    // attempt readlink, even if statx omitted its type mask. It is not projected
    // into a missing statx field or used to fabricate a complete profile.
    if before.file_type().is_symlink() {
        cancel.check()?;
        file.symlink.target = match reader.read_link(entry, cancel)? {
            Ok(bytes) => MetadataValue::Known(bytes),
            Err(error) => absence(&error),
        };
        cancel.check()?;
    }
    let coverage = if file.basic_profile_complete() {
        LocalScanCoverage::Complete
    } else {
        LocalScanCoverage::Partial
    };
    Ok((file, coverage))
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
) -> LocalDetectorResult<MetadataValue<LocalTimestamp>> {
    if mask & bit == 0 {
        return Ok(MetadataValue::Unavailable);
    }
    Ok(MetadataValue::Known(
        LocalTimestamp::new(time.tv_sec, time.tv_nsec)
            .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::ModelInvalid))?,
    ))
}
fn from_statx(
    s: &libc::statx,
    before: &Metadata,
    path: LocalRelativePath,
) -> LocalDetectorResult<LocalFile> {
    let kind = field(s.stx_mask, libc::STATX_TYPE, entry_kind(s.stx_mode as u32));
    let target = non_link_target(&kind);
    Ok(LocalFile {
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
    })
}
fn from_fstat(
    m: &Metadata,
    path: LocalRelativePath,
    statx_error: &io::Error,
) -> LocalDetectorResult<LocalFile> {
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
fn timestamp(seconds: i64, nanos: i64) -> LocalDetectorResult<LocalTimestamp> {
    let nanos = u32::try_from(nanos)
        .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::ModelInvalid))?;
    LocalTimestamp::new(seconds, nanos)
        .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::ModelInvalid))
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
