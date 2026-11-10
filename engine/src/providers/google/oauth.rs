//! Desktop OAuth, fixed identity endpoints and RS256 ID-token verification.
//! No browser, callback server, account storage or credential persistence.
use super::client::{decode, invalid_argument};
use super::*;
use reqwest::{Method, Url};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::AsyncReadExt;

const AUTH: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN: &str = "https://oauth2.googleapis.com/token";
const REVOKE: &str = "https://oauth2.googleapis.com/revoke";
const USERINFO: &str = "https://openidconnect.googleapis.com/v1/userinfo";
const DISCOVERY: &str = "https://accounts.google.com/.well-known/openid-configuration";
const KEYS: &str = "https://www.googleapis.com/oauth2/v3/certs";

impl GoogleDriveClient {
    /// Reads at most 64 KiB of desktop configuration asynchronously. Individual
    /// resolved overrides take precedence over the selected configuration file.
    /// Invalid explicit files fail without falling back or exposing their path.
    pub async fn load_google_config(
        &self,
        sources: GoogleConfigSources,
    ) -> GoogleResult<GoogleOAuthConfig> {
        let path = sources.oauth_file_env.or(sources.bundled_config_path);
        let mut config: GoogleOAuthConfig = if let Some(path) = path {
            let file = tokio::fs::File::open(path)
                .await
                .map_err(|_| invalid_argument())?;
            let mut bytes = Vec::new();
            file.take(65537)
                .read_to_end(&mut bytes)
                .await
                .map_err(|_| invalid_argument())?;
            if bytes.len() > 65536 {
                return Err(invalid_argument());
            }
            let value: serde_json::Value =
                serde_json::from_slice(&bytes).map_err(|_| invalid_argument())?;
            let value = value.get("installed").unwrap_or(&value);
            serde_json::from_value(value.clone()).map_err(|_| invalid_argument())?
        } else {
            GoogleOAuthConfig::default()
        };
        if let Some(id) = sources.client_id_env {
            config.client_id = id;
        }
        if let Some(secret) = sources.client_secret_env {
            config.client_secret = Some(secret);
        }
        if config.client_id.is_empty()
            || config
                .redirect_uris
                .iter()
                .any(|uri| redirect(uri).is_err())
        {
            return Err(invalid_argument());
        }
        Ok(config)
    }

    /// Builds the official browser URL. The caller owns state/PKCE generation
    /// and callback validation; secrets are never included in this URL.
    pub fn authorization_request(
        &self,
        config: &GoogleOAuthConfig,
        request: GoogleAuthorizationRequest,
    ) -> GoogleResult<GoogleAuthorizationUrl> {
        if config.client_id.is_empty()
            || request.state.is_empty()
            || request.code_challenge_method != "S256"
            || request.code_challenge.len() != 43
            || !request
                .code_challenge
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || request.scopes.is_empty()
            || request
                .scopes
                .iter()
                .any(|scope| scope.is_empty() || !config.scopes.contains(scope))
        {
            return Err(invalid_argument());
        }
        validate_redirect(config, &request.redirect_uri)?;
        let mut url = Url::parse(AUTH).expect("constant OAuth URL is valid");
        {
            let mut query = url.query_pairs_mut();
            query.extend_pairs([
                ("client_id", config.client_id.as_str()),
                ("response_type", "code"),
                ("redirect_uri", request.redirect_uri.as_str()),
                ("state", request.state.as_str()),
                ("code_challenge", request.code_challenge.as_str()),
                ("code_challenge_method", "S256"),
                ("scope", request.scopes.join(" ").as_str()),
            ]);
            for (key, value) in [
                ("access_type", request.access_type.as_deref()),
                ("prompt", request.prompt.as_deref()),
                ("login_hint", request.login_hint.as_deref()),
            ] {
                if let Some(value) = value {
                    query.append_pair(key, value);
                }
            }
        }
        Ok(GoogleAuthorizationUrl { url: url.into() })
    }

    /// Exchanges one authorization code; a failed response never causes retry.
    pub async fn exchange_authorization_code(
        &self,
        config: &GoogleOAuthConfig,
        code: &str,
        redirect_uri: &str,
        pkce_verifier: &str,
    ) -> GoogleResult<GoogleTokenResponse> {
        validate_redirect(config, redirect_uri)?;
        if config.client_id.is_empty()
            || code.is_empty()
            || !(43..=128).contains(&pkce_verifier.len())
            || !pkce_verifier
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
        {
            return Err(invalid_argument());
        }
        let mut form = vec![
            ("grant_type", "authorization_code"),
            ("client_id", &config.client_id),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", pkce_verifier),
        ];
        if let Some(secret) = &config.client_secret {
            form.push(("client_secret", secret));
        }
        token_response(&self.oauth_call("token", &form).await?)
    }

