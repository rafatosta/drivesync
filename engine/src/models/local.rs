//! Selected local observation data and structural validation, without I/O.
use super::{LocalFileId, MetadataValue, ModelError, ModelResult};

/// Nonempty Unix path bytes below a synchronization root.
///
/// Construction checks syntax only: it does not prove existence or confinement
/// against symlinks, mounts or concurrent filesystem changes. Root observations
/// use a whole-sync scope rather than an empty entry path.
///
/// ```
/// use drivesync_engine::models::LocalRelativePath;
/// let path = LocalRelativePath::new(b"folder/non-utf8-\xff".to_vec())?;
/// assert_eq!(path.as_bytes(), b"folder/non-utf8-\xff");
/// # Ok::<(), drivesync_engine::models::ModelError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalRelativePath(Vec<u8>);

impl LocalRelativePath {
    /// Preserves Unix bytes, rejecting root, NUL and ambiguous/escaping components.
    ///
    /// Returns `InvalidRelativePath` for empty, absolute, trailing-slash or
    /// repeated-slash paths and components exactly `.` or `..`. No I/O,
    /// decoding, normalization or policy filtering is performed.
    pub fn new(bytes: Vec<u8>) -> ModelResult<Self> {
        if bytes.contains(&0)
            || bytes
                .split(|byte| *byte == b'/')
                .any(|part| part.is_empty() || part == b"." || part == b"..")
        {
            return Err(ModelError::InvalidRelativePath);
        }
        Ok(Self(bytes))
    }

    /// Borrows validated bytes without copying, decoding or resolving a path.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Local filesystem time, preserving signed seconds and nanosecond precision.
///
/// `ctime` uses this representation but is not a creation timestamp. Public
/// fields allow collector assembly; enclosing metadata validation also checks
/// the fraction so struct literals cannot bypass that validation boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTimestamp {
    pub unix_seconds: i64,
    pub nanoseconds: u32,
}

impl LocalTimestamp {
    /// Preserves seconds and a fraction below one billion nanoseconds.
    ///
    /// Returns `InvalidTimestamp` for an out-of-range fraction, without carrying
    /// into seconds, rounding, reading a clock or applying a timezone.
    pub fn new(unix_seconds: i64, nanoseconds: u32) -> ModelResult<Self> {
        if nanoseconds >= 1_000_000_000 {
            return Err(ModelError::InvalidTimestamp);
        }
        Ok(Self {
            unix_seconds,
            nanoseconds,
        })
    }
}

/// Observed entry type; `Unknown` is retained uncertainty, not a regular file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalEntryKind {
    Regular,
    Directory,
    Symlink,
    Fifo,
    Socket,
    BlockDevice,
    CharacterDevice,
    Other,
    Unknown,
}

/// Filesystem device numbers, sampled evidence rather than a stable identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceNumber {
    pub major: u32,
    pub minor: u32,
}

/// Raw Linux mode and its explicit rwx/setuid/setgid/sticky permission bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalFileMode {
    pub raw_mode: u32,
    pub permission_bits: u16,
}

/// Numeric ownership evidence; these values never authorize an operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalOwner {
    pub uid: MetadataValue<u32>,
    pub gid: MetadataValue<u32>,
}

/// Modification/status-change times and optional birth time; excludes atime.
/// No relationship between these timestamps establishes version precedence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalTimes {
    pub modified_at: MetadataValue<LocalTimestamp>,
    pub changed_at: MetadataValue<LocalTimestamp>,
    pub created_at: MetadataValue<LocalTimestamp>,
}

/// Sampled size, alias and device/inode evidence, without content hashes.
/// Size has the meaning reported for the entry type, not necessarily file bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalStorage {
    pub size_bytes: MetadataValue<u64>,
    pub hard_link_count: MetadataValue<u64>,
    pub inode: MetadataValue<u64>,
    pub filesystem_device: MetadataValue<DeviceNumber>,
}

/// Optional raw statx attributes and the corresponding support mask.
/// Only bits covered by the mask have supported semantics; raw bits are retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFileFlags {
    pub statx_attributes: MetadataValue<u64>,
    pub statx_attributes_mask: MetadataValue<u64>,
}

