//! Physical collector behind the stable local metadata port, without events or scans.
use super::{
    filesystem::{self, DirectoryChain, Stamp},
    models::*,
};
use crate::models::*;
use std::{
    fs::{File, Metadata},
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::Component,
};

/// A root-bound collector retaining only a root descriptor between requests.
pub(crate) struct FilesystemMetadataCollector {
    scope: LocalDetectorScope,
    root: File,
}

/// Deterministic observation boundaries, also used by isolated race tests.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum CollectionStage {
    Opened,
    Observed,
}

impl FilesystemMetadataCollector {
    /// Opens a configured root without following links or creating anything.
    /// Errors are sanitized InvalidScope, RootUnavailable, PermissionDenied or ResourceLimit.
    pub(crate) fn new(scope: LocalDetectorScope) -> LocalDetectorResult<Self> {
        if scope.account_id.0.trim().is_empty()
            || scope.sync_id.0.trim().is_empty()
            || !scope.local_root.is_absolute()
            || scope.local_root.as_os_str().as_bytes().contains(&0)
            || scope
                .local_root
                .components()
                .any(|c| matches!(c, Component::ParentDir) || c.as_os_str() == ".drivesync")
        {
            return Err(LocalDetectorFailure::new(LocalDetectorError::InvalidScope));
        }
        let chain = DirectoryChain::open_root(&scope.local_root, &LocalCancellation::new())?;
        let root = chain
            .last()
            .try_clone()
            .map_err(|e| filesystem::failure(e, true))?;
        Ok(Self { scope, root })
    }

    /// Runs real collection with hooks at safe observation boundaries for deterministic tests.
    /// Production supplies a no-op hook; the collector itself and filesystem are never mocked.
    pub(super) fn collect_with(
        &self,
        request: LocalMetadataRequest,
        mut stage: impl FnMut(CollectionStage),
    ) -> LocalDetectorResult<LocalMetadataResult> {
        let cancel = &request.cancellation;
        cancel.check()?;
        if request
            .relative_path
            .as_bytes()
            .split(|b| *b == b'/')
            .any(|part| part == b".drivesync")
        {
            return Err(LocalDetectorFailure::new(LocalDetectorError::InvalidScope));
        }
        let mut chain = DirectoryChain::open_root(&self.scope.local_root, cancel)?;
        cancel.check()?;
        let actual = chain
            .last()
            .metadata()
            .map_err(|e| filesystem::failure(e, true))?;
        cancel.check()?;
        let original = self
            .root
            .metadata()
            .map_err(|e| filesystem::failure(e, true))?;
        if !filesystem::same_entry(&actual, &original) {
            return Err(LocalDetectorFailure::new(
                LocalDetectorError::RootUnavailable,
            ));
        }
        let Some((entry, name)) = chain.open_entry(&request.relative_path, cancel)? else {
            return Ok(inconclusive());
        };
        cancel.check()?;
        let before = entry
            .metadata()
            .map_err(|e| filesystem::failure(e, false))?;
        stage(CollectionStage::Opened);
        chain.validate(cancel)?;
        if !filesystem::entry_still_bound(&chain, &name, &before, cancel)? {
            return Ok(inconclusive());
        }
        cancel.check()?;
        let (file, coverage) = basic_metadata(&entry, &before, request.relative_path, cancel)?;
        stage(CollectionStage::Observed);
        chain.validate(cancel)?;
        if !filesystem::entry_still_bound(&chain, &name, &before, cancel)? {
            return Ok(inconclusive());
        }
        cancel.check()?;
        let after = entry
            .metadata()
            .map_err(|e| filesystem::failure(e, false))?;
        if Stamp::from(&before) != Stamp::from(&after) {
            return Ok(inconclusive());
        }
        cancel.check()?;
        file.validate()
            .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::ModelInvalid))?;
        Ok(LocalMetadataResult {
            file: Some(file),
            coverage,
        })
    }
}
impl LocalMetadataCollector for FilesystemMetadataCollector {
    fn scope(&self) -> LocalDetectorScope {
        self.scope.clone()
    }
    fn collect(&self, request: LocalMetadataRequest) -> LocalDetectorResult<LocalMetadataResult> {
        self.collect_with(request, |_| {})
    }
}

fn inconclusive() -> LocalMetadataResult {
    LocalMetadataResult {
        file: None,
        coverage: LocalScanCoverage::Partial,
    }
}

/// Maps only the fields approved in Models; no content, ACL or xattr queries occur.
fn basic_metadata(
    entry: &File,
    m: &Metadata,
    path: LocalRelativePath,
    cancel: &LocalCancellation,
) -> LocalDetectorResult<(LocalFile, LocalScanCoverage)> {
    use MetadataValue::{Known, NotApplicable, NotCollected};
    let kind = entry_kind(m.mode());
    let mut coverage = LocalScanCoverage::Complete;
    let target = if kind == LocalEntryKind::Symlink {
        match filesystem::read_link(entry, cancel)? {
            Ok(bytes) => Known(bytes),
            Err(error) => {
                coverage = LocalScanCoverage::Partial;
                if error.kind() == std::io::ErrorKind::PermissionDenied {
                    MetadataValue::PermissionDenied
                } else {
                    MetadataValue::Unavailable
                }
            }
        }
    } else {
        NotApplicable
    };
    let file = LocalFile {
        local_id: None,
        relative_path: path,
        kind: Known(kind),
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
            created_at: NotCollected,
        },
        storage: LocalStorage {
            size_bytes: Known(m.size()),
            hard_link_count: Known(m.nlink()),
            inode: Known(m.ino()),
            filesystem_device: Known(DeviceNumber {
                major: libc::major(m.dev()),
                minor: libc::minor(m.dev()),
            }),
        },
        flags: LocalFileFlags {
            statx_attributes: NotCollected,
            statx_attributes_mask: NotCollected,
        },
        symlink: LocalSymlink { target },
    };
    Ok((file, coverage))
}
fn timestamp(seconds: i64, nanos: i64) -> LocalDetectorResult<LocalTimestamp> {
    let nanos = u32::try_from(nanos)
        .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::ModelInvalid))?;
    LocalTimestamp::new(seconds, nanos)
        .map_err(|_| LocalDetectorFailure::new(LocalDetectorError::ModelInvalid))
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
