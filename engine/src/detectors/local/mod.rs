//! Root-bound metadata collection and native local event production.

mod changes;
mod collector;
mod filesystem;
mod metadata;
mod models;
mod runtime;
mod watcher;

pub use collector::FilesystemMetadataCollector;
pub use models::{
    LocalCancellation, LocalDetectorError, LocalDetectorFailure, LocalDetectorResult,
    LocalDetectorScope, LocalMetadataRequest, LocalMetadataResult, LocalScanCoverage,
    LocalScanResult,
};

#[cfg(test)]
mod tests;

pub use models::{
    LocalActivity, LocalDetectorCreateRequest, LocalDetectorCreateResult, LocalDetectorSettings,
    LocalDetectorState, LocalNotificationSink, LocalStartMode, LocalStartRequest, LocalStartResult,
    LocalStatusRequest, LocalStatusResult, LocalStopReason, LocalStopRequest, LocalStopResult,
};
pub use runtime::LocalDetector;
