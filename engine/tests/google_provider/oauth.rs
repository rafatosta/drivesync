//! Desktop OAuth/OIDC contracts. The committed RSA key is a public test fixture,
//! never an application credential or a real Google signing key.
use crate::google_provider::support::{session, Reply, Server};
use drivesync_engine::providers::google::*;
use std::time::{SystemTime, UNIX_EPOCH};

const DISCOVERY: &str = r#"{"issuer":"https://accounts.google.com","authorization_endpoint":"https://accounts.google.com/o/oauth2/v2/auth","token_endpoint":"https://oauth2.googleapis.com/token","userinfo_endpoint":"https://openidconnect.googleapis.com/v1/userinfo","jwks_uri":"https://www.googleapis.com/oauth2/v3/certs"}"#;
const KEYS: &str = include_str!("fixtures/oidc-test-jwks.json");

fn config() -> GoogleOAuthConfig {
    GoogleOAuthConfig {
        client_id: "client-id".into(),
        client_secret: Some("client-secret".into()),
        redirect_uris: vec!["http://127.0.0.1/callback".into()],
        scopes: vec!["openid".into(), "email".into()],
    }
}
fn auth_request() -> GoogleAuthorizationRequest {
    GoogleAuthorizationRequest {
        redirect_uri: "http://127.0.0.1:43123/callback".into(),
        state: "random-state".into(),
        code_challenge: "a".repeat(43),
        code_challenge_method: "S256".into(),
        scopes: vec!["openid".into()],
        ..Default::default()
    }
}
fn client(server: &Server) -> GoogleDriveClient {
    server.client().with_oauth_base(&server.base).unwrap()
}
fn form(body: &[u8]) -> std::collections::BTreeMap<String, String> {
    let url = reqwest::Url::parse(&format!(
        "http://localhost/?{}",
        std::str::from_utf8(body).unwrap()
    ))
    .unwrap();
    url.query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}
fn validation() -> GoogleIdTokenValidation {
    GoogleIdTokenValidation {
        issuer: "https://accounts.google.com".into(),
        audience: "client-id".into(),
        nonce: Some("nonce".into()),
        clock_skew_seconds: Some(0),
    }
}
fn claims() -> serde_json::Value {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    serde_json::json!({"iss":"https://accounts.google.com","aud":"client-id","sub":"stable-sub","exp":now+3600,"iat":now,"nonce":"nonce"})
}
fn sign(claims: &serde_json::Value) -> String {
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some("test-key".into());
    jsonwebtoken::encode(
        &header,
        claims,
        &jsonwebtoken::EncodingKey::from_rsa_pem(include_bytes!("fixtures/oidc-test-private.pem"))
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn authorization_url_contains_pkce_state_and_explicit_scopes() {
    let result = GoogleDriveClient::new()
        .authorization_request(&config(), auth_request())
        .unwrap();
    let url = reqwest::Url::parse(&result.url).unwrap();
    assert_eq!(url.host_str(), Some("accounts.google.com"));
    assert_eq!(url.path(), "/o/oauth2/v2/auth");
    let q: std::collections::BTreeMap<_, _> = url.query_pairs().collect();
    for (k, v) in [
        ("response_type", "code"),
        ("client_id", "client-id"),
        ("scope", "openid"),
        ("state", "random-state"),
        ("code_challenge_method", "S256"),
        ("redirect_uri", "http://127.0.0.1:43123/callback"),
    ] {
        assert_eq!(q.get(k).unwrap(), v);
    }
    assert_eq!(q.get("code_challenge").unwrap(), &"a".repeat(43));
    assert!(!result.url.contains("client-secret"));
    assert!(!format!("{result:?}").contains("random-state"));
}

#[test]
fn invalid_authorization_inputs_are_rejected_without_network() {
    for field in ["state", "pkce", "method", "redirect", "scope"] {
        let mut request = auth_request();
        match field {
            "state" => request.state.clear(),
            "pkce" => request.code_challenge = "short".into(),
            "method" => request.code_challenge_method = "plain".into(),
            "redirect" => request.redirect_uri = "https://attacker.test/callback".into(),
            _ => request.scopes = vec!["unapproved".into()],
        };
        assert_eq!(
            GoogleDriveClient::new()
                .authorization_request(&config(), request)
                .unwrap_err()
                .kind,
            GoogleApiError::InvalidArgument
        );
    }
}

#[tokio::test]
async fn config_sources_have_explicit_precedence_and_redact_secrets() {
    let path = std::env::temp_dir().join(format!("drivesync-oauth-{}.json", std::process::id()));
    std::fs::write(&path,r#"{"installed":{"client_id":"file-id","client_secret":"file-secret","redirect_uris":["http://127.0.0.1/callback"]}}"#).unwrap();
    let result = GoogleDriveClient::new()
        .load_google_config(GoogleConfigSources {
            client_id_env: Some("override-id".into()),
            oauth_file_env: Some(path.to_str().unwrap().into()),
            ..Default::default()
        })
        .await
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(result.client_id, "override-id");
    assert_eq!(result.client_secret.as_deref(), Some("file-secret"));
    assert_eq!(result.redirect_uris, vec!["http://127.0.0.1/callback"]);
    assert!(!format!("{result:?}").contains("file-secret"));
}

#[tokio::test]
async fn explicit_bad_config_does_not_fall_back_or_expose_paths() {
    let err = GoogleDriveClient::new()
        .load_google_config(GoogleConfigSources {
            oauth_file_env: Some("/private/nonexistent-config".into()),
            client_id_env: Some("override".into()),
            ..Default::default()
        })
        .await
        .unwrap_err();
    assert_eq!(err.kind, GoogleApiError::InvalidArgument);
    assert!(!format!("{err:?}").contains("private"));
}

#[tokio::test]
async fn code_exchange_uses_a_form_body_and_no_secret_query() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"access_token":"access-secret","token_type":"Bearer","expires_in":3600,"refresh_token":"refresh-secret"}"#,
    )]);
    let result = client(&server)
        .exchange_authorization_code(
            &config(),
            "auth-code",
            "http://127.0.0.1:43123/callback",
            &"v".repeat(43),
        )
        .await
        .unwrap();
    assert_eq!(result.access_token, "access-secret");
    assert_eq!(result.refresh_token.as_deref(), Some("refresh-secret"));
    let requests = server.recorded();
    let req = &requests[0];
    let body = form(&req.body);
    assert_eq!(req.method, "POST");
    assert_eq!(req.target, "/token");
    assert_eq!(
        req.header("content-type"),
        Some("application/x-www-form-urlencoded")
    );
    for (k, v) in [
        ("grant_type", "authorization_code"),
        ("code", "auth-code"),
        ("client_id", "client-id"),
        ("client_secret", "client-secret"),
        ("redirect_uri", "http://127.0.0.1:43123/callback"),
    ] {
        assert_eq!(body.get(k).unwrap(), v);
    }
    assert_eq!(body.get("code_verifier"), Some(&"v".repeat(43)));
    assert!(!format!("{result:?}").contains("secret"));
}

