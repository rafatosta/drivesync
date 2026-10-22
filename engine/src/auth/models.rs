//! Private authentication data types and validation.
//!
//! Credentials are sensitive and never implement `Debug`. These models are
//! distinct from the public, sanitized account metadata in the engine.
use super::{google, AuthError};
use crate::{AccountId, AccountSummary};
use serde::{Deserialize, Serialize};

// Deliberately no Debug on credentials or provider responses.
/// Credential bundle stored only in the desktop credential vault.
/// Never log, print, or serialize this value outside secure storage.
#[derive(Serialize, Deserialize)]
pub(super) struct Credential {
    /// Provider subject used to detect mismatched account identity.
    pub(super) subject: String,
    /// Short-lived bearer token; never expose to the UI.
    pub(super) access_token: String,
    /// Refresh credential reused until a replacement is supplied.
    pub(super) refresh_token: String,
    /// Unix expiration timestamp, in seconds.
    pub(super) expires_at: u64,
}
/// OAuth tokens returned by a provider exchange or refresh.
/// A refresh response may omit a new refresh token; retain the stored one.
pub(super) struct Tokens {
    pub(super) access_token: String,
    pub(super) refresh_token: Option<String>,
    pub(super) expires_in: u64,
    pub(super) scope: Option<String>,
}
/// Provider identity response, not yet validated as an account summary.
/// Email is optional and used only when the provider marked it verified.
#[derive(Deserialize)]
pub(super) struct Identity {
    pub(super) sub: String,
    pub(super) email: Option<String>,
    #[serde(default)]
    pub(super) email_verified: bool,
}
impl Identity {
    /// Checks provider identity and returns sanitized account metadata.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::InvalidResponse`] for an empty, oversized or
    /// control-character-containing provider subject.
    pub(super) fn summary(self) -> Result<AccountSummary, AuthError> {
        if self.sub.is_empty() || self.sub.len() > 255 || self.sub.chars().any(char::is_control) {
            return Err(AuthError::InvalidResponse);
        }
        Ok(AccountSummary {
            id: AccountId::from_subject(&self.sub),
            provider_subject: self.sub,
            email: self
                .email
                .filter(|email| self.email_verified && !email.is_empty()),
        })
    }
}
/// Validates a token response before credentials are accepted or persisted.
///
/// Rejects missing or short-lived access tokens and missing required scopes
/// when the provider includes a scope list.
///
/// # Errors
///
/// Returns [`AuthError::InvalidResponse`] or
/// [`AuthError::MissingPermissions`] for invalid token responses.
pub(super) fn validate_tokens(tokens: &Tokens) -> Result<(), AuthError> {
    if tokens.access_token.is_empty() || tokens.expires_in <= 60 {
        return Err(AuthError::InvalidResponse);
    }
    if let Some(scope) = &tokens.scope {
        let scopes: Vec<_> = scope.split_whitespace().collect();
        if !scopes.contains(&"openid") || !scopes.contains(&google::DRIVE_SCOPE) {
            return Err(AuthError::MissingPermissions);
        }
    }
    Ok(())
}
