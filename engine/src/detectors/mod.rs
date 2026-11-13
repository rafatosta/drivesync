//! Local and remote observation with durable, confirmable event queues.
//!
//! PersistentDetector and RemoteDetector provide scoped durable queues.
//! The unified detector facade remains future work. Collector construction and collection
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

pub use local::{
    LocalActivity, LocalDetector, LocalDetectorCreateRequest, LocalDetectorCreateResult,
    LocalDetectorSettings, LocalDetectorState, LocalNotificationSink, LocalStartMode,
    LocalStartRequest, LocalStartResult, LocalStatusRequest, LocalStatusResult, LocalStopReason,
    LocalStopRequest, LocalStopResult,
};

mod storage;
pub use storage::{
    BatchId, DeliveryState, EventDisposition, FileBatch, QueueError, QueueFailure, QueueResult,
    QueueStats, TakeEventsRequest,
};
mod persistent;
pub use persistent::{PersistentDetector, PersistentDetectorRequest};

pub(crate) mod remote;
pub use remote::{
    RemoteActivity, RemoteCoverage, RemoteDetector, RemoteDetectorCreateRequest,
    RemoteDetectorScope, RemoteDetectorSettings, RemoteDiagnostics, RemoteError,
    RemoteEventSummary, RemoteFailure, RemotePollMode, RemotePollRequest, RemotePollResult,
    RemoteResult, RemoteStartRequest, RemoteStartResult, RemoteState, RemoteStatusRequest,
    RemoteStatusResult, RemoteStopRequest, RemoteStopResult,
};
