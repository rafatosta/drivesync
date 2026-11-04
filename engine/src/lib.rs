//! Account authorization and folder configuration engine.
//!
//! The public facade performs no file synchronization or conflict resolution.
mod accounts;
mod auth;
mod configuration;
mod error;
mod facade;
mod local_detector;
mod models;
mod remote;
mod remote_detector;

pub use auth::{AuthError, Cancellation, VaultFailure, VaultOperation};
pub use configuration::{DriveFolder, FolderConfiguration};
pub use error::EngineError;
pub use facade::DriveSyncEngine;
pub use local_detector::{
    reconcile_local_inventories, InvalidLocalInventory, LocalEntry, LocalInventory, LocalMetadata,
    LocalObject, LocalObservation,
};
pub use models::{AccountId, AccountSummary, RestoredAccount};
pub use remote::FolderPage;
pub use remote_detector::{
    DetectionCheck, DetectionCorpus, DetectionError, DetectionFailureKind, RemoteDelivery,
    RemoteEvent, RemoteEventKind, RemoteFileCapabilities, RemoteFileMetadata, RemoteMembership,
};
