//! Independent DriveSync engine library.
//! Domain modules remain private; user interfaces depend on this facade.
//! Sync functionality is intentionally not implemented yet.

mod accounts;
mod auth;
mod changes;
mod configuration;
mod conflicts;
mod coordinator;
mod download;
mod ipc;
mod local;
mod notifications;
mod planner;
mod queue;
mod remote;
mod state;
mod ui;
mod upload;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineStatus {
    NotConfigured,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    NotImplemented(&'static str),
    Authentication(AuthError),
    InvalidFolder,
    LocalFolderIdentity,
    FolderOverlap,
    RemoteFolderUnavailable,
    RemoteForbidden,
    DriveApiDisabled,
    RemoteRateLimited,
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotImplemented(operation) => write!(f, "{operation} is not implemented"),
            Self::Authentication(error) => error.fmt(f),
            Self::LocalFolderIdentity => f.write_str("Could not verify this folder’s location. Check that your desktop portal is up to date and choose the folder again."),
            Self::InvalidFolder => f.write_str("Choose an existing, writable local folder."),
            Self::FolderOverlap => f.write_str("The local folder overlaps another account’s folder. Choose a different location."),
            Self::RemoteFolderUnavailable => f.write_str("This Google Drive folder is unavailable. Choose it again."),
            Self::RemoteForbidden => f.write_str("Google Drive denied access to this folder. Check this account’s access or choose another folder."),
            Self::DriveApiDisabled => f.write_str("Google Drive access is unavailable because Drive API is disabled for this app. Contact the DriveSync distributor."),
            Self::RemoteRateLimited => f.write_str("Google Drive is receiving too many requests. Please try again later."),
        }
    }
}
impl std::error::Error for EngineError {}

pub use auth::{AuthError, Cancellation};
pub use configuration::{DriveFolder, FolderConfiguration};
pub use remote::FolderPage;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct AccountId(pub String);
impl AccountId {
    fn from_subject(subject: &str) -> Self {
        Self(format!("google:{subject}"))
    }
}

/// Sanitized identity. Saved/listed metadata alone does not prove authentication.
/// add_account/verify_account confirm it at the time of the operation; no sync
/// root is implied.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountSummary {
    pub id: AccountId,
    pub provider_subject: String,
    pub email: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SyncRootId(pub u64);

/// A saved identity plus the result of verifying it during this startup.
/// `error == None` is the only authenticated outcome; failures retain the profile.
#[derive(Debug, Clone)]
pub struct RestoredAccount {
    pub account: AccountSummary,
    pub error: Option<AuthError>,
    pub folder: Option<FolderConfiguration>,
}

#[derive(Default)]
pub struct DriveSyncEngine {
    auth: auth::AuthProvider,
    accounts: accounts::Accounts,
}

impl DriveSyncEngine {
    pub fn new() -> Self {
        Self::default()
    }
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
    pub fn saved_folder(&self, id: &AccountId) -> Result<Option<FolderConfiguration>, EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        Ok(self
            .accounts
            .folders(&config.client_id)
            .map_err(EngineError::Authentication)?
            .into_iter()
            .find(|f| &f.account_id == id))
    }
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
    pub fn start_sync(&self, _root: SyncRootId) -> Result<(), EngineError> {
        Err(EngineError::NotImplemented("start_sync"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unimplemented_operations_are_explicit() {
        let engine = DriveSyncEngine::new();
        assert_eq!(engine.status(), EngineStatus::NotConfigured);
        assert!(matches!(
            engine.start_sync(SyncRootId(1)),
            Err(EngineError::NotImplemented(_))
        ));
    }
}

#[cfg(test)]
mod live_google_tests {
    use super::*;
    #[test]
    #[ignore = "explicit opt-in; accesses the real Google account and desktop vault"]
    fn saved_account_can_browse_drive_folders() {
        assert_eq!(
            std::env::var("DRIVESYNC_LIVE_GOOGLE_TEST").as_deref(),
            Ok("1")
        );
        let engine = DriveSyncEngine::new();
        let accounts = engine.list_accounts().expect("saved profile index");
        let id = &accounts.first().expect("at least one saved account").id;
        let page = engine
            .drive_folders(id, "root", None)
            .unwrap_or_else(|e| panic!("Sanitized Drive diagnostic: {e:?}"));
        let config = auth::OAuthConfig::load()
            .map_err(|_| "OAuth configuration")
            .unwrap();
        engine
            .auth
            .with_access_config(&config, id, |token| remote::get(token, "root"))
            .unwrap_or_else(|e| panic!("Sanitized root metadata diagnostic: {e:?}"));
        if let Some(folder) = page.folders.first() {
            engine
                .drive_folders(id, &folder.id, None)
                .unwrap_or_else(|e| panic!("Sanitized child folder diagnostic: {e:?}"));
        }
        println!("Real Google folder listing and metadata retrieval succeeded. No account or folder identifiers printed.");
    }
}
