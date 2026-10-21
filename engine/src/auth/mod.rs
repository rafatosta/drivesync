//! Desktop OAuth. No GTK dependencies; secrets never cross the engine facade.
mod callback;
mod google;
mod storage;
#[cfg(test)]
mod tests;

use crate::{AccountId, AccountSummary};
use oauth2::{CsrfToken, PkceCodeChallenge};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub use callback::Cancellation;
pub use google::OAuthConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthError {
    Configuration,
    Busy,
    Cancelled,
    TimedOut,
    BrowserUnavailable,
    CallbackUnavailable,
    InvalidCallback,
    Denied,
    InvalidCode,
    Network,
    ProviderUnavailable,
    InvalidResponse,
    MissingPermissions,
    IdentityMismatch,
    ReauthorizationRequired,
    SecureStorage,
    AccountStorage,
    Internal,
}
impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Configuration => {
                "Google connection is unavailable in this build. Please install a configured DriveSync release or contact the distributor."
            }
            Self::Busy => "An account connection is already in progress.",
            Self::Cancelled => "Account connection was cancelled.",
            Self::TimedOut => "Google authorization timed out. Please try again.",
            Self::BrowserUnavailable => "Could not open the system browser. Please try again.",
            Self::CallbackUnavailable => "Could not receive the browser's authorization response.",
            Self::InvalidCallback => "The authorization response could not be verified.",
            Self::Denied => "Google authorization was declined.",
            Self::InvalidCode => {
                "The authorization code expired or was rejected. Please connect again."
            }
            Self::Network => "Could not reach Google. Check your connection and try again.",
            Self::ProviderUnavailable => {
                "Google is temporarily unavailable. Please try again later."
            }
            Self::InvalidResponse => "Google returned an unexpected authorization response.",
            Self::MissingPermissions => {
                "Required permissions were not granted. Please connect again."
            }
            Self::IdentityMismatch => "The credential does not belong to the requested account.",
            Self::ReauthorizationRequired => "The account needs to be connected again.",
            Self::SecureStorage => {
                "Could not access the desktop credential vault. Unlock your keyring and try again."
            }
            Self::AccountStorage => "Could not read or update the account list. Please try again.",
            Self::Internal => {
                "Account connection could not be completed. Please restart DriveSync."
            }
        })
    }
}
impl std::error::Error for AuthError {}

// Deliberately no Debug on credentials or provider responses.
#[derive(Serialize, Deserialize)]
struct Credential {
    subject: String,
    access_token: String,
    refresh_token: String,
    expires_at: u64,
}
struct Tokens {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
    scope: Option<String>,
}
#[derive(Deserialize)]
struct Identity {
    sub: String,
    email: Option<String>,
    #[serde(default)]
    email_verified: bool,
}
trait GoogleApi: Send + Sync {
    fn exchange(
        &self,
        config: &OAuthConfig,
        code: &str,
        redirect: &str,
        verifier: &str,
    ) -> Result<Tokens, AuthError>;
    fn refresh(&self, config: &OAuthConfig, refresh: &str) -> Result<Tokens, AuthError>;
    fn identify(&self, access: &str) -> Result<Identity, AuthError>;
}
trait CredentialStore: Send + Sync {
    fn load(&self, client: &str, id: &AccountId) -> Result<Option<Credential>, AuthError>;
    fn save(&self, client: &str, id: &AccountId, value: &Credential) -> Result<(), AuthError>;
    fn delete(&self, client: &str, id: &AccountId) -> Result<(), AuthError>;
}

type AccountLocks = Mutex<HashMap<AccountId, Arc<Mutex<()>>>>;
pub(crate) struct AuthProvider {
    api: Arc<dyn GoogleApi>,
    store: Arc<dyn CredentialStore>,
    locks: AccountLocks,
    authorizing: AtomicBool,
}
impl Default for AuthProvider {
    fn default() -> Self {
        Self {
            api: Arc::new(google::GoogleHttp),
            store: Arc::new(storage::DesktopVault),
            locks: Mutex::new(HashMap::new()),
            authorizing: AtomicBool::new(false),
        }
    }
}
struct LoginGuard<'a>(&'a AtomicBool);
impl Drop for LoginGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
impl AuthProvider {
    pub(crate) fn authorize(
        &self,
        config: &OAuthConfig,
        cancel: &Cancellation,
        open_browser: impl FnOnce(&str) -> Result<(), AuthError>,
    ) -> Result<AccountSummary, AuthError> {
        if self.authorizing.swap(true, Ordering::AcqRel) {
            return Err(AuthError::Busy);
        }
        let _guard = LoginGuard(&self.authorizing);
        cancel.check()?;
        self.authorize_config(config, cancel, open_browser)
    }

    fn authorize_config(
        &self,
        config: &OAuthConfig,
        cancel: &Cancellation,
        open_browser: impl FnOnce(&str) -> Result<(), AuthError>,
    ) -> Result<AccountSummary, AuthError> {
        cancel.check()?;
        let listener = callback::Listener::bind()?;
        let redirect = listener.redirect_uri();
        let state = CsrfToken::new_random();
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let url = config.authorization_url(&redirect, &state, challenge)?;
        open_browser(&url)?;
        let code = listener.receive(&state, cancel, Duration::from_secs(300))?;
        cancel.check()?;
        let tokens = self
            .api
            .exchange(config, &code, &redirect, verifier.secret())?;
        self.complete(config, tokens, cancel)
    }

