//! Schema-aware metadata adapter. Detector control never enumerates LocalFile fields.
use super::{filesystem, models::*};
use crate::models::*;
use std::{
    fs::{File, Metadata},
    io,
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
        crate::filesystem::metadata::statx(file)
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
        Ok(stat) => crate::filesystem::metadata::from_statx(&stat, before, path)
            .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::ModelInvalid))?,
        Err(error) => crate::filesystem::metadata::from_fstat(before, path, &error)
            .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::ModelInvalid))?,
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

fn absence<T>(error: &io::Error) -> MetadataValue<T> {
    match error.raw_os_error() {
        Some(libc::ENOSYS | libc::EOPNOTSUPP) => MetadataValue::Unsupported,
        Some(libc::EPERM | libc::EACCES) => MetadataValue::PermissionDenied,
        _ => MetadataValue::Unavailable,
    }
}
