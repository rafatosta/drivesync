//! DriveSync engine library.

pub mod providers;

pub mod accounts;
pub mod models;
pub use models::{AccountId, ProviderId, Timestamp};

mod connection;
pub use accounts::{
    Account, AccountState, AccountsError, ListAccountsRequest, ListAccountsResult,
    UpdateAccountRequest, UpdateAccountResult,
};
pub use connection::*;
