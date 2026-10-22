//! Independent DriveSync engine library.
//!
//! GTK and presentation dependencies are confined to the application crate.
//! All consumers interact with [`DriveSyncEngine`] and the types reexported
//! here; internal modules are not exposed as independent public APIs.
//! Synchronization and file transfers are not implemented yet.
mod accounts;
mod auth;
mod changes;
mod configuration;
mod conflicts;
mod coordinator;
mod download;
mod error;
mod facade;
mod ipc;
mod local;
mod models;
mod notifications;
mod planner;
mod queue;
mod remote;
mod state;
mod ui;
mod upload;

pub use auth::{AuthError, Cancellation};
pub use configuration::{DriveFolder, FolderConfiguration};
pub use error::EngineError;
pub use facade::DriveSyncEngine;
pub use models::{AccountId, AccountSummary, EngineStatus, RestoredAccount, SyncRootId};
pub use remote::FolderPage;
