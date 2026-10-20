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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccountId(pub String);
impl AccountId {
    fn from_subject(subject: &str) -> Self {
        Self(format!("google:{subject}"))
    }
}

/// Sanitized identity. Authentication is confirmed at the time of the operation;
/// this does not claim that a synchronization root is configured or ready.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountSummary {
    pub id: AccountId,
    pub provider_subject: String,
    pub email: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SyncRootId(pub u64);

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
        let account = self
            .auth
            .authorize(cancel, open_browser)
            .map_err(EngineError::Authentication)?;
        self.accounts
            .record(account.clone())
            .map_err(EngineError::Authentication)?;
        Ok(account)
    }
    pub fn list_accounts(&self) -> Result<Vec<AccountSummary>, EngineError> {
        self.accounts.list().map_err(EngineError::Authentication)
    }
    /// Reuses persisted credentials, renewing if necessary, without exposing them.
    pub fn verify_account(&self, id: &AccountId) -> Result<AccountSummary, EngineError> {
        let account = self.auth.verify(id).map_err(EngineError::Authentication)?;
        self.accounts
            .record(account.clone())
            .map_err(EngineError::Authentication)?;
        Ok(account)
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
        assert!(engine.list_accounts().unwrap().is_empty());
        assert!(matches!(
            engine.start_sync(SyncRootId(1)),
            Err(EngineError::NotImplemented(_))
        ));
    }
}
