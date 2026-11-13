//! Public access to the implemented local metadata collector.
//!
//! This namespace does not yet provide the unified detector, event queue,
//! watcher lifecycle or remote observation API. Construction and collection
//! perform synchronous filesystem I/O and should run off the UI thread.
//!
//! ```
//! use drivesync_engine::detectors::{
//!     LocalCancellation, LocalMetadataCollector, LocalMetadataRequest,
//! };
//! use drivesync_engine::models::LocalRelativePath;
//!
//! fn request() -> Result<LocalMetadataRequest, drivesync_engine::models::ModelError> {
//!     Ok(LocalMetadataRequest {
//!         relative_path: LocalRelativePath::new(b"folder/file".to_vec())?,
//!         cancellation: LocalCancellation::new(),
//!     })
//! }
//! fn assert_send_sync<T: Send + Sync>() {}
//! assert_send_sync::<LocalMetadataCollector>();
//! let token = request()?.cancellation;
//! token.cancel();
//! assert!(token.is_cancelled());
//! # Ok::<(), drivesync_engine::models::ModelError>(())
//! ```
//!
//! Implementation modules are not an additional public route:
//! ```compile_fail,E0603
//! use drivesync_engine::detectors::local::collector::FilesystemMetadataCollector;
//! ```
//!
//! Syscall injection and race hooks remain private even on the exported collector:
//! ```compile_fail,E0624
//! use drivesync_engine::detectors::{LocalMetadataCollector, LocalMetadataRequest};
//! fn inject(collector: LocalMetadataCollector, request: LocalMetadataRequest) {
//!     collector.collect_with(request, |_| {});
//! }
//! ```

mod local;

pub use local::{
    FilesystemMetadataCollector as LocalMetadataCollector, LocalCancellation, LocalDetectorError,
    LocalDetectorFailure, LocalDetectorResult, LocalDetectorScope, LocalMetadataRequest,
    LocalMetadataResult, LocalScanCoverage, LocalScanResult,
};
