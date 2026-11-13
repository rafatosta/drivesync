//! Physical collector behind the stable local metadata port, without events or scans.
use super::{
    filesystem::{self, DirectoryChain, Stamp},
    metadata::{self, LinuxMetadataReader, MetadataReader},
    models::*,
};
use std::{fs::File, os::unix::ffi::OsStrExt, path::Component};

/// A root-bound collector retaining only a root descriptor between requests.
pub struct FilesystemMetadataCollector {
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
    pub fn new(scope: LocalDetectorScope) -> LocalDetectorResult<Self> {
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

    /// Returns the original account/synchronization/root binding without disk I/O.
    /// This remains available when the root is inaccessible or has been replaced.
    pub fn scope(&self) -> LocalDetectorScope {
        self.scope.clone()
    }

    /// Collects the selected metadata for one entry without reading file contents.
    ///
    /// This synchronous operation must run off the UI thread. It checks the
    /// caller's cancellation token, refuses symbolic ancestors/reserved paths,
    /// and revalidates the held root and entry. A missing entry yields no file
    /// with Partial coverage, never confirmed deletion. Typed failures contain
    /// sanitized diagnostics; cancellation does not undo earlier observations.
    pub fn collect(
        &self,
        request: LocalMetadataRequest,
    ) -> LocalDetectorResult<LocalMetadataResult> {
        self.collect_with(request, |_| {})
    }

    /// Revalidates the configured path against the root pinned by this instance.
    pub(super) fn root_chain(
        &self,
        cancel: &LocalCancellation,
    ) -> LocalDetectorResult<DirectoryChain> {
        let chain = DirectoryChain::open_root(&self.scope.local_root, cancel)?;
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
        Ok(chain)
    }

    /// Runs real collection with hooks at safe observation boundaries for deterministic tests.
    /// Production supplies a no-op hook; the collector itself and filesystem are never mocked.
    pub(super) fn collect_with(
        &self,
        request: LocalMetadataRequest,
        stage: impl FnMut(CollectionStage),
    ) -> LocalDetectorResult<LocalMetadataResult> {
        self.collect_using(request, stage, &LinuxMetadataReader)
    }

    /// Keeps syscall fault injection private to the physical adapter boundary.
    pub(super) fn collect_using(
        &self,
        request: LocalMetadataRequest,
        mut stage: impl FnMut(CollectionStage),
        reader: &impl MetadataReader,
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
        let mut chain = self.root_chain(cancel)?;
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
        let (file, coverage) =
            metadata::collect(&entry, &before, request.relative_path, cancel, reader)?;
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
        Self::scope(self)
    }
    fn collect(&self, request: LocalMetadataRequest) -> LocalDetectorResult<LocalMetadataResult> {
        Self::collect(self, request)
    }
}

fn inconclusive() -> LocalMetadataResult {
    LocalMetadataResult {
        file: None,
        coverage: LocalScanCoverage::Partial,
    }
}
