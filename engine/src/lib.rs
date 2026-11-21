//! DriveSync engine library for account connections and provider access.

pub mod accounts;
#[allow(dead_code)] // Observation contracts are established before producers in later steps.
mod detectors;
pub mod models;
pub mod providers;
pub use models::{AccountId, ProviderId, Timestamp};

mod connection;
pub use accounts::{
    Account, AccountState, AccountsError, ListAccountsRequest, ListAccountsResult,
    RemoveAccountRequest, RemoveAccountResult, UpdateAccountRequest, UpdateAccountResult,
};
pub use connection::*;