    /// Refreshes access without inventing a replacement refresh token.
    pub async fn refresh_access_token(
        &self,
        config: &GoogleOAuthConfig,
        refresh_token: &str,
    ) -> GoogleResult<GoogleTokenResponse> {
        if config.client_id.is_empty() || refresh_token.is_empty() {
            return Err(invalid_argument());
        }
        let mut form = vec![
            ("grant_type", "refresh_token"),
            ("client_id", &config.client_id),
            ("refresh_token", refresh_token),
        ];
        if let Some(secret) = &config.client_secret {
            form.push(("client_secret", secret));
        }
        token_response(&self.oauth_call("token", &form).await?)
    }

    /// Revokes one token using a form body, never a URL query or log message.
    pub async fn revoke_token(&self, token: &str) -> GoogleResult<()> {
        if token.is_empty() {
            return Err(invalid_argument());
        }
        self.oauth_call("revoke", &[("token", token)]).await?;
        Ok(())
    }

    /// Retrieves the stable OpenID subject under the supplied session.
    pub async fn get_userinfo(&self, session: &GoogleSession) -> GoogleResult<GoogleUserInfo> {
        let response = self
            .send(
                session,
                Method::GET,
                self.identity_endpoint("userinfo"),
                None,
                &[],
            )
            .await?;
        let info: GoogleUserInfo = decode(&self.response_body(response, 200).await?)?;
        if info.sub.is_empty() {
            return Err(invalid_response());
        }
        Ok(info)
    }

