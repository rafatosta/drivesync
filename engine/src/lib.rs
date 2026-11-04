//! Account authorization and folder configuration engine.
//!
//! The public facade performs no file synchronization or conflict resolution.
mod accounts;
mod auth;
mod configuration;
mod error;
mod facade;
mod models;
mod remote;

pub use auth::{AuthError, Cancellation, VaultFailure, VaultOperation};
pub use configuration::{DriveFolder, FolderConfiguration};
pub use error::EngineError;
pub use facade::DriveSyncEngine;
pub use models::{AccountId, AccountSummary, RestoredAccount};
pub use remote::FolderPage;
