//! Account-isolated persistence for synchronization baselines and operation state.
//!
//! The Inventory stores facts and transaction revisions only. It does not collect observations,
//! analyze synchronization, or execute filesystem or provider operations.

mod database;
pub(crate) mod models;

use database::Database;
use models::{
    InventoryError, InventoryLookup, InventoryLookupResult, InventoryWrite, InventoryWriteResult,
    WriteStatus,
};
use std::{
    path::PathBuf,
    sync::{Mutex, MutexGuard},
};

/// Private storage location and account scope for one Inventory instance.
pub(crate) struct InventoryConfig {
    /// Dedicated private directory in which per-account databases are stored.
    pub directory: PathBuf,
    /// Account whose synchronization records this instance may access.
    pub account_id: crate::models::AccountId,
}

/// Typed persistence facade for one account's synchronization state.
///
/// A database is opened or created beneath the configured directory. Its filename is derived
/// from the opaque account identifier; every SQL operation also filters by account and sync root.
pub(crate) struct Inventory {
    account_id: crate::models::AccountId,
    database: Mutex<Database>,
}

impl Inventory {
    /// Opens this account's private database without reading or modifying account credentials.
    pub(crate) fn open(config: InventoryConfig) -> Result<Self, InventoryError> {
        let database = Database::open(&config.directory, &config.account_id)?;
        Ok(Self {
            account_id: config.account_id,
            database: Mutex::new(database),
        })
    }

    /// Returns a conclusive result only when the database and scope were readable.
    pub(crate) fn lookup(&self, request: InventoryLookup) -> InventoryLookupResult {
        if request.account_id != self.account_id {
            return InventoryLookupResult::unavailable(
                "Requested account is outside this Inventory scope.",
            );
        }
        match self.database() {
            Ok(database) => database.lookup(request),
            Err(()) => InventoryLookupResult::unavailable("Inventory is unavailable."),
        }
    }

    /// Reads all facts in one account/root, failing rather than returning a partial view.
    pub(crate) fn enumerate(
        &self,
        query: models::InventoryScopeQuery,
    ) -> Result<Vec<models::InventorySnapshot>, InventoryError> {
        if query.account_id != self.account_id {
            return Err(InventoryError::InvalidConfiguration);
        }
        self.database()
            .map_err(|_| InventoryError::Unavailable)?
            .enumerate(query)
    }

    /// Atomically records a typed operation effect with optional optimistic revision checking.
    pub(crate) fn write(&self, request: InventoryWrite) -> InventoryWriteResult {
        if request.identity.account_id != self.account_id {
            return InventoryWriteResult::new(WriteStatus::Rejected, None, request.recorded_at);
        }
        match self.database() {
            Ok(mut database) => database.write(request),
            Err(()) => {
                InventoryWriteResult::new(WriteStatus::Unavailable, None, request.recorded_at)
            }
        }
    }

    fn database(&self) -> Result<MutexGuard<'_, Database>, ()> {
        self.database.lock().map_err(|_| ())
    }
}
