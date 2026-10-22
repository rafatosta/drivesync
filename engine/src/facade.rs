//! DriveSync engine facade implementation.
//!
//! Coordinates independent account, authentication, Drive and local folder
//! modules without exposing credentials or platform storage to the GUI.
use crate::{
    accounts, auth, configuration, remote, AccountId, AccountSummary, AuthError,
    Cancellation, EngineError, EngineStatus, FolderConfiguration, FolderPage,
    RestoredAccount, SyncRootId,
};

/// Public boundary for account authorization, restoration and folder setup.
///
/// Blocking calls must be dispatched off the GTK main thread. Internal
/// authentication and storage adapters remain hidden behind this facade.
#[derive(Default)]
pub struct DriveSyncEngine {
    auth: auth::AuthProvider,
    accounts: accounts::Accounts,
}

impl DriveSyncEngine {
    /// Constructs the engine using the platform's default adapters.
    pub fn new() -> Self {
        Self::default()
    }
    /// Returns the current coarse engine status.
    ///
    /// Synchronization is not yet implemented in this MVP.
    pub fn status(&self) -> EngineStatus {
        EngineStatus::NotConfigured
    }
    /// Blocking operation: call from a worker, never the GTK main thread.
    /// The browser port must report launch failure; it receives no credentials.
    pub fn add_account(
        &self,
        cancel: &Cancellation,
        open_browser: impl FnOnce(&str) -> Result<(), AuthError>,
    ) -> Result<AccountSummary, EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        let account = self
            .auth
            .authorize(&config, cancel, open_browser)
            .map_err(EngineError::Authentication)?;
        self.accounts
            .record(&config.client_id, account.clone())
            .map_err(EngineError::Authentication)?;
        Ok(account)
    }
    /// Blocking disk read of saved profiles, not a verification of authentication.
    pub fn list_accounts(&self) -> Result<Vec<AccountSummary>, EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        self.accounts
            .list(&config.client_id)
            .map_err(EngineError::Authentication)
    }
    /// Reuses persisted credentials, renewing if necessary, without exposing them.
    pub fn verify_account(&self, id: &AccountId) -> Result<AccountSummary, EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        let account = self
            .auth
            .verify_with_config(&config, id)
            .map_err(EngineError::Authentication)?;
        self.accounts
            .record(&config.client_id, account.clone())
            .map_err(EngineError::Authentication)?;
        Ok(account)
    }
    /// Blocking startup operation. Cached profiles never imply authentication.
    pub fn restore_accounts(&self) -> Result<Vec<RestoredAccount>, EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        // Migrate profiles left by older local logout: prune only a confirmed
        // missing vault entry. Offline, revoked access and locked vault are not absence.
        for profile in self
            .accounts
            .list(&config.client_id)
            .map_err(EngineError::Authentication)?
        {
            self.auth
                .prune_if_missing(&config, &profile.id, || {
                    self.accounts.forget(&config.client_id, &profile.id)
                })
                .map_err(EngineError::Authentication)?;
        }
        self.restore_with(&config.client_id, |id| {
            self.auth.verify_with_config(&config, id)
        })
    }
    fn restore_with(
        &self,
        client: &str,
        mut verify: impl FnMut(&AccountId) -> Result<AccountSummary, AuthError>,
    ) -> Result<Vec<RestoredAccount>, EngineError> {
        let profiles = self
            .accounts
            .list(client)
            .map_err(EngineError::Authentication)?;
        let folders = self
            .accounts
            .folders(client)
            .map_err(EngineError::Authentication)?;
        Ok(profiles
            .into_iter()
            .map(|cached| match verify(&cached.id) {
                Ok(account) if account.id == cached.id => {
                    let error = self.accounts.record(client, account.clone()).err();
                    RestoredAccount {
                        folder: folders.iter().find(|f| f.account_id == account.id).cloned(),
                        account,
                        error,
                    }
                }
                Ok(_) => RestoredAccount {
                    folder: folders.iter().find(|f| f.account_id == cached.id).cloned(),
                    account: cached,
                    error: Some(AuthError::IdentityMismatch),
                },
                Err(error) => RestoredAccount {
                    folder: folders.iter().find(|f| f.account_id == cached.id).cloned(),
                    account: cached,
                    error: Some(error),
                },
            })
            .collect())
    }
    /// Blocking metadata lookup; tokens stay inside auth.
    pub fn drive_folders(
        &self,
        id: &AccountId,
        parent: &str,
        page: Option<&str>,
    ) -> Result<FolderPage, EngineError> {
        self.auth
            .with_access(id, |token| remote::list(token, parent, page))
    }
    /// Returns the saved folder pair for an account, without remote validation.
    ///
    /// # Errors
    ///
    /// Returns an [`EngineError`] if OAuth configuration or metadata storage
    /// cannot be read.
    pub fn saved_folder(&self, id: &AccountId) -> Result<Option<FolderConfiguration>, EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        Ok(self
            .accounts
            .folders(&config.client_id)
            .map_err(EngineError::Authentication)?
            .into_iter()
            .find(|f| &f.account_id == id))
    }
    /// Configures a remote Drive folder with a writable local directory.
    ///
    /// Verifies remote folder metadata, resolves local/portal paths and rejects
    /// overlap with other accounts' configured roots.
    ///
    /// # Errors
    ///
    /// Returns an [`EngineError`] for provider access, an invalid directory,
    /// unidentified portal origin, root overlap or persistence failure.
    pub fn configure_folder(
        &self,
        id: &AccountId,
        remote_id: &str,
        local: &std::path::Path,
    ) -> Result<FolderConfiguration, EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        let remote = self
            .auth
            .with_access_config(&config, id, |token| remote::get(token, remote_id))?;
        let local = configuration::local_folder(local)?;
        let local_display = configuration::local_origin(&local)?;
        let folder = FolderConfiguration {
            account_id: id.clone(),
            remote,
            local,
            local_display,
        };
        self.accounts.configure(&config.client_id, folder.clone())?;
        Ok(folder)
    }
    /// Blocking local logout: forgets credentials and app settings, never user files.
    pub fn sign_out(&self, id: &AccountId) -> Result<(), EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        self.auth
            .sign_out_then(&config, id, || self.accounts.forget(&config.client_id, id))
            .map_err(EngineError::Authentication)
    }
    /// Reserved sync entrypoint. File synchronization is not implemented.
    ///
    /// # Errors
    ///
    /// Always returns [`EngineError::NotImplemented`] in the current MVP.
    pub fn start_sync(&self, _root: SyncRootId) -> Result<(), EngineError> {
        Err(EngineError::NotImplemented("start_sync"))
    }
}
#[cfg(test)]
mod tests;
#[cfg(test)]
mod live_google_tests;
