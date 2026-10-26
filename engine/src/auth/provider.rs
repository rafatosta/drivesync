//! OAuth workflow orchestration and account-scoped synchronization.
//!
//! Does not implement HTTP or platform vault APIs directly. Those services
//! are injected through ports and have production adapters in sibling modules.
use super::{
    callback::{self, Cancellation},
    google::{self, OAuthConfig},
    models::{validate_tokens, Credential, Tokens},
    ports::{CredentialStore, GoogleApi},
    storage, AuthError,
};
use crate::{AccountId, AccountSummary};
use oauth2::{CsrfToken, PkceCodeChallenge};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Per-account mutexes shared across operations on the same provider instance.
pub(super) type AccountLocks = Mutex<HashMap<AccountId, Arc<Mutex<()>>>>;
/// Coordinates OAuth, credential persistence and per-account verification.
/// The interactive-operation gate prevents concurrent login/logout, while
/// individual account locks serialize reads, token renewals and vault updates.
pub(crate) struct AuthProvider {
    pub(super) api: Arc<dyn GoogleApi>,
    pub(super) store: Arc<dyn CredentialStore>,
    pub(super) locks: AccountLocks,
    pub(super) authorizing: AtomicBool,
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
/// Resets the interactive-operation gate when authorization or logout ends.
struct LoginGuard<'a>(&'a AtomicBool);
impl Drop for LoginGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
impl AuthProvider {
    /// Begins interactive OAuth with PKCE and an ephemeral loopback callback.
    ///
    /// The browser callback is a port supplied by the caller; neither tokens
    /// nor stored credentials are returned through it. Successful completion
    /// requires identity verification and credential-vault persistence.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError`] for an existing interactive operation,
    /// cancellation, callback or provider failures, identity mismatches or
    /// credential-vault errors.
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

    /// Performs OAuth authorization after the caller has handled the login gate.
    ///
    /// # Errors
    ///
    /// Propagates cancellation, browser, callback, provider and storage errors.
    pub(super) fn authorize_config(
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

    /// Returns the shared mutex for a provider account, creating it as needed.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::Internal`] if the lock registry is poisoned.
    pub(super) fn account_lock(&self, id: &AccountId) -> Result<Arc<Mutex<()>>, AuthError> {
        Ok(self
            .locks
            .lock()
            .map_err(|_| AuthError::Internal)?
            .entry(id.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone())
    }

    /// Checks provider identity and stores a validated token bundle.
    ///
    /// Preserves an existing refresh token if the provider does not rotate it.
    /// Credential writes occur under the account's mutex.
    ///
    /// # Errors
    ///
    /// Returns an authentication error for invalid tokens, identity changes,
    /// cancellation or vault failures; it never reports success before save.
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
        let write_access = tokens.scope.as_deref().is_some_and(|scope| {
            scope.split_whitespace().any(|granted| granted == google::DRIVE_SCOPE)
        });
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
            write_access,
        };
        cancel.check()?;
        self.store
            .save(&config.client_id, &account.id, &credential)?;
        Ok(account)
    }

    pub(crate) fn can_upload(
        &self,
        config: &OAuthConfig,
        id: &AccountId,
    ) -> Result<bool, AuthError> {
        let lock = self.account_lock(id)?;
        let _guard = lock.lock().map_err(|_| AuthError::Internal)?;
        let credential = self
            .store
            .load(&config.client_id, id)?
            .ok_or(AuthError::ReauthorizationRequired)?;
        if AccountId::from_subject(&credential.subject) != *id {
            return Err(AuthError::IdentityMismatch);
        }
        Ok(credential.write_access)
    }
    /// Removes old profile metadata only when the vault confirms no entry.
    ///
    /// A locked, unavailable or malformed vault is never evidence of logout.
    /// The supplied cleanup callback runs only after confirmed absence.
    ///
    /// # Errors
    ///
    /// Returns errors from locking or the cleanup callback.
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
    /// Removes a stored credential and then runs app-metadata cleanup.
    ///
    /// Login and logout share an interactive gate; the account mutex also
    /// waits for any in-flight account work. A failed metadata cleanup may
    /// occur after vault deletion and must be reported to the caller.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::Busy`] during login, or propagates vault,
    /// synchronization and metadata-cleanup errors.
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

    /// Performs an engine operation with a verified access token.
    ///
    /// The bearer token is passed only to the supplied closure while the
    /// account mutex is held. Callers must not expose it in return values,
    /// logs or user-visible errors.
    ///
    /// # Errors
    ///
    /// Returns an [`crate::EngineError`] from verification or the operation.
    pub(crate) fn with_access<T>(
        &self,
        id: &AccountId,
        operation: impl FnOnce(&str) -> Result<T, crate::EngineError>,
    ) -> Result<T, crate::EngineError> {
        let config = OAuthConfig::load().map_err(crate::EngineError::Authentication)?;
        self.with_access_config(&config, id, operation)
    }
    /// Like [`Self::with_access`], using a supplied OAuth configuration.
    ///
    /// Verifies the account before locking and reloading its credential,
    /// so renewal completes without holding the same mutex twice.
    ///
    /// # Errors
    ///
    /// Returns [`crate::EngineError`] for authorization, vault or operation errors.
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

    /// Verifies a saved account and renews nearly expired access tokens.
    ///
    /// The account lock serializes renewals. Updated credentials are saved
    /// only after identity is verified; a refresh response that omits a new
    /// refresh token preserves the previous one.
    ///
    /// # Errors
    ///
    /// Returns an [`AuthError`] for missing credentials, invalid identity,
    /// provider errors or failed vault access.
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
            if let Some(scope) = &tokens.scope {
                credential.write_access =
                    scope.split_whitespace().any(|s| s == google::DRIVE_SCOPE);
            }
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
/// Returns current Unix time in seconds.
///
/// # Errors
///
/// Returns [`AuthError::Internal`] if the system clock predates the epoch.
pub(super) fn now() -> Result<u64, AuthError> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AuthError::Internal)?
        .as_secs())
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
