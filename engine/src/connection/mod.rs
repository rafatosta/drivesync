//! Application connection lifecycle, independent of GTK and synchronization.
mod models;
mod sign_in;
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
}
/// This connection-only engine owns no synchronization associations. Every
/// authorization attempt holds a read lease; local removal requires exclusion
/// against all attempts, including additions whose identity is not yet known.
#[derive(Default)]
struct ConnectionOwnership {
    gate: Arc<tokio::sync::RwLock<()>>,
}
struct ConnectionRemoval {
    _lease: tokio::sync::OwnedRwLockWriteGuard<()>,
}
impl RemovalGuard for ConnectionRemoval {}
impl AccountAssociations for ConnectionOwnership {
    fn lock_for_removal(&self, _: &AccountId) -> AccountsResult<Box<dyn RemovalGuard>> {
        let lease = self
            .gate
            .clone()
            .try_write_owned()
            .map_err(|_| AccountsFailure::new(AccountsError::AccountInUse))?;
        Ok(Box::new(ConnectionRemoval { _lease: lease }))
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
                            config.scopes = vec!["openid".into(), "email".into(), "profile".into()];
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
    /// Active authorization attempts reject removal with AccountInUse, preventing
    /// a late callback from republishing credentials after successful removal.
    /// Dropping the future does not roll back a durable removal already started.
    pub async fn disconnect_account(
        &self,
        request: RemoveAccountRequest,
    ) -> EngineResult<RemoveAccountResult> {
        let accounts = self.inner.accounts.clone();
        self.runtime()
            .spawn(async move { accounts.remove(request).await.map_err(EngineError::from) })
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
