//! DriveSync engine library for account connections and provider access.

pub mod accounts;
#[allow(dead_code)] // Observation contracts are established before producers in later steps.
mod detectors;
#[allow(dead_code)] // Some recovery and conflict writes are reserved for later lifecycle policies.
mod inventory;
pub mod models;
#[allow(dead_code)] // Producer ingress is activated in the next reconstruction step.
mod orchestrator;
pub mod providers;
pub use models::{AccountId, ProviderId, Timestamp};

mod connection;
pub use accounts::{
    Account, AccountState, AccountsError, ListAccountsRequest, ListAccountsResult,
    RemoveAccountRequest, RemoveAccountResult, UpdateAccountRequest, UpdateAccountResult,
};
pub use connection::*;

mod synchronization;
pub use synchronization::*;