#[tokio::test]
async fn refresh_does_not_invent_a_refresh_token() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"access_token":"new","token_type":"Bearer","expires_in":3600}"#,
    )]);
    let result = client(&server)
        .refresh_access_token(&config(), "old-refresh")
        .await
        .unwrap();
    assert_eq!(result.refresh_token, None);
    let requests = server.recorded();
    let body = form(&requests[0].body);
    assert_eq!(body.get("grant_type").unwrap(), "refresh_token");
    assert_eq!(body.get("refresh_token").unwrap(), "old-refresh");
    assert!(requests[0].decoded_query().is_empty());
}

#[tokio::test]
async fn invalid_grant_is_sanitized_and_not_retryable() {
    let server = Server::start(vec![Reply::json(
        400,
        r#"{"error":"invalid_grant","error_description":"private secret"}"#,
    )]);
    let err = client(&server)
        .refresh_access_token(&config(), "secret-refresh")
        .await
        .unwrap_err();
    assert_eq!(err.kind, GoogleApiError::InvalidGrant);
    assert!(!err.retryable);
    assert_eq!(err.http_status, Some(400));
    assert!(!format!("{err:?}").contains("secret"));
}

#[tokio::test]
async fn revocation_sends_token_in_form_and_accepts_empty_success() {
    let server = Server::start(vec![Reply::json(200, "")]);
    client(&server)
        .revoke_token("revocation-secret")
        .await
        .unwrap();
    let requests = server.recorded();
    assert_eq!(requests[0].target, "/revoke");
    assert_eq!(
        form(&requests[0].body).get("token").unwrap(),
        "revocation-secret"
    );
    assert!(requests[0].header("authorization").is_none());
}

