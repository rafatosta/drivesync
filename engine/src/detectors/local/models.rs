//! Private collector contracts, without the unimplemented event envelope.
use crate::models::{AccountId, LocalFile, LocalRelativePath, SyncId};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

/// Immutable operational binding; not a filesystem or logical item identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalDetectorScope {
    pub sync_id: SyncId,
    pub account_id: AccountId,
    pub local_root: PathBuf,
}

/// Read coverage, never proof of absence, identity or content integrity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalScanCoverage {
    Complete,
    Partial,
    Unavailable,
}

/// Shared detector errors; only collector-relevant variants are exercised here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalDetectorError {
    InvalidScope,
    InvalidState,
    RootUnavailable,
    PermissionDenied,
    PartialObservation,
    WatcherUnavailable,
    ResourceLimit,
    Busy,
    ModelInvalid,
    PreconditionFailed,
    Cancelled,
    NotificationFailed,
    Internal,
}

/// Planned scan counters, without implementing or scheduling a scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalScanResult {
    pub coverage: LocalScanCoverage,
    pub observed_count: u64,
    pub emitted_count: u64,
}

/// Sanitized failure; a single metadata collection has no scan progress.
#[derive(Debug, Clone)]
pub(crate) struct LocalDetectorFailure {
    pub kind: LocalDetectorError,
    pub message: String,
    pub retryable: bool,
    pub progress: Option<LocalScanResult>,
}
impl LocalDetectorFailure {
    /// Uses fixed diagnostics, never OS errors or user-controlled paths.
    pub(super) fn new(kind: LocalDetectorError) -> Self {
        use LocalDetectorError::*;
        let (message, retryable) = match kind {
            InvalidScope => ("invalid local scope", false),
            RootUnavailable => ("local root unavailable", true),
            PermissionDenied => ("local metadata access denied", true),
            PartialObservation => ("local observation requires recheck", true),
            ResourceLimit => ("local observation resource limit", true),
            ModelInvalid => ("invalid local metadata", false),
            Cancelled => ("local observation cancelled", false),
            _ => ("local detector failure", false),
        };
        Self {
            kind,
            message: message.into(),
            retryable,
            progress: None,
        }
    }
}
impl std::fmt::Display for LocalDetectorFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for LocalDetectorFailure {}

/// Result of internal local observation operations.
pub(crate) type LocalDetectorResult<T> = Result<T, LocalDetectorFailure>;

/// Cloneable generation token; only detector control should call `cancel`.
#[derive(Debug, Clone)]
pub(crate) struct LocalCancellation(Arc<AtomicBool>);
impl LocalCancellation {
    /// Creates an independent, uncancelled generation.
    pub(super) fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }
    /// Cancels this generation irreversibly, including every existing clone.
    pub(super) fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    /// Reads the cancellation flag without I/O or side effects.
    pub(crate) fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    /// Checks cancellation before starting another I/O operation.
    pub(super) fn check(&self) -> LocalDetectorResult<()> {
        if self.is_cancelled() {
            Err(LocalDetectorFailure::new(LocalDetectorError::Cancelled))
        } else {
            Ok(())
        }
    }
}

/// One validated entry path and its cooperative cancellation generation.
pub(crate) struct LocalMetadataRequest {
    pub relative_path: LocalRelativePath,
    pub cancellation: LocalCancellation,
}
/// Complete selected metadata, or explicit uncertainty; no parallel event list.
#[derive(Debug)]
pub(crate) struct LocalMetadataResult {
    pub file: Option<LocalFile>,
    pub coverage: LocalScanCoverage,
}
/// Stable collection boundary independent of LocalFile's field layout.
pub(crate) trait LocalMetadataCollector: Send + Sync {
    /// Returns the original configuration without accessing the filesystem.
    fn scope(&self) -> LocalDetectorScope;
    /// Observes one entry without following symlinks or reading file contents.
    fn collect(&self, request: LocalMetadataRequest) -> LocalDetectorResult<LocalMetadataResult>;
}
