//! Private SQLite storage for Inventory snapshots.

mod connection;
mod lookup;
mod write;

use super::models::{
    InventoryError, InventoryLookup, InventoryLookupResult, InventoryWrite, InventoryWriteResult,
};
use crate::models::AccountId;
use rusqlite::Connection;
use std::path::Path;

use super::models;

/// Account-bound database connection. Consumers receive only typed Inventory methods.
pub(super) struct Database {
    pub(super) account_id: AccountId,
    pub(super) connection: Connection,
}

impl Database {
    /// Opens the hashed account database inside a private directory.
    pub(super) fn open(directory: &Path, account_id: &AccountId) -> Result<Self, InventoryError> {
        if !directory.is_absolute() || account_id.0.trim().is_empty() {
            return Err(InventoryError::InvalidConfiguration);
        }
        connection::ensure_private_directory(directory)?;
        let path = connection::database_path(directory, account_id);
        let db = Self::from_connection(connection::open_connection(&path)?, account_id.clone())?;
        Ok(db)
    }

    fn from_connection(
        connection: Connection,
        account_id: AccountId,
    ) -> Result<Self, InventoryError> {
        connection::initialize(&connection, &account_id)?;
        Ok(Self {
            account_id,
            connection,
        })
    }

    /// Reads one snapshot without treating SQL failures as confirmed absence.
    pub(super) fn lookup(&self, request: InventoryLookup) -> InventoryLookupResult {
        lookup::lookup(self, request)
    }

    /// Applies one write in an immediate SQLite transaction.
    pub(super) fn write(&mut self, request: InventoryWrite) -> InventoryWriteResult {
        write::write(self, request)
    }
}

#[cfg(test)]
mod tests;