#[tokio::test]
async fn userinfo_requires_stable_sub_and_uses_session_authorization() {
    for (body, valid) in [
        (
            r#"{"sub":"stable","email":"u@example.test","email_verified":true}"#,
            true,
        ),
        (r#"{"email":"u@example.test"}"#, false),
        (r#"{"sub":""}"#, false),
    ] {
        let server = Server::start(vec![Reply::json(200, body)]);
        let result = client(&server).get_userinfo(&session()).await;
        if valid {
            assert_eq!(result.unwrap().sub, "stable");
        } else {
            assert_eq!(result.unwrap_err().kind, GoogleApiError::InvalidResponse);
        }
        let requests = server.recorded();
        assert_eq!(requests[0].path(), "/v1/userinfo");
        assert_eq!(
            requests[0].header("authorization"),
            Some("Bearer tok-A-secret")
        );
    }
}

#[tokio::test]
async fn discovery_and_keys_use_fixed_google_identity_endpoints() {
    let server = Server::start(vec![
        Reply::json(200, DISCOVERY),
        Reply::json(200, DISCOVERY),
        Reply::json(200, KEYS),
    ]);
    let client = client(&server);
    assert_eq!(
        client.get_oidc_discovery().await.unwrap().issuer,
        "https://accounts.google.com"
    );
    assert_eq!(
        client.get_oidc_keys().await.unwrap().keys[0].kid.as_deref(),
        Some("test-key")
    );
    let reqs = server.recorded();
    assert_eq!(reqs[0].path(), "/.well-known/openid-configuration");
    assert_eq!(reqs[2].path(), "/oauth2/v3/certs");
    assert!(reqs.iter().all(|r| r.header("authorization").is_none()));
}

#[tokio::test]
async fn hostile_discovery_key_url_is_not_followed() {
    let body = DISCOVERY.replace(
        "https://www.googleapis.com/oauth2/v3/certs",
        "http://attacker.test/keys",
    );
    let server = Server::start(vec![Reply::json(200, &body)]);
    assert_eq!(
        client(&server).get_oidc_keys().await.unwrap_err().kind,
        GoogleApiError::InvalidResponse
    );
    assert_eq!(server.recorded().len(), 1);
}

#[tokio::test]
async fn signed_id_token_is_verified_with_google_keys() {
    let server = Server::start(vec![Reply::json(200, DISCOVERY), Reply::json(200, KEYS)]);
    let claims = client(&server)
        .validate_id_token(&sign(&claims()), validation())
        .await
        .unwrap();
    assert_eq!(claims.sub, "stable-sub");
    assert_eq!(claims.aud, GoogleAudience::One("client-id".into()));
}

#[tokio::test]
async fn incorrect_claims_and_signatures_are_rejected() {
    for field in [
        "iss",
        "aud",
        "exp",
        "iat",
        "nonce",
        "sub",
        "signature",
        "algorithm",
        "kid",
        "azp",
    ] {
        let mut value = claims();
        match field {
            "exp" => value["exp"] = serde_json::json!(1),
            "iat" => value["iat"] = serde_json::json!(u64::MAX),
            "sub" => value["sub"] = serde_json::json!(""),
            "azp" => value["azp"] = serde_json::json!("other"),
            "iss" | "aud" | "nonce" => value[field] = serde_json::json!("wrong"),
            _ => {}
        }
        let token = if field == "algorithm" {
            jsonwebtoken::encode(
                &jsonwebtoken::Header::default(),
                &value,
                &jsonwebtoken::EncodingKey::from_secret(b"test"),
            )
            .unwrap()
        } else if field == "kid" {
            let mut h = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
            h.kid = Some("unknown".into());
            jsonwebtoken::encode(
                &h,
                &value,
                &jsonwebtoken::EncodingKey::from_rsa_pem(include_bytes!(
                    "fixtures/oidc-test-private.pem"
                ))
                .unwrap(),
            )
            .unwrap()
        } else {
            let mut token = sign(&value);
            if field == "signature" {
                let start = token.rfind('.').unwrap() + 1;
                token.replace_range(
                    start..start + 1,
                    if &token[start..start + 1] == "A" {
                        "B"
                    } else {
                        "A"
                    },
                );
            }
            token
        };
        let server = Server::start(vec![Reply::json(200, DISCOVERY), Reply::json(200, KEYS)]);
        let err = client(&server)
            .validate_id_token(&token, validation())
            .await
            .unwrap_err();
        assert_eq!(err.kind, GoogleApiError::AuthenticationRequired, "{field}");
        assert!(!format!("{err:?}").contains(&token));
    }
}

#[tokio::test]
async fn multiple_audiences_require_matching_authorized_party() {
    for azp in [None, Some("wrong"), Some("client-id")] {
        let mut value = claims();
        value["aud"] = serde_json::json!(["client-id", "other"]);
        if let Some(azp) = azp {
            value["azp"] = serde_json::json!(azp);
        }
        let server = Server::start(vec![Reply::json(200, DISCOVERY), Reply::json(200, KEYS)]);
        let result = client(&server)
            .validate_id_token(&sign(&value), validation())
            .await;
        assert_eq!(result.is_ok(), azp == Some("client-id"));
    }
}

#[tokio::test]
async fn malformed_token_responses_are_not_success() {
    for body in [
        "[]",
        "not json",
        r#"{"access_token":"","token_type":"Bearer","expires_in":3600}"#,
        r#"{"access_token":"secret","token_type":"Basic","expires_in":3600}"#,
    ] {
        let server = Server::start(vec![Reply::json(200, body)]);
        assert_eq!(
            client(&server)
                .refresh_access_token(&config(), "refresh")
                .await
                .unwrap_err()
                .kind,
            GoogleApiError::InvalidResponse
        );
    }
}
