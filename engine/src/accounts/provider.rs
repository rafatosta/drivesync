//! Accounts-owned adaptation of existing provider contracts; no HTTP here.
use super::*;
use crate::providers::google::*;
use credentials::Credentials;
/// An already obtained provider authorization. Secret fields remain private.
pub struct ProviderAuthorizationHandle {
    pub(super) provider: ProviderId,
    pub(super) tokens: GoogleTokenResponse,
}
impl ProviderAuthorizationHandle {
    /// Consumes the technical result obtained through Google's provider API.
    /// Does not perform consent, code exchange or any network request.
    pub fn google(tokens: GoogleTokenResponse) -> Self {
        Self {
            provider: ProviderId("google".into()),
            tokens,
        }
    }
}
impl std::fmt::Debug for ProviderAuthorizationHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProviderAuthorizationHandle { .. }")
    }
}
/// Opaque session context, exposing only the provider-specific safe session.
#[derive(Debug)]
pub struct ProviderSessionHandle {
    pub(super) google: GoogleSession,
}
impl ProviderSessionHandle {
    /// Borrows the provider session without exposing its access token.
    pub fn google(&self) -> &GoogleSession {
        &self.google
    }
}
pub(super) struct Identity {
    pub subject: String,
    pub email: Option<String>,
    pub name: Option<String>,
}
/// Internal dependency seam, not a universal trait added to Providers.
pub(super) trait Access: Send + Sync {
    fn identity(&self, runtime: &tokio::runtime::Handle, access: &str) -> AccountsResult<Identity>;
    fn refresh(
        &self,
        runtime: &tokio::runtime::Handle,
        value: &Credentials,
    ) -> AccountsResult<GoogleTokenResponse>;
}
pub(super) struct GoogleAccess {
    pub client: GoogleDriveClient,
    pub config: GoogleOAuthConfig,
}
fn failure(error: GoogleApiFailure) -> AccountsFailure {
    let kind = match error.kind {
        GoogleApiError::InvalidGrant
        | GoogleApiError::AuthenticationRequired
        | GoogleApiError::ReauthorizationRequired
        | GoogleApiError::PermissionDenied
        | GoogleApiError::InsufficientScope => AccountsError::AuthorizationFailed,
        _ => AccountsError::ProviderUnavailable,
    };
    let mut failure = AccountsFailure::new(kind);
    failure.retryable = error.retryable;
    failure
}
impl Access for GoogleAccess {
    fn identity(&self, runtime: &tokio::runtime::Handle, access: &str) -> AccountsResult<Identity> {
        let session = GoogleSession::new("identity-verification", access);
        let info = runtime
            .block_on(self.client.get_userinfo(&session))
            .map_err(failure)?;
        Ok(Identity {
            subject: info.sub,
            email: info.email,
            name: info.name,
        })
    }
    fn refresh(
        &self,
        runtime: &tokio::runtime::Handle,
        value: &Credentials,
    ) -> AccountsResult<GoogleTokenResponse> {
        let token = value
            .refresh
            .as_deref()
            .ok_or_else(|| AccountsFailure::new(AccountsError::AuthorizationFailed))?;
        runtime
            .block_on(self.client.refresh_access_token(&self.config, token))
            .map_err(failure)
    }
}
