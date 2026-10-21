use super::*;
use oauth2::PkceCodeVerifier;
use std::{collections::HashMap, io::Write, net::TcpStream, sync::atomic::AtomicUsize};

#[derive(Default)]
struct MemoryVault {
    entries: Mutex<HashMap<(String, AccountId), String>>,
    fail: AtomicBool,
    fail_save: AtomicBool,
}
impl CredentialStore for MemoryVault {
    fn load(&self, client: &str, id: &AccountId) -> Result<Option<Credential>, AuthError> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(AuthError::SecureStorage);
        }
        Ok(self
            .entries
            .lock()
            .unwrap()
            .get(&(client.into(), id.clone()))
            .map(|s| serde_json::from_str(s).unwrap()))
    }
    fn save(&self, client: &str, id: &AccountId, value: &Credential) -> Result<(), AuthError> {
        if self.fail.load(Ordering::SeqCst) || self.fail_save.load(Ordering::SeqCst) {
            return Err(AuthError::SecureStorage);
        }
        self.entries.lock().unwrap().insert(
            (client.into(), id.clone()),
            serde_json::to_string(value).unwrap(),
        );
        Ok(())
    }
    fn delete(&self, client: &str, id: &AccountId) -> Result<(), AuthError> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(AuthError::SecureStorage);
        }
        self.entries
            .lock()
            .unwrap()
            .remove(&(client.into(), id.clone()));
        Ok(())
    }
}
#[derive(Default)]
struct FakeGoogle {
    refreshes: AtomicUsize,
    failure: Mutex<Option<AuthError>>,
    mismatch: AtomicBool,
}
impl GoogleApi for FakeGoogle {
    fn exchange(
        &self,
        _: &OAuthConfig,
        code: &str,
        _: &str,
        verifier: &str,
    ) -> Result<Tokens, AuthError> {
        if let Some(error) = *self.failure.lock().unwrap() {
            return Err(error);
        }
        assert!(verifier.len() >= 43);
        if code != "valid-code" {
            return Err(AuthError::InvalidCode);
        }
        Ok(tokens("a"))
    }
    fn refresh(&self, _: &OAuthConfig, refresh: &str) -> Result<Tokens, AuthError> {
        self.refreshes.fetch_add(1, Ordering::SeqCst);
        if let Some(error) = *self.failure.lock().unwrap() {
            return Err(error);
        }
        let subject = refresh.strip_prefix("refresh-").unwrap();
        let mut tokens = tokens(subject);
        tokens.refresh_token = None; // Google need not rotate a refresh token.
        Ok(tokens)
    }
    fn identify(&self, access: &str) -> Result<Identity, AuthError> {
        let sub = if self.mismatch.load(Ordering::SeqCst) {
            "other"
        } else {
            access.strip_prefix("access-").unwrap()
        };
        Ok(Identity {
            sub: sub.into(),
            email: Some(format!("{sub}@example.com")),
            email_verified: true,
        })
    }
}
fn config() -> OAuthConfig {
    OAuthConfig {
        client_id: "test.apps.googleusercontent.com".into(),
        client_secret: None,
    }
}
fn tokens(subject: &str) -> Tokens {
    Tokens {
        access_token: format!("access-{subject}"),
        refresh_token: Some(format!("refresh-{subject}")),
        expires_in: 3600,
        scope: Some(format!("openid email {}", google::DRIVE_SCOPE)),
    }
}
fn provider() -> (AuthProvider, Arc<MemoryVault>, Arc<FakeGoogle>) {
    let store = Arc::new(MemoryVault::default());
    let api = Arc::new(FakeGoogle::default());
    (
        AuthProvider {
            api: api.clone(),
            store: store.clone(),
            locks: Mutex::new(HashMap::new()),
            authorizing: AtomicBool::new(false),
        },
        store,
        api,
    )
}
fn request(authority: &str, query: &str) -> String {
    format!("GET /oauth/callback?{query} HTTP/1.1\r\nHost: {authority}\r\n\r\n")
}
fn browser_response(url: &str, query: &str) -> Result<(), AuthError> {
    let url = url::Url::parse(url).unwrap();
    let params: HashMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(params["code_challenge_method"], "S256");
    assert_eq!(params["access_type"], "offline");
    assert!(params["scope"].contains(google::DRIVE_SCOPE));
    let redirect = url::Url::parse(&params["redirect_uri"]).unwrap();
    let authority = format!(
        "{}:{}",
        redirect.host_str().unwrap(),
        redirect.port().unwrap()
    );
    let request = request(&authority, &format!("state={}&{query}", params["state"]));
    TcpStream::connect(authority)
        .unwrap()
        .write_all(request.as_bytes())
        .unwrap();
    Ok(())
}
fn expire(store: &MemoryVault, id: &AccountId) {
    let mut credential = store.load(&config().client_id, id).unwrap().unwrap();
    credential.expires_at = 0;
    store.save(&config().client_id, id, &credential).unwrap();
}

