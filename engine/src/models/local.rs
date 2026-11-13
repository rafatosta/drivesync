//! Unix entry paths and local timestamp values, without filesystem access.
use super::{ModelError, ModelResult};

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
