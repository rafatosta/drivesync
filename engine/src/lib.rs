//! Account authorization and folder configuration engine.
//!
//! The public facade performs no file synchronization or conflict resolution.
//! Previous synchronization contracts and models remain in the source tree as
//! references and are not linked into this engine.
mod accounts;
mod auth;
mod configuration;
mod contracts;
mod error;
mod facade;
mod models;
mod remote;
mod root_status;
mod state;

pub use auth::{AuthError, Cancellation, VaultFailure, VaultOperation};
pub use configuration::{DriveFolder, FolderConfiguration};
pub use contracts::{BulkImpact, BulkPolicy, BulkReview};
pub use error::EngineError;
pub use facade::DriveSyncEngine;
pub use models::{AccountId, AccountSummary, EngineStatus, RestoredAccount, SyncRootId};
pub use remote::FolderPage;
pub use root_status::RootSuspension;