    /// Verifies RS256 signature, key ID, issuer, audience, times and optional
    /// nonce. Only fixed Google discovery/key endpoints are trusted. No cache
    /// or automatic refresh loop is created; unknown keys fail this call.
    pub async fn validate_id_token(
        &self,
        id_token: &str,
        validation: GoogleIdTokenValidation,
    ) -> GoogleResult<GoogleIdClaims> {
        use jsonwebtoken::{Algorithm, DecodingKey, Validation};
        let bad = || GoogleApiFailure::new(GoogleApiError::AuthenticationRequired, None);
        let skew = validation.clock_skew_seconds.unwrap_or(60);
        if !["https://accounts.google.com", "accounts.google.com"]
            .contains(&validation.issuer.as_str())
            || validation.audience.is_empty()
            || skew > 300
        {
            return Err(invalid_argument());
        }
        if id_token.len() > 32768 {
            return Err(bad());
        }
        let header = jsonwebtoken::decode_header(id_token).map_err(|_| bad())?;
        if header.alg != Algorithm::RS256 {
            return Err(bad());
        }
        let kid = header
            .kid
            .as_deref()
            .filter(|kid| !kid.is_empty())
            .ok_or_else(bad)?;
        let keys = self.get_oidc_keys().await?;
        let mut matching = keys
            .keys
            .iter()
            .filter(|key| key.kid.as_deref() == Some(kid));
        let key = matching.next().ok_or_else(bad)?;
        if matching.next().is_some()
            || key.kty != "RSA"
            || key.alg.as_deref().is_some_and(|alg| alg != "RS256")
            || key.use_.as_deref().is_some_and(|usage| usage != "sig")
        {
            return Err(bad());
        }
        let key = DecodingKey::from_rsa_components(
            key.n.as_deref().ok_or_else(bad)?,
            key.e.as_deref().ok_or_else(bad)?,
        )
        .map_err(|_| bad())?;
        let mut rules = Validation::new(Algorithm::RS256);
        rules.leeway = skew;
        rules.validate_nbf = true;
        rules.set_issuer(&[&validation.issuer]);
        rules.set_audience(&[&validation.audience]);
        rules.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        let claims = jsonwebtoken::decode::<GoogleIdClaims>(id_token, &key, &rules)
            .map_err(|_| bad())?
            .claims;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| bad())?
            .as_secs();
        let multiple = matches!(&claims.aud,GoogleAudience::Many(audiences) if audiences.len()>1);
        if claims.sub.is_empty()
            || claims.iat > now.saturating_add(skew)
            || claims.exp.saturating_add(skew) <= now
            || claims.exp <= claims.iat
            || validation
                .nonce
                .as_ref()
                .is_some_and(|expected| claims.nonce.as_ref() != Some(expected))
            || claims
                .azp
                .as_ref()
                .is_some_and(|azp| azp != &validation.audience)
            || (multiple && claims.azp.is_none())
        {
            return Err(bad());
        }
        Ok(claims)
    }

    /// Retrieves discovery from its fixed Google origin and validates endpoint
    /// identity before any returned URL can be used by another operation.
    pub async fn get_oidc_discovery(&self) -> GoogleResult<GoogleOidcDiscovery> {
        let discovery: GoogleOidcDiscovery = decode(&self.oauth_call("discovery", &[]).await?)?;
        if discovery.issuer != "https://accounts.google.com"
            || discovery.authorization_endpoint != AUTH
            || discovery.token_endpoint != TOKEN
            || discovery.userinfo_endpoint != USERINFO
            || discovery.jwks_uri != KEYS
        {
            return Err(invalid_response());
        }
        Ok(discovery)
    }

    /// Retrieves typed public signing keys after checking the discovery URL.
    pub async fn get_oidc_keys(&self) -> GoogleResult<GoogleJwkSet> {
        self.get_oidc_discovery().await?;
        let keys: GoogleJwkSet = decode(&self.oauth_call("keys", &[]).await?)?;
        if keys.keys.is_empty() {
            return Err(invalid_response());
        }
        Ok(keys)
    }

    /// Overrides identity endpoints only for validated loopback HTTP tests.
    #[cfg(feature = "test-endpoints")]
    pub fn with_oauth_base(mut self, base: &str) -> GoogleResult<Self> {
        let validated = Self::with_base_urls(base, base)?;
        self.oauth_base = Some(validated.drive_base);
        Ok(self)
    }

    fn identity_endpoint(&self, kind: &str) -> Url {
        #[cfg(feature = "test-endpoints")]
        if let Some(base) = &self.oauth_base {
            let path = match kind {
                "token" => "/token",
                "revoke" => "/revoke",
                "userinfo" => "/v1/userinfo",
                "discovery" => "/.well-known/openid-configuration",
                _ => "/oauth2/v3/certs",
            };
            let mut url = base.clone();
            url.set_path(path);
            return url;
        }
        Url::parse(match kind {
            "token" => TOKEN,
            "revoke" => REVOKE,
            "userinfo" => USERINFO,
            "discovery" => DISCOVERY,
            _ => KEYS,
        })
        .expect("constant identity URL is valid")
    }

    async fn oauth_call(&self, kind: &str, form: &[(&str, &str)]) -> GoogleResult<Vec<u8>> {
        let http = self.http.as_ref().map_err(Clone::clone)?;
        let url = self.identity_endpoint(kind);
        let request = if form.is_empty() {
            http.get(url)
        } else {
            http.post(url).form(form)
        };
        let response = request
            .timeout(self.limits.request_timeout)
            .send()
            .await
            .map_err(super::client::transport_failure)?;
        self.response_body(response, 200).await
    }
}

fn token_response(body: &[u8]) -> GoogleResult<GoogleTokenResponse> {
    let response: GoogleTokenResponse = decode(body)?;
    if response.access_token.is_empty()
        || !response.token_type.eq_ignore_ascii_case("Bearer")
        || response.expires_in == 0
    {
        return Err(invalid_response());
    }
    Ok(response)
}
fn invalid_response() -> GoogleApiFailure {
    GoogleApiFailure::new(GoogleApiError::InvalidResponse, None)
}
fn redirect(raw: &str) -> GoogleResult<Url> {
    let url = Url::parse(raw).map_err(|_| invalid_argument())?;
    let host = url.host_str().unwrap_or("");
    let local = host == "localhost"
        || host
            .trim_matches(['[', ']'])
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback());
    if url.scheme() != "http"
        || !local
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid_argument());
    }
    Ok(url)
}
fn validate_redirect(config: &GoogleOAuthConfig, raw: &str) -> GoogleResult<()> {
    let url = redirect(raw)?;
    if !config.redirect_uris.iter().any(|candidate| {
        redirect(candidate).is_ok_and(|candidate| {
            candidate.host_str() == url.host_str() && candidate.path() == url.path()
        })
    }) {
        return Err(invalid_argument());
    }
    Ok(())
}
