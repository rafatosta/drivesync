//! Independent DriveSync engine library.
//!
//! GTK and presentation dependencies are confined to the application crate.
//! All consumers interact with [`DriveSyncEngine`] and the types reexported
//! here; internal modules are not exposed as independent public APIs.
//! Manual binary downloads consume durable incremental remote observations,
//! update intact managed copies and retain prior versions.
//! Create-only uploads are optional per folder pair; remote overwrite and deletion are unavailable.
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
pub use changes::{ChangeKind, ReconciliationEntry};
pub use configuration::{DriveFolder, FolderConfiguration};
pub use conflicts::{
    classify_differences, AttributeChange, BaseEvidence, ConflictFailure, ConflictHistoryEvent,
    ConflictHistoryKind, ConflictOutcome, ConflictRecord, ConflictStatus, ContentComparison,
    DifferenceDecision, DifferenceInput, DifferenceKind, DifferenceReason, Disposition, LocalState,
    ObservationFailure, RemoteState, ResolutionObservation, ResolutionProposal, SideDifference,
    StaleResolutionReason,
};
pub use coordinator::RootSuspension;
pub use error::EngineError;
pub use facade::DriveSyncEngine;
pub use models::{AccountId, AccountSummary, EngineStatus, RestoredAccount, SyncRootId};
pub use remote::{FolderPage, InventoryEntry, InventoryKind, RemoteInventory, SkipReason};

pub use planner::{
    decide, BulkImpact, BulkPolicy, BulkReview, ConfirmedBase, ConfirmedLocation, ConflictReason,
    DownloadPlan, LocalObservation, PlanAction, PlanEntry,
};

pub use download::{
    ConflictCopy, DownloadAttention, DownloadIssue, DownloadItemProgress, DownloadProgress,
    DownloadReport, DownloadStage, PreparationProgress, PreviousPath,
};

pub use state::recovery::StateRecovery;
