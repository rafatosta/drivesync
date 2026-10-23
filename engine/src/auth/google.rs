//! HTTP adapter for Google OAuth and OpenID Connect.
//!
//! Network requests have bounded durations and response sizes. Raw provider
//! error bodies are converted to typed, sanitized authentication errors.
use super::{
    models::{validate_tokens, Identity, Tokens},
    ports::GoogleApi,
    AuthError,
};
use oauth2::{
    basic::BasicClient, AuthUrl, ClientId, CsrfToken, PkceCodeChallenge, RedirectUrl, Scope,
};
use reqwest::blocking::{Client, Response};
use serde::Deserialize;
use std::{io::Read, time::Duration};

/// Read-only Drive permission requested during authorization.
pub(super) const DRIVE_SCOPE: &str = "https://www.googleapis.com/auth/drive.readonly";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const MAX_RESPONSE: u64 = 64 * 1024;

#[path = "../../oauth_config.rs"]
mod config;
/// OAuth configuration loaded from explicit runtime or packaged settings.
pub use config::OAuthConfig;
impl OAuthConfig {
    pub fn load() -> Result<Self, AuthError> {
        // Runtime overrides are deliberately separate from distribution build inputs.
        let read_env = |name| {
            std::env::var(name).map(Some).or_else(|e| match e {
                std::env::VarError::NotPresent => Ok(None),
                _ => Err(AuthError::Configuration),
            })
        };
        let id = read_env("DRIVESYNC_GOOGLE_CLIENT_ID")?;
        let secret = read_env("DRIVESYNC_GOOGLE_CLIENT_SECRET")?;
        let file = if id.is_none() {
            std::env::var_os("DRIVESYNC_GOOGLE_OAUTH_FILE")
                .map(|path| {
                    let file = std::fs::File::open(path).map_err(|_| AuthError::Configuration)?;
                    let mut data = String::new();
                    file.take(MAX_RESPONSE + 1)
                        .read_to_string(&mut data)
                        .map_err(|_| AuthError::Configuration)?;
                    Ok(data)
                })
                .transpose()?
        } else {
            None
        };
        Self::select(
            id,
            secret,
            file.as_deref(),
            include_str!(concat!(env!("OUT_DIR"), "/google-oauth.json")),
        )
        .map_err(|_| AuthError::Configuration)
    }
    pub(super) fn authorization_url(
        &self,
        redirect: &str,
        state: &CsrfToken,
        challenge: PkceCodeChallenge,
    ) -> Result<String, AuthError> {
        let client = BasicClient::new(ClientId::new(self.client_id.clone()))
            .set_auth_uri(
                AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".into())
                    .map_err(|_| AuthError::Configuration)?,
            )
            .set_redirect_uri(
                RedirectUrl::new(redirect.into()).map_err(|_| AuthError::Configuration)?,
            );
        let (url, _) = client
            .authorize_url(|| state.clone())
            .set_pkce_challenge(challenge)
            .add_scope(Scope::new("openid".into()))
            .add_scope(Scope::new("email".into()))
            .add_scope(Scope::new(DRIVE_SCOPE.into()))
            .add_extra_param("access_type", "offline")
            .add_extra_param("prompt", "consent select_account")
            .url();
        Ok(url.into())
    }
}

