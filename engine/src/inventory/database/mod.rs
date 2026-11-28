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

    /// Enumerates a consistent scoped view, including unseen and pending items.
    #[cfg(test)]
    pub(super) fn enumerate(
        &self,
        query: models::InventoryScopeQuery,
    ) -> Result<Vec<models::InventorySnapshot>, models::InventoryError> {
        self.enumerate_observed(query, None)
    }

    pub(super) fn enumerate_observed(
        &self,
        query: models::InventoryScopeQuery,
        mut activity: Option<&mut super::activity::Guard>,
    ) -> Result<Vec<models::InventorySnapshot>, models::InventoryError> {
        if query.account_id != self.account_id || query.sync_root_id.0.trim().is_empty() {
            return Err(models::InventoryError::InvalidConfiguration);
        }
        let mut statement = self.connection.prepare(
            "SELECT snapshot_json, revision, item_id FROM inventory_items WHERE account_id = ?1 AND sync_root_id = ?2 ORDER BY item_id"
        ).map_err(|_| models::InventoryError::Unavailable)?;
        let rows = statement
            .query_map(
                rusqlite::params![query.account_id.0, query.sync_root_id.0],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .map_err(|_| models::InventoryError::Unavailable)?;
        let mut result = Vec::new();
        let mut processed = 0u64;
        for row in rows {
            let (json, revision, item_id) = row.map_err(|_| models::InventoryError::Unavailable)?;
            let mut snapshot: models::InventorySnapshot =
                serde_json::from_str(&json).map_err(|_| models::InventoryError::Unavailable)?;
            if snapshot.identity.account_id != query.account_id
                || snapshot.identity.sync_root_id != query.sync_root_id
                || snapshot.identity.item_id.as_ref().map(|id| &id.0) != Some(&item_id)
                || revision <= 0
            {
                return Err(models::InventoryError::Unavailable);
            }
            snapshot.revision = revision as u64;
            result.push(snapshot);
            processed = processed.saturating_add(1);
            if let Some(activity) = activity.as_deref_mut() {
                activity.progress(processed, None);
            }
        }
        Ok(result)
    }

    /// Applies one write in an immediate SQLite transaction.
    #[cfg(test)]
    pub(super) fn write(&mut self, request: InventoryWrite) -> InventoryWriteResult {
        write::write(self, request)
    }

    pub(super) fn write_observed(
        &mut self,
        request: InventoryWrite,
        activity: Option<&mut super::activity::Guard>,
    ) -> (InventoryWriteResult, bool) {
        let mut rolled_back = false;
        let result = write::write_observed(self, request, activity, &mut rolled_back);
        (result, rolled_back)
    }
}

#[cfg(test)]
mod tests;
