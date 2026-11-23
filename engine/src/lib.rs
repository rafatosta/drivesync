//! DriveSync engine library for account connections and provider access.

pub mod accounts;
pub mod conflicts;
#[allow(dead_code)] // Some diagnostic and recovery contracts are reserved for later policies.
mod detectors;
#[allow(dead_code)] // Some recovery and conflict writes are reserved for later lifecycle policies.
mod inventory;
pub mod models;
#[allow(dead_code)] // Some operation and recovery policies are still deliberately unsupported.
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
