//! Account identity, local metadata and private credential ownership.
mod models;
mod storage;
pub use models::*;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use storage::Storage;
/// A lease supplied by the external connection owner. Drop releases exclusion.
pub trait RemovalGuard: Send {}
/// The owner must atomically reject existing links and prevent new links until
/// the returned guard is dropped. Unknown association state must fail closed.
pub trait AccountAssociations: Send + Sync {
    fn lock_for_removal(&self, account: &AccountId) -> AccountsResult<Box<dyn RemovalGuard>>;
}
/// Explicit account storage and association configuration.
pub struct AccountsConfig {
    directory: PathBuf,
    application: String,
    associations: Arc<dyn AccountAssociations>,
}
impl AccountsConfig {
    /// Selects a private absolute directory and application-scoped secure storage.
    pub fn new(
        directory: PathBuf,
        application_id: impl Into<String>,
        associations: Arc<dyn AccountAssociations>,
    ) -> Self {
        Self {
            directory,
            application: application_id.into(),
            associations,
        }
    }
}
/// Local account service. Clones share the same store and operational state.
#[derive(Clone)]
pub struct Accounts {
    store: Arc<Mutex<Storage>>,
    _associations: Arc<dyn AccountAssociations>,
}
impl Accounts {
    /// Opens metadata exclusively. Performs no OAuth, network or keyring I/O.
    /// Call outside the GTK main loop because opening metadata accesses disk.
    pub fn new(config: AccountsConfig) -> AccountsResult<Self> {
        Ok(Self {
            store: Arc::new(Mutex::new(Storage::open(
                config.directory,
                config.application,
            )?)),
            _associations: config.associations,
        })
    }
    /// Reads a local snapshot. No authentication or secure-storage access.
    pub fn get(&self, request: GetAccountRequest) -> AccountsResult<GetAccountResult> {
        let store = self.store.lock().map_err(|_| storage::storage_error())?;
        let account = store
            .data
            .records
            .get(&request.account_id)
            .ok_or_else(|| AccountsFailure::new(AccountsError::NotFound))?
            .account
            .clone();
        Ok(GetAccountResult { account })
    }
    /// Lists local metadata, optionally filtering by service, without network I/O.
    pub fn list(&self, request: ListAccountsRequest) -> AccountsResult<ListAccountsResult> {
        let store = self.store.lock().map_err(|_| storage::storage_error())?;
        Ok(ListAccountsResult {
            accounts: store
                .data
                .records
                .values()
                .filter(|r| {
                    request
                        .provider
                        .as_ref()
                        .is_none_or(|p| p == &r.account.provider)
                })
                .map(|r| r.account.clone())
                .collect(),
        })
    }
    /// Persists a presentation-only edit. None clears the display name.
    pub fn update(&self, request: UpdateAccountRequest) -> AccountsResult<UpdateAccountResult> {
        if request
            .display_name
            .as_ref()
            .is_some_and(|n| n.trim().is_empty())
        {
            return Err(AccountsFailure::new(AccountsError::InvalidRequest));
        }
        let mut store = self.store.lock().map_err(|_| storage::storage_error())?;
        if store.data.intent.is_some() {
            return Err(storage::storage_error());
        }
        let mut data = store.data.clone();
        let record = data
            .records
            .get_mut(&request.account_id)
            .ok_or_else(|| AccountsFailure::new(AccountsError::NotFound))?;
        record.account.display_name = request.display_name;
        record.account.updated_at = now()?;
        let account = record.account.clone();
        store.save(data)?;
        Ok(UpdateAccountResult { account })
    }
    /// Returns current in-memory activity without accessing any dependency.
    pub fn status(&self, _: AccountsStatusRequest) -> AccountsStatusResult {
        AccountsStatusResult {
            active_operations: vec![],
        }
    }
}
fn now() -> AccountsResult<Timestamp> {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| AccountsFailure::new(AccountsError::Internal))?
        .as_millis();
    Ok(Timestamp {
        unix_millis: i64::try_from(millis)
            .map_err(|_| AccountsFailure::new(AccountsError::Internal))?,
    })
}