/// Production implementation of the Google API port using blocking HTTP.
pub(super) struct GoogleHttp;
impl GoogleHttp {
    fn client() -> Result<Client, AuthError> {
        Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .user_agent("DriveSync/0.1")
            .build()
            .map_err(|_| AuthError::Network)
    }
    fn tokens<'a>(
        config: &'a OAuthConfig,
        mut form: Vec<(&'a str, &'a str)>,
        refreshing: bool,
    ) -> Result<Tokens, AuthError> {
        form.push(("client_id", &config.client_id));
        if let Some(secret) = &config.client_secret {
            form.push(("client_secret", secret));
        }
        let response = Self::client()?
            .post(TOKEN_URL)
            .form(&form)
            .send()
            .map_err(|_| AuthError::Network)?;
        parse_tokens(response, refreshing)
    }
}
impl GoogleApi for GoogleHttp {
    fn exchange(
        &self,
        config: &OAuthConfig,
        code: &str,
        redirect: &str,
        verifier: &str,
    ) -> Result<Tokens, AuthError> {
        Self::tokens(
            config,
            vec![
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", redirect),
                ("code_verifier", verifier),
            ],
            false,
        )
    }
    fn refresh(&self, config: &OAuthConfig, refresh: &str) -> Result<Tokens, AuthError> {
        Self::tokens(
            config,
            vec![("grant_type", "refresh_token"), ("refresh_token", refresh)],
            true,
        )
    }
    fn identify(&self, access: &str) -> Result<Identity, AuthError> {
        let response = Self::client()?
            .get("https://openidconnect.googleapis.com/v1/userinfo")
            .bearer_auth(access)
            .send()
            .map_err(|_| AuthError::Network)?;
        let status = response.status().as_u16();
        let data = read_body(response)?;
        match status {
            200 => serde_json::from_slice(&data).map_err(|_| AuthError::InvalidResponse),
            401 | 403 => Err(AuthError::ReauthorizationRequired),
            429 | 500..=599 => Err(AuthError::ProviderUnavailable),
            _ => Err(AuthError::InvalidResponse),
        }
    }
}
fn read_body(response: Response) -> Result<Vec<u8>, AuthError> {
    let mut data = Vec::new();
    response
        .take(MAX_RESPONSE + 1)
        .read_to_end(&mut data)
        .map_err(|_| AuthError::Network)?;
    if data.len() as u64 > MAX_RESPONSE {
        return Err(AuthError::InvalidResponse);
    }
    Ok(data)
}
fn parse_tokens(response: Response, refreshing: bool) -> Result<Tokens, AuthError> {
    let status = response.status().as_u16();
    decode_tokens(status, &read_body(response)?, refreshing)
}
/// Decodes and sanitizes the provider token response without leaking its body.
///
/// # Errors
///
/// Returns typed authentication errors for rejected grants, invalid responses,
/// or transient provider failures.
pub(super) fn decode_tokens(
    status: u16,
    data: &[u8],
    refreshing: bool,
) -> Result<Tokens, AuthError> {
    if status == 429 || status >= 500 {
        return Err(AuthError::ProviderUnavailable);
    }
    if status != 200 {
        #[derive(Deserialize)]
        struct ErrorBody {
            error: String,
        }
        let error =
            serde_json::from_slice::<ErrorBody>(data).map_err(|_| AuthError::InvalidResponse)?;
        return Err(match error.error.as_str() {
            "invalid_grant" if refreshing => AuthError::ReauthorizationRequired,
            "invalid_grant" => AuthError::InvalidCode,
            "access_denied" => AuthError::Denied,
            "invalid_client" | "unauthorized_client" => AuthError::Configuration,
            "temporarily_unavailable" | "server_error" => AuthError::ProviderUnavailable,
            _ => AuthError::InvalidResponse,
        });
    }
    #[derive(Deserialize)]
    struct TokenBody {
        access_token: String,
        token_type: String,
        expires_in: u64,
        refresh_token: Option<String>,
        scope: Option<String>,
    }
    let body: TokenBody = serde_json::from_slice(data).map_err(|_| AuthError::InvalidResponse)?;
    if !body.token_type.eq_ignore_ascii_case("bearer") {
        return Err(AuthError::InvalidResponse);
    }
    let tokens = Tokens {
        access_token: body.access_token,
        refresh_token: body.refresh_token,
        expires_in: body.expires_in,
        scope: body.scope,
    };
    validate_tokens(&tokens)?;
    Ok(tokens)
}
