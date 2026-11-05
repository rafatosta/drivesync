//! Account authorization and folder configuration engine.
//!
//! Initial download is explicit and separate from ongoing synchronization.
mod accounts;
mod activity_history;
mod auth;
mod configuration;
mod download;
mod error;
mod facade;
mod local_detector;
mod local_state;
mod models;
mod remote;
mod remote_detector;

pub use activity_history::{ActivityHistory, ActivityPage, ActivityRecord};
pub use auth::{AuthError, Cancellation, VaultFailure, VaultOperation};
pub use configuration::{DriveFolder, FolderConfiguration};
pub use download::{DownloadProgress, InitialDownload};
pub use error::EngineError;
pub use facade::DriveSyncEngine;
pub use local_detector::{
    LocalActivity, LocalBatch, LocalCoverageGap, LocalItemKind, LocalNotice, LocalScope,
    LocalWatchError, LocalWatchOptions, LocalWatchState, LocalWatcher,
};
pub use models::{AccountId, AccountSummary, RestoredAccount};
pub use remote::FolderPage;
pub use remote_detector::{
    DetectionCheck, DetectionCorpus, DetectionError, DetectionFailureKind, RemoteDelivery,
    RemoteEvent, RemoteEventKind, RemoteFileCapabilities, RemoteFileMetadata, RemoteMembership,
};

pub use local_state::{
    InvestigationReason, ItemId, ItemSnapshot, ItemState, LocalConnectivity, LocalStateCheckpoint,
    LocalStateError, LocalStateStore, PendingInvestigation, TrackedItem, WorkPriority,
};