/// Literal symlink target bytes; never interpreted as a confined entry path.
/// Known non-symlink entries use `MetadataValue::NotApplicable`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalSymlink {
    pub target: MetadataValue<Vec<u8>>,
}

/// Selected local metadata evidence, assembled by the separate filesystem collector.
///
/// This record can be partial. It neither performs collection nor determines
/// physical coverage, persistent identity, content equality or deletion policy.
/// New local records intentionally have no persistence format in this stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFile {
    pub local_id: Option<LocalFileId>,
    pub relative_path: LocalRelativePath,
    pub kind: MetadataValue<LocalEntryKind>,
    pub mode: MetadataValue<LocalFileMode>,
    pub owner: LocalOwner,
    pub times: LocalTimes,
    pub storage: LocalStorage,
    pub flags: LocalFileFlags,
    pub symlink: LocalSymlink,
}

impl LocalFile {
    /// Checks structural consistency without reading, mutating or completing evidence.
    ///
    /// Returns `EmptyIdentity` for a supplied blank logical ID,
    /// `InvalidTimestamp` for invalid known fractions, and `InconsistentMetadata`
    /// for contradictory modes, flags or applicability. Coherent unknown/partial
    /// evidence and real zero values are valid. No timestamp ordering, physical
    /// coverage, identity correlation or filesystem access is inferred.
    pub fn validate(&self) -> ModelResult<()> {
        use MetadataValue::{Known, NotApplicable};
        if self
            .local_id
            .as_ref()
            .is_some_and(|id| id.0.trim().is_empty())
        {
            return Err(ModelError::EmptyIdentity);
        }
        applicable(&self.kind)?;
        applicable(&self.mode)?;
        for value in [&self.owner.uid, &self.owner.gid] {
            applicable(value)?;
        }
        for value in [
            &self.times.modified_at,
            &self.times.changed_at,
            &self.times.created_at,
        ] {
            applicable(value)?;
            if let Known(time) = value {
                LocalTimestamp::new(time.unix_seconds, time.nanoseconds)?;
            }
        }
        for value in [
            &self.storage.size_bytes,
            &self.storage.hard_link_count,
            &self.storage.inode,
            &self.flags.statx_attributes,
            &self.flags.statx_attributes_mask,
        ] {
            applicable(value)?;
        }
        applicable(&self.storage.filesystem_device)?;

        if let Known(mode) = &self.mode {
            if mode.permission_bits != (mode.raw_mode & 0o7777) as u16 {
                return Err(ModelError::InconsistentMetadata);
            }
            if let Known(kind) = self.kind {
                if kind != LocalEntryKind::Unknown && kind != mode_kind(mode.raw_mode) {
                    return Err(ModelError::InconsistentMetadata);
                }
            }
        }

        let flags = &self.flags;
        if !matches!(
            (&flags.statx_attributes, &flags.statx_attributes_mask),
            (Known(_), Known(_))
        ) && flags.statx_attributes != flags.statx_attributes_mask
        {
            return Err(ModelError::InconsistentMetadata);
        }

        let target_valid = match self.kind {
            Known(kind) if !matches!(kind, LocalEntryKind::Symlink | LocalEntryKind::Unknown) => {
                matches!(self.symlink.target, NotApplicable)
            }
            _ => !matches!(self.symlink.target, NotApplicable),
        };
        if !target_valid {
            return Err(ModelError::InconsistentMetadata);
        }
        Ok(())
    }
}

/// Attributes other than a non-symlink target cannot be marked non-applicable.
fn applicable<T>(value: &MetadataValue<T>) -> ModelResult<()> {
    if matches!(value, MetadataValue::NotApplicable) {
        Err(ModelError::InconsistentMetadata)
    } else {
        Ok(())
    }
}

/// Interprets only Linux file-type bits, without consulting the filesystem.
fn mode_kind(raw_mode: u32) -> LocalEntryKind {
    match raw_mode & libc::S_IFMT {
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
