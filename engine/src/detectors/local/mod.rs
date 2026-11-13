//! Root-bound local metadata collection, independent of event delivery and watchers.

mod collector;
mod filesystem;
mod metadata;
mod models;

pub use collector::FilesystemMetadataCollector;
pub use models::{
    LocalCancellation, LocalDetectorError, LocalDetectorFailure, LocalDetectorResult,
    LocalDetectorScope, LocalMetadataRequest, LocalMetadataResult, LocalScanCoverage,
    LocalScanResult,
};

#[cfg(test)]
mod tests;
