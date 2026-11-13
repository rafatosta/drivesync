//! DriveSync engine library.

pub mod providers;

pub mod accounts;
pub mod models;
pub use models::{AccountId, ProviderId, Timestamp};

mod connection;
pub use accounts::{
    Account, AccountState, AccountsError, ListAccountsRequest, ListAccountsResult,
    RemoveAccountRequest, RemoveAccountResult, UpdateAccountRequest, UpdateAccountResult,
};
pub use connection::*;

pub mod detectors;

pub mod orchestrator;
pub use orchestrator::Orchestrator;
