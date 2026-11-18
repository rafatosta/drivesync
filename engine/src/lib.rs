//! DriveSync engine library.

pub mod providers;

pub mod accounts;
pub mod models;
pub use models::{AccountId, ProviderId, Timestamp};

mod synchronization;
pub use synchronization::{
    SynchronizationAction, SynchronizationActivity, SynchronizationCounters, SynchronizationEvent,
    SynchronizationIssue, SynchronizationIssueKind, SynchronizationItem,
    SynchronizationLocalSource, SynchronizationLocation, SynchronizationProgress,
    SynchronizationProgressUnit, SynchronizationRemoteSource, SynchronizationSources,
    SynchronizationState, SynchronizationStatus, SynchronizationSubscription,
    SynchronizationWaitReason, SynchronizationWork, SynchronizationWorkDetail,
};

mod connection;
pub use accounts::{
    Account, AccountState, AccountsError, ListAccountsRequest, ListAccountsResult,
    RemoveAccountRequest, RemoveAccountResult, UpdateAccountRequest, UpdateAccountResult,
};
pub use connection::*;

pub mod detectors;

pub mod orchestrator;
pub use orchestrator::Orchestrator;

pub mod inventory;

mod filesystem;

pub mod scanner;

pub mod analyzer;

pub mod operations;