    fn account_lock(&self, id: &AccountId) -> Result<Arc<Mutex<()>>, AuthError> {
        Ok(self
            .locks
            .lock()
            .map_err(|_| AuthError::Internal)?
            .entry(id.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone())
    }

    fn complete(
        &self,
        config: &OAuthConfig,
        tokens: Tokens,
        cancel: &Cancellation,
    ) -> Result<AccountSummary, AuthError> {
        validate_tokens(&tokens)?;
        cancel.check()?;
        let identity = self.api.identify(&tokens.access_token)?;
        let account = identity.summary()?;
        let lock = self.account_lock(&account.id)?;
        let _guard = lock.lock().map_err(|_| AuthError::Internal)?;
        let old = self.store.load(&config.client_id, &account.id)?;
        if old
            .as_ref()
            .is_some_and(|c| c.subject != account.provider_subject)
        {
            return Err(AuthError::IdentityMismatch);
        }
        let refresh = tokens
            .refresh_token
            .or_else(|| old.map(|c| c.refresh_token))
            .filter(|s| !s.is_empty())
            .ok_or(AuthError::ReauthorizationRequired)?;
        let credential = Credential {
            subject: account.provider_subject.clone(),
            access_token: tokens.access_token,
            refresh_token: refresh,
            expires_at: now()?.saturating_add(tokens.expires_in),
        };
        cancel.check()?;
        self.store
            .save(&config.client_id, &account.id, &credential)?;
        Ok(account)
    }

    pub(crate) fn prune_if_missing(
        &self,
        config: &OAuthConfig,
        id: &AccountId,
        forget: impl FnOnce() -> Result<(), AuthError>,
    ) -> Result<(), AuthError> {
        if self.authorizing.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let _login_guard = LoginGuard(&self.authorizing);
        let lock = self.account_lock(id)?;
        let _guard = lock.lock().map_err(|_| AuthError::Internal)?;
        match self.store.load(&config.client_id, id) {
            Ok(None) => forget(),
            // A locked/unavailable/corrupt vault is never evidence of logout.
            Ok(Some(_)) | Err(_) => Ok(()),
        }
    }
    #[cfg(test)]
    pub(crate) fn sign_out(&self, config: &OAuthConfig, id: &AccountId) -> Result<(), AuthError> {
        self.sign_out_then(config, id, || Ok(()))
    }
    pub(crate) fn sign_out_then(
        &self,
        config: &OAuthConfig,
        id: &AccountId,
        forget: impl FnOnce() -> Result<(), AuthError>,
    ) -> Result<(), AuthError> {
        // Share the interactive-operation gate: an in-flight login must not
        // recreate credentials after successful logout.
        if self.authorizing.swap(true, Ordering::AcqRel) {
            return Err(AuthError::Busy);
        }
        let _login_guard = LoginGuard(&self.authorizing);
        let lock = self.account_lock(id)?;
        let _guard = lock.lock().map_err(|_| AuthError::Internal)?;
        self.store.delete(&config.client_id, id)?;
        forget()
    }

    pub(crate) fn with_access<T>(
        &self,
        id: &AccountId,
        operation: impl FnOnce(&str) -> Result<T, crate::EngineError>,
    ) -> Result<T, crate::EngineError> {
        let config = OAuthConfig::load().map_err(crate::EngineError::Authentication)?;
        self.with_access_config(&config, id, operation)
    }
    pub(crate) fn with_access_config<T>(
        &self,
        config: &OAuthConfig,
        id: &AccountId,
        operation: impl FnOnce(&str) -> Result<T, crate::EngineError>,
    ) -> Result<T, crate::EngineError> {
        self.verify_with_config(config, id)
            .map_err(crate::EngineError::Authentication)?;
        let lock = self
            .account_lock(id)
            .map_err(crate::EngineError::Authentication)?;
        let _guard = lock
            .lock()
            .map_err(|_| crate::EngineError::Authentication(AuthError::Internal))?;
        let credential = self
            .store
            .load(&config.client_id, id)
            .map_err(crate::EngineError::Authentication)?
            .ok_or(crate::EngineError::Authentication(
                AuthError::ReauthorizationRequired,
            ))?;
        operation(&credential.access_token)
    }

    pub(crate) fn verify_with_config(
        &self,
        config: &OAuthConfig,
        id: &AccountId,
    ) -> Result<AccountSummary, AuthError> {
        let lock = self.account_lock(id)?;
        let _guard = lock.lock().map_err(|_| AuthError::Internal)?;
        let mut credential = self
            .store
            .load(&config.client_id, id)?
            .ok_or(AuthError::ReauthorizationRequired)?;
        if AccountId::from_subject(&credential.subject) != *id {
            return Err(AuthError::IdentityMismatch);
        }
        let renewed = credential.expires_at <= now()?.saturating_add(60);
        if renewed {
            let tokens = self.api.refresh(config, &credential.refresh_token)?;
            validate_tokens(&tokens)?;
            credential.access_token = tokens.access_token;
            if let Some(refresh) = tokens.refresh_token.filter(|s| !s.is_empty()) {
                credential.refresh_token = refresh;
            }
            credential.expires_at = now()?.saturating_add(tokens.expires_in);
        }
        let account = self.api.identify(&credential.access_token)?.summary()?;
        if account.id != *id {
            return Err(AuthError::IdentityMismatch);
        }
        if renewed {
            self.store.save(&config.client_id, id, &credential)?;
        }
        Ok(account)
    }
}
impl Identity {
    fn summary(self) -> Result<AccountSummary, AuthError> {
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
fn validate_tokens(tokens: &Tokens) -> Result<(), AuthError> {
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
fn now() -> Result<u64, AuthError> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AuthError::Internal)?
        .as_secs())
}