#[test]
fn pkce_matches_rfc7636_vector_and_random_values_are_distinct() {
    let verifier = PkceCodeVerifier::new("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into());
    assert_eq!(
        PkceCodeChallenge::from_code_verifier_sha256(&verifier).as_str(),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
    let (challenge_a, verifier_a) = PkceCodeChallenge::new_random_sha256();
    let (challenge_b, verifier_b) = PkceCodeChallenge::new_random_sha256();
    assert_ne!(challenge_a.as_str(), challenge_b.as_str());
    assert_ne!(verifier_a.secret(), verifier_b.secret());
    assert!((43..=128).contains(&verifier_a.secret().len()));
    assert_ne!(
        CsrfToken::new_random().secret(),
        CsrfToken::new_random().secret()
    );
}
#[test]
fn callback_checks_state_host_path_method_and_duplicates() {
    let state = CsrfToken::new("expected".into());
    let authority = "127.0.0.1:1234";
    let valid = request(authority, "state=expected&code=real-code");
    assert_eq!(
        callback::parse_callback(&valid, authority, &state).unwrap(),
        "real-code"
    );
    for invalid in [
        valid.replace("expected", "forged"),
        valid.replace("GET ", "POST "),
        valid.replace("/oauth/callback", "/other"),
        valid.replace("Host: 127.0.0.1", "Host: attacker.invalid"),
        request(authority, "state=expected&state=expected&code=a"),
        request(authority, "code=a"),
        request(authority, "state=expected&code="),
        request(authority, "state=expected&code=a&code=b"),
        request(authority, "state=expected&code=a&error=access_denied"),
    ] {
        assert_eq!(
            callback::parse_callback(&invalid, authority, &state),
            Err(AuthError::InvalidCallback)
        );
    }
    assert_eq!(
        callback::parse_callback(
            &request(authority, "state=expected&error=access_denied"),
            authority,
            &state
        ),
        Err(AuthError::Denied)
    );
    assert_eq!(
        callback::parse_callback(
            &request(authority, "state=expected&error=server_error"),
            authority,
            &state
        ),
        Err(AuthError::ProviderUnavailable)
    );
}
#[test]
fn valid_browser_flow_exchanges_code_identifies_and_persists_before_success() {
    let (provider, store, _) = provider();
    let account = provider
        .authorize_config(&config(), &Cancellation::new(), |url| {
            browser_response(url, "code=valid-code")
        })
        .unwrap();
    assert_eq!(account.provider_subject, "a");
    assert_eq!(account.email.as_deref(), Some("a@example.com"));
    let saved = store
        .load(&config().client_id, &account.id)
        .unwrap()
        .unwrap();
    assert_eq!(saved.subject, "a");
    assert_eq!(saved.refresh_token, "refresh-a");
    assert!(saved.expires_at > now().unwrap());
}
#[test]
fn denied_invalid_code_and_failed_exchange_do_not_persist() {
    for (query, injected, expected) in [
        ("error=access_denied", None, AuthError::Denied),
        ("code=bad-code", None, AuthError::InvalidCode),
        (
            "code=valid-code",
            Some(AuthError::Network),
            AuthError::Network,
        ),
        (
            "code=valid-code",
            Some(AuthError::ProviderUnavailable),
            AuthError::ProviderUnavailable,
        ),
    ] {
        let (provider, store, api) = provider();
        *api.failure.lock().unwrap() = injected;
        assert_eq!(
            provider.authorize_config(&config(), &Cancellation::new(), |url| browser_response(
                url, query
            )),
            Err(expected)
        );
        assert!(store.entries.lock().unwrap().is_empty());
    }
}
#[test]
fn cancellation_browser_failure_and_callback_expiry_are_explicit() {
    let (provider, store, _) = provider();
    let cancel = Cancellation::new();
    cancel.cancel();
    assert_eq!(
        provider.authorize_config(&config(), &cancel, |_| panic!("must not open browser")),
        Err(AuthError::Cancelled)
    );
    assert_eq!(
        provider.authorize_config(&config(), &Cancellation::new(), |_| Err(
            AuthError::BrowserUnavailable
        )),
        Err(AuthError::BrowserUnavailable)
    );
    let listener = callback::Listener::bind().unwrap();
    assert_eq!(
        listener.receive(
            &CsrfToken::new_random(),
            &Cancellation::new(),
            Duration::ZERO
        ),
        Err(AuthError::TimedOut)
    );
    let listener = callback::Listener::bind().unwrap();
    assert_eq!(
        listener.receive(&CsrfToken::new_random(), &cancel, Duration::from_secs(1)),
        Err(AuthError::Cancelled)
    );
    assert!(store.entries.lock().unwrap().is_empty());
}
#[test]
fn forged_request_does_not_consume_the_valid_callback() {
    let listener = callback::Listener::bind().unwrap();
    let address = url::Url::parse(&listener.redirect_uri()).unwrap();
    let authority = format!("127.0.0.1:{}", address.port().unwrap());
    for state in ["forged", "correct"] {
        TcpStream::connect(&authority)
            .unwrap()
            .write_all(request(&authority, &format!("state={state}&code=valid")).as_bytes())
            .unwrap();
    }
    assert_eq!(
        listener
            .receive(
                &CsrfToken::new("correct".into()),
                &Cancellation::new(),
                Duration::from_secs(2)
            )
            .unwrap(),
        "valid"
    );
}
#[test]
fn token_responses_are_typed_without_exposing_provider_details() {
    let valid = br#"{"access_token":"secret-access","token_type":"Bearer","expires_in":3600,"refresh_token":"secret-refresh"}"#;
    let token = google::decode_tokens(200, valid, false).unwrap();
    assert_eq!(token.access_token, "secret-access");
    assert_eq!(
        google::decode_tokens(
            400,
            br#"{"error":"invalid_grant","error_description":"secret-code"}"#,
            false
        )
        .err(),
        Some(AuthError::InvalidCode)
    );
    assert_eq!(
        google::decode_tokens(400, br#"{"error":"invalid_grant"}"#, true).err(),
        Some(AuthError::ReauthorizationRequired)
    );
    assert_eq!(
        google::decode_tokens(503, b"upstream", false).err(),
        Some(AuthError::ProviderUnavailable)
    );
    assert_eq!(
        google::decode_tokens(200, b"{}", false).err(),
        Some(AuthError::InvalidResponse)
    );
    assert!(!AuthError::InvalidCode.to_string().contains("secret"));
    let mut missing = tokens("a");
    missing.scope = Some("openid email".into());
    assert_eq!(
        validate_tokens(&missing),
        Err(AuthError::MissingPermissions)
    );
}
#[test]
fn expired_credentials_renew_once_under_concurrent_requests_and_keep_refresh() {
    let (provider, store, api) = provider();
    let account = provider
        .complete(&config(), tokens("a"), &Cancellation::new())
        .unwrap();
    expire(&store, &account.id);
    let provider = Arc::new(provider);
    let workers: Vec<_> = (0..6)
        .map(|_| {
            let provider = provider.clone();
            let id = account.id.clone();
            std::thread::spawn(move || provider.verify_with_config(&config(), &id).unwrap())
        })
        .collect();
    for worker in workers {
        assert_eq!(worker.join().unwrap().id, account.id);
    }
    assert_eq!(api.refreshes.load(Ordering::SeqCst), 1);
    assert_eq!(
        store
            .load(&config().client_id, &account.id)
            .unwrap()
            .unwrap()
            .refresh_token,
        "refresh-a"
    );
}
#[test]
fn restart_reuses_credentials_and_failure_on_a_does_not_change_b() {
    let (provider, store, api) = provider();
    let a = provider
        .complete(&config(), tokens("a"), &Cancellation::new())
        .unwrap();
    let b = provider
        .complete(&config(), tokens("b"), &Cancellation::new())
        .unwrap();
    expire(&store, &a.id);
    let restarted = AuthProvider {
        store: store.clone(),
        api: api.clone(),
        locks: Mutex::new(HashMap::new()),
        authorizing: AtomicBool::new(false),
    };
    for error in [AuthError::Network, AuthError::ReauthorizationRequired] {
        *api.failure.lock().unwrap() = Some(error);
        assert_eq!(restarted.verify_with_config(&config(), &a.id), Err(error));
        assert_eq!(
            restarted.verify_with_config(&config(), &b.id).unwrap().id,
            b.id
        );
        assert_eq!(
            store
                .load(&config().client_id, &a.id)
                .unwrap()
                .unwrap()
                .refresh_token,
            "refresh-a"
        );
    }
    let mut other_client = config();
    other_client.client_id = "other.apps.googleusercontent.com".into();
    assert_eq!(
        restarted.verify_with_config(&other_client, &b.id),
        Err(AuthError::ReauthorizationRequired)
    );
}
#[test]
fn identity_mismatch_and_storage_failure_never_report_authenticated() {
    let (provider, store, api) = provider();
    let account = provider
        .complete(&config(), tokens("a"), &Cancellation::new())
        .unwrap();
    api.mismatch.store(true, Ordering::SeqCst);
    assert_eq!(
        provider.verify_with_config(&config(), &account.id),
        Err(AuthError::IdentityMismatch)
    );
    store.fail.store(true, Ordering::SeqCst);
    assert_eq!(
        provider.complete(&config(), tokens("b"), &Cancellation::new()),
        Err(AuthError::SecureStorage)
    );
}
#[test]
fn email_is_optional_and_unverified_email_is_not_presented() {
    let identity = Identity {
        sub: "stable-subject".into(),
        email: Some("unverified@example.com".into()),
        email_verified: false,
    };
    assert_eq!(identity.summary().unwrap().email, None);
    let identity = Identity {
        sub: "stable-subject".into(),
        email: None,
        email_verified: true,
    };
    assert_eq!(identity.summary().unwrap().id.0, "google:stable-subject");
    assert_eq!(
        Identity {
            sub: "".into(),
            email: None,
            email_verified: false
        }
        .summary(),
        Err(AuthError::InvalidResponse)
    );
}
#[test]
fn missing_refresh_token_cancellation_and_busy_never_publish_success() {
    let (provider, store, _) = provider();
    let mut response = tokens("a");
    response.refresh_token = None;
    assert_eq!(
        provider.complete(&config(), response, &Cancellation::new()),
        Err(AuthError::ReauthorizationRequired)
    );
    let cancel = Cancellation::new();
    cancel.cancel();
    assert_eq!(
        provider.complete(&config(), tokens("a"), &cancel),
        Err(AuthError::Cancelled)
    );
    provider.authorizing.store(true, Ordering::SeqCst);
    assert_eq!(
        provider.authorize(&config(), &Cancellation::new(), |_| panic!("already busy")),
        Err(AuthError::Busy)
    );
    assert!(store.entries.lock().unwrap().is_empty());
}

#[test]
fn vault_save_failure_after_google_success_never_commits_credentials() {
    let (provider, store, _) = provider();
    store.fail_save.store(true, Ordering::SeqCst);
    assert_eq!(
        provider.complete(&config(), tokens("a"), &Cancellation::new()),
        Err(AuthError::SecureStorage)
    );
    assert!(store.entries.lock().unwrap().is_empty());
}

#[test]
fn reconnecting_without_rotation_keeps_the_same_accounts_refresh_token() {
    let (provider, store, _) = provider();
    let account = provider
        .complete(&config(), tokens("a"), &Cancellation::new())
        .unwrap();
    let mut response = tokens("a");
    response.refresh_token = None;
    assert_eq!(
        provider
            .complete(&config(), response, &Cancellation::new())
            .unwrap()
            .id,
        account.id
    );
    assert_eq!(
        store
            .load(&config().client_id, &account.id)
            .unwrap()
            .unwrap()
            .refresh_token,
        "refresh-a"
    );
}

#[test]
fn renewal_identity_mismatch_preserves_the_previous_vault_entry() {
    let (provider, store, api) = provider();
    let account = provider
        .complete(&config(), tokens("a"), &Cancellation::new())
        .unwrap();
    expire(&store, &account.id);
    api.mismatch.store(true, Ordering::SeqCst);
    assert_eq!(
        provider.verify_with_config(&config(), &account.id),
        Err(AuthError::IdentityMismatch)
    );
    assert_eq!(
        store
            .load(&config().client_id, &account.id)
            .unwrap()
            .unwrap()
            .expires_at,
        0
    );
}

#[test]
fn remote_access_port_renews_and_never_runs_after_vault_failure() {
    let (provider, store, api) = provider();
    let account = provider
        .complete(&config(), tokens("a"), &Cancellation::new())
        .unwrap();
    expire(&store, &account.id);
    let value = provider
        .with_access_config(&config(), &account.id, |token| {
            assert_eq!(token, "access-a");
            Ok(42)
        })
        .unwrap();
    assert_eq!(value, 42);
    assert_eq!(api.refreshes.load(Ordering::SeqCst), 1);
    store.fail.store(true, Ordering::SeqCst);
    let result: Result<(), crate::EngineError> =
        provider.with_access_config(&config(), &account.id, |_| {
            panic!("vault failure must stop request")
        });
    assert_eq!(
        result,
        Err(crate::EngineError::Authentication(AuthError::SecureStorage))
    );
}

#[test]
fn logout_is_local_idempotent_and_isolated_by_account_and_client() {
    let (provider, store, api) = provider();
    let a = provider
        .complete(&config(), tokens("a"), &Cancellation::new())
        .unwrap();
    let b = provider
        .complete(&config(), tokens("b"), &Cancellation::new())
        .unwrap();
    let mut other = config();
    other.client_id = "other.apps.googleusercontent.com".into();
    provider
        .complete(&other, tokens("a"), &Cancellation::new())
        .unwrap();
    provider.sign_out(&config(), &a.id).unwrap();
    provider.sign_out(&config(), &a.id).unwrap();
    assert!(store.load(&config().client_id, &a.id).unwrap().is_none());
    assert!(store.load(&config().client_id, &b.id).unwrap().is_some());
    assert!(store.load(&other.client_id, &a.id).unwrap().is_some());
    let restarted = AuthProvider {
        store,
        api,
        ..AuthProvider::default()
    };
    assert_eq!(
        restarted.verify_with_config(&config(), &a.id),
        Err(AuthError::ReauthorizationRequired)
    );
    assert!(restarted.verify_with_config(&config(), &b.id).is_ok());
}
#[test]
fn logout_reports_vault_failure_and_refuses_pending_authorization() {
    let (provider, store, _) = provider();
    let account = provider
        .complete(&config(), tokens("a"), &Cancellation::new())
        .unwrap();
    provider.authorizing.store(true, Ordering::SeqCst);
    assert_eq!(
        provider.sign_out(&config(), &account.id),
        Err(AuthError::Busy)
    );
    provider.authorizing.store(false, Ordering::SeqCst);
    store.fail.store(true, Ordering::SeqCst);
    assert_eq!(
        provider.sign_out(&config(), &account.id),
        Err(AuthError::SecureStorage)
    );
    store.fail.store(false, Ordering::SeqCst);
    assert!(store
        .load(&config().client_id, &account.id)
        .unwrap()
        .is_some());
    provider.sign_out(&config(), &account.id).unwrap();
}
#[test]
fn logout_waits_for_in_flight_account_work_before_deleting_credentials() {
    let (provider, store, _) = provider();
    let account = provider
        .complete(&config(), tokens("a"), &Cancellation::new())
        .unwrap();
    let provider = Arc::new(provider);
    let lock = provider.account_lock(&account.id).unwrap();
    let guard = lock.lock().unwrap();
    let (send, receive) = std::sync::mpsc::channel();
    let worker = provider.clone();
    let id = account.id.clone();
    let thread = std::thread::spawn(move || {
        send.send(worker.sign_out(&config(), &id)).unwrap();
    });
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !provider.authorizing.load(Ordering::Acquire) {
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(receive.try_recv().is_err());
    assert!(store
        .load(&config().client_id, &account.id)
        .unwrap()
        .is_some());
    drop(guard);
    assert_eq!(
        receive.recv_timeout(Duration::from_secs(5)).unwrap(),
        Ok(())
    );
    thread.join().unwrap();
    assert!(store
        .load(&config().client_id, &account.id)
        .unwrap()
        .is_none());
}
