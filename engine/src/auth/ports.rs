//! Adapter contracts for Google OAuth and the desktop credential store.
//!
//! Traits let the provider logic be tested without real network or vault access.
use super::{
    google::OAuthConfig,
    models::{Credential, Identity, Tokens},
    AuthError,
};
use crate::AccountId;

/// Port for Google OAuth token and identity operations.
/// Implemented by the HTTP adapter and in-memory fakes in tests.
pub(super) trait GoogleApi: Send + Sync {
    /// Exchanges an authorization code using the PKCE verifier.
    fn exchange(
        &self,
        config: &OAuthConfig,
        code: &str,
        redirect: &str,
        verifier: &str,
    ) -> Result<Tokens, AuthError>;
    /// Renews an access token without requiring refresh-token rotation.
    fn refresh(&self, config: &OAuthConfig, refresh: &str) -> Result<Tokens, AuthError>;
    /// Retrieves the account identity for an access token.
    fn identify(&self, access: &str) -> Result<Identity, AuthError>;
}
/// Port for credential storage scoped by OAuth client and account.
/// Implementations must not treat vault errors as missing credentials.
pub(super) trait CredentialStore: Send + Sync {
    /// Loads saved credentials; `Ok(None)` means confirmed absence only.
    fn load(&self, client: &str, id: &AccountId) -> Result<Option<Credential>, AuthError>;
    /// Persists credentials securely for one OAuth client and account.
    fn save(&self, client: &str, id: &AccountId, value: &Credential) -> Result<(), AuthError>;
    /// Deletes credentials; absence is treated as success by the adapter.
    fn delete(&self, client: &str, id: &AccountId) -> Result<(), AuthError>;
}
