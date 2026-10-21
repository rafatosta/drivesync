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
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotImplemented(operation) => write!(f, "{operation} is not implemented"),
            Self::Authentication(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for EngineError {}

pub use auth::{AuthError, Cancellation};

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
        Ok(profiles
            .into_iter()
            .map(|cached| match verify(&cached.id) {
                Ok(account) if account.id == cached.id => {
                    let error = self.accounts.record(client, account.clone()).err();
                    RestoredAccount { account, error }
                }
                Ok(_) => RestoredAccount {
                    account: cached,
                    error: Some(AuthError::IdentityMismatch),
                },
                Err(error) => RestoredAccount {
                    account: cached,
                    error: Some(error),
                },
            })
            .collect())
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
