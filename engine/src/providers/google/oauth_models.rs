//! OAuth and OIDC protocol data; secret-bearing Debug output is redacted.
use serde::{Deserialize, Serialize};

/// JWT audiences can be one string or a list, without discarding claims.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GoogleAudience {
    One(String),
    Many(Vec<String>),
}
impl Default for GoogleAudience {
    fn default() -> Self {
        Self::One(String::new())
    }
}

/// Protocol contract for ConfigSources.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct GoogleConfigSources {
    #[serde(default)]
    pub client_id_env: Option<String>,
    #[serde(default)]
    pub client_secret_env: Option<String>,
    #[serde(default)]
    pub oauth_file_env: Option<String>,
    #[serde(default)]
    pub bundled_config_path: Option<String>,
}
impl std::fmt::Debug for GoogleConfigSources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleConfigSources")
            .finish_non_exhaustive()
    }
}

/// Protocol contract for OAuthConfig.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct GoogleOAuthConfig {
    pub client_id: String,
    #[serde(default)]
    pub client_secret: Option<String>,
    #[serde(default)]
    pub redirect_uris: Vec<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
}
impl std::fmt::Debug for GoogleOAuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleOAuthConfig").finish_non_exhaustive()
    }
}

/// Protocol contract for AuthorizationRequest.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct GoogleAuthorizationRequest {
    pub redirect_uri: String,
    pub state: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub access_type: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub login_hint: Option<String>,
}
impl std::fmt::Debug for GoogleAuthorizationRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleAuthorizationRequest")
            .finish_non_exhaustive()
    }
}

/// Protocol contract for AuthorizationUrl.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct GoogleAuthorizationUrl {
    pub url: String,
}
impl std::fmt::Debug for GoogleAuthorizationUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleAuthorizationUrl")
            .finish_non_exhaustive()
    }
}

/// Protocol contract for TokenResponse.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct GoogleTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub id_token: Option<String>,
    #[serde(default)]
    pub refresh_token_expires_in: Option<u64>,
}
impl std::fmt::Debug for GoogleTokenResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleTokenResponse")
            .finish_non_exhaustive()
    }
}

/// Protocol contract for UserInfo.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct GoogleUserInfo {
    pub sub: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub email_verified: Option<bool>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub picture: Option<String>,
    #[serde(default)]
    pub hd: Option<String>,
}
impl std::fmt::Debug for GoogleUserInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleUserInfo").finish_non_exhaustive()
    }
}

/// Protocol contract for IdTokenValidation.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct GoogleIdTokenValidation {
    pub issuer: String,
    pub audience: String,
    #[serde(default)]
    pub nonce: Option<String>,
    #[serde(default)]
    pub clock_skew_seconds: Option<u64>,
}
impl std::fmt::Debug for GoogleIdTokenValidation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleIdTokenValidation")
            .finish_non_exhaustive()
    }
}

/// Protocol contract for IdClaims.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct GoogleIdClaims {
    pub iss: String,
    pub aud: GoogleAudience,
    #[serde(default)]
    pub azp: Option<String>,
    pub sub: String,
    pub exp: u64,
    pub iat: u64,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub email_verified: Option<bool>,
    #[serde(default)]
    pub nonce: Option<String>,
}
impl std::fmt::Debug for GoogleIdClaims {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleIdClaims").finish_non_exhaustive()
    }
}

/// Protocol contract for OidcDiscovery.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct GoogleOidcDiscovery {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
    pub jwks_uri: String,
    #[serde(default)]
    pub revocation_endpoint: Option<String>,
    #[serde(default)]
    pub scopes_supported: Option<Vec<String>>,
}
impl std::fmt::Debug for GoogleOidcDiscovery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleOidcDiscovery")
            .finish_non_exhaustive()
    }
}

/// Protocol contract for JwkSet.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct GoogleJwkSet {
    #[serde(default)]
    pub keys: Vec<GoogleJwk>,
}
impl std::fmt::Debug for GoogleJwkSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleJwkSet").finish_non_exhaustive()
    }
}

/// Protocol contract for Jwk.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct GoogleJwk {
    #[serde(default)]
    pub kid: Option<String>,
    pub kty: String,
    #[serde(default)]
    pub alg: Option<String>,
    #[serde(rename = "use")]
    #[serde(default)]
    pub use_: Option<String>,
    #[serde(default)]
    pub n: Option<String>,
    #[serde(default)]
    pub e: Option<String>,
    #[serde(default)]
    pub crv: Option<String>,
    #[serde(default)]
    pub x: Option<String>,
    #[serde(default)]
    pub y: Option<String>,
}
impl std::fmt::Debug for GoogleJwk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleJwk").finish_non_exhaustive()
    }
}
