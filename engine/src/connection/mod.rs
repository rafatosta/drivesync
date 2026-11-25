//! Application account connection lifecycle, independent of GTK.
mod conflicts;
mod models;
mod remote_folders;
mod scope;
mod sign_in;
mod synchronization;
use crate::{accounts::*, providers::google::*};
pub use models::*;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::runtime::{Builder, Runtime};

/// Runtime shutdown is safe even when the last Engine is dropped on an async task.
struct Executor(Option<Runtime>);
impl Drop for Executor {
    fn drop(&mut self) {
        if let Some(runtime) = self.0.take() {
            runtime.shutdown_background();
        }
    }
}
struct Inner {
    executor: Executor,
    accounts: Accounts,
    client: GoogleDriveClient,
    oauth: Option<GoogleOAuthConfig>,
    ownership: Arc<ConnectionOwnership>,
    attempts: Mutex<HashMap<GoogleSignInId, Arc<sign_in::Attempt>>>,
    synchronization_directory: std::path::PathBuf,
    synchronizations: tokio::sync::Mutex<HashMap<crate::AccountId, synchronization::Session>>,
    #[cfg(test)]
    sources_enabled: bool,
}
/// Unidentified sign-ins may resolve to any saved account. Other work is account-scoped.
#[derive(Default)]
struct ConnectionOwnership {
    unidentified: Arc<tokio::sync::RwLock<()>>,
    accounts: Mutex<HashMap<AccountId, Arc<AccountOwnership>>>,
}
#[derive(Default)]
struct AccountOwnership {
    admission: tokio::sync::Mutex<()>,
    gate: Arc<tokio::sync::RwLock<()>>,
    cancel_queries: tokio::sync::watch::Sender<()>,
}
impl ConnectionOwnership {
    fn account(&self, id: &AccountId) -> Arc<AccountOwnership> {
        self.accounts
            .lock()
            .unwrap()
            .entry(id.clone())
            .or_default()
            .clone()
    }
}
struct ConnectionRemoval {
    _unidentified: tokio::sync::OwnedRwLockWriteGuard<()>,
    _account: tokio::sync::OwnedRwLockWriteGuard<()>,
}
impl RemovalGuard for ConnectionRemoval {}
impl AccountAssociations for ConnectionOwnership {
    fn lock_for_removal(&self, account: &AccountId) -> AccountsResult<Box<dyn RemovalGuard>> {
        let in_use = |_| AccountsFailure::new(AccountsError::AccountInUse);
        Ok(Box::new(ConnectionRemoval {
            _unidentified: self
                .unidentified
                .clone()
                .try_write_owned()
                .map_err(in_use)?,
            _account: self
                .account(account)
                .gate
                .clone()
                .try_write_owned()
                .map_err(in_use)?,
        }))
    }
}
/// Shared application entry point. All public async requests run on a private
/// executor and may be awaited from GLib without entering a Tokio runtime.
#[derive(Clone)]
pub struct Engine {
    inner: Arc<Inner>,
}
impl Engine {
    /// Opens private account metadata without initiating consent or keyring I/O.
    /// Missing/invalid OAuth setup only disables sign-in; it never resets accounts.
    pub async fn open(config: EngineConfig) -> EngineResult<Self> {
        Self::open_with_client(config, None).await
    }
    async fn open_with_client(
        config: EngineConfig,
        client: Option<GoogleDriveClient>,
    ) -> EngineResult<Self> {
        if !config.data_directory.is_absolute()
            || config.application_id.trim().is_empty()
            || config
                .google_oauth_file
                .as_ref()
                .is_some_and(|p| !p.is_absolute())
        {
            return Err(EngineError::new(EngineErrorKind::InvalidConfiguration));
        }
        let synchronization_directory = synchronization::directory(&config.data_directory);
        let executor = Executor(Some(
            Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .map_err(|_| EngineError::new(EngineErrorKind::Internal))?,
        ));
        let initialized = executor
            .0
            .as_ref()
            .unwrap()
            .spawn(async move {
                let client = client.unwrap_or_default();
                let oauth = if let Some(path) = config.google_oauth_file {
                    match client
                        .load_google_config(GoogleConfigSources {
                            oauth_file_env: Some(path.to_string_lossy().into_owned()),
                            ..Default::default()
                        })
                        .await
                    {
                        Ok(mut config) => {
                            config.scopes = vec![
                                "openid".into(),
                                "email".into(),
                                "profile".into(),
                                "https://www.googleapis.com/auth/drive".into(),
                            ];
                            Some(config)
                        }
                        Err(_) => None,
                    }
                } else {
                    None
                };
                let ownership = Arc::new(ConnectionOwnership::default());
                let setup = AccountsConfig::new(
                    config.data_directory,
                    config.application_id,
                    ownership.clone(),
                )
                .with_google(client.clone(), oauth.clone().unwrap_or_default());
                let accounts = tokio::task::spawn_blocking(move || Accounts::new(setup))
                    .await
                    .map_err(|_| EngineError::new(EngineErrorKind::Internal))??;
                Ok::<_, EngineError>((accounts, client, oauth, ownership))
            })
            .await
            .map_err(|_| EngineError::new(EngineErrorKind::Internal))??;
        Ok(Self {
            inner: Arc::new(Inner {
                executor,
                accounts: initialized.0,
                client: initialized.1,
                oauth: initialized.2,
                ownership: initialized.3,
                attempts: Mutex::new(HashMap::new()),
                synchronization_directory,
                synchronizations: tokio::sync::Mutex::new(HashMap::new()),
                #[cfg(test)]
                sources_enabled: true,
            }),
        })
    }
    fn runtime(&self) -> &Runtime {
        self.inner
            .executor
            .0
            .as_ref()
            .expect("executor is live while Engine is owned")
    }
    /// Lists metadata locally; no Google session, keyring or network is consulted.
    pub async fn list_accounts(
        &self,
        request: ListAccountsRequest,
    ) -> EngineResult<ListAccountsResult> {
        let accounts = self.inner.accounts.clone();
        self.runtime()
            .spawn_blocking(move || accounts.list(request).map_err(EngineError::from))
            .await
            .map_err(|_| EngineError::new(EngineErrorKind::Internal))?
    }
    /// Disconnects one account by deleting its local credentials and metadata.
    /// No local/remote files are deleted and Google authorization is not revoked.
    /// Stops synchronization and cancels folder browsing and pending reauthorization.
    /// Waits for in-flight mutations and credential writes before removing the account.
    /// An unidentified sign-in can still exclude removal until its identity is resolved.
    /// Dropping the future does not roll back a durable removal already started.
    pub async fn disconnect_account(
        &self,
        request: RemoveAccountRequest,
    ) -> EngineResult<RemoveAccountResult> {
        let inner = self.inner.clone();
        self.runtime()
            .spawn(async move {
                // Serialize removal against scope replacement, and release operation leases first.
                let mut sessions = inner.synchronizations.lock().await;
                let account_id = request.account_id.clone();
                let ownership = inner.ownership.account(&account_id);
                let _admission = ownership.admission.lock().await;
                ownership.cancel_queries.send_replace(());
                inner.cancel_reauthorization(&account_id);
                if let Some(session) = sessions.get(&account_id) {
                    session.stop().await;
                }
                // The sessions and admission locks prevent new account work from
                // slipping in between draining and durable removal.
                drop(ownership.gate.write().await);
                let result = inner
                    .accounts
                    .remove(request)
                    .await
                    .map_err(EngineError::from);
                if result.is_ok() {
                    sessions.remove(&account_id);
                }
                result
            })
            .await
            .map_err(|_| EngineError::new(EngineErrorKind::Internal))?
    }
    /// Persists only the local display name. None clears it; failure preserves
    /// the previously saved value and is returned to the caller for draft handling.
    pub async fn update_account(
        &self,
        request: UpdateAccountRequest,
    ) -> EngineResult<UpdateAccountResult> {
        let accounts = self.inner.accounts.clone();
        self.runtime()
            .spawn_blocking(move || accounts.update(request).map_err(EngineError::from))
            .await
            .map_err(|_| EngineError::new(EngineErrorKind::Internal))?
    }
}
#[cfg(test)]
mod tests;

#[cfg(test)]
mod live_initial_download_tests;

#[cfg(test)]
mod live_initial_upload_tests;

#[cfg(test)]
mod live_continuous_tests;

#[cfg(test)]
mod live_tree_tests;

#[cfg(test)]
mod live_conflict_tests;

#[cfg(test)]
mod live_projection_tests;

#[cfg(test)]
mod live_trash_tests;

#[cfg(test)]
mod live_acceptance_tests;

#[cfg(test)]
mod live_stage12_tests;
