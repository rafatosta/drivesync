//! Public boundary checks; no desktop credentials or real Google service.
use super::*;
#[tokio::test]
async fn open_without_oauth_preserves_local_use() {
    let directory = std::env::temp_dir().join(format!(
        "connection-empty-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let engine = Engine::open(EngineConfig {
        data_directory: directory.clone(),
        application_id: "test.connection".into(),
        google_oauth_file: None,
    })
    .await
    .unwrap();
    assert!(engine
        .list_accounts(ListAccountsRequest { provider: None })
        .await
        .unwrap()
        .accounts
        .is_empty());
    assert_eq!(
        engine
            .begin_google_sign_in(BeginGoogleSignInRequest { account_id: None })
            .await
            .unwrap_err()
            .kind,
        EngineErrorKind::AuthorizationUnavailable
    );
    drop(engine);
    std::fs::remove_dir_all(directory).unwrap();
}

use crate::accounts::tests::http_support as http;
use http::{Reply, Server};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
struct Fixture {
    directory: PathBuf,
    engine: Engine,
    server: Server,
}
impl Fixture {
    async fn new(replies: Vec<Reply>) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "connection-flow-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("oauth.json");
        std::fs::write(&file, r#"{"installed":{"client_id":"synthetic-client","redirect_uris":["http://127.0.0.1/"]}}"#).unwrap();
        let server = Server::start(replies);
        let client = GoogleDriveClient::with_base_urls(
            format!("{}/drive/v3", server.base),
            format!("{}/upload/drive/v3", server.base),
        )
        .unwrap()
        .with_oauth_base(&server.base)
        .unwrap();
        let mut engine = Engine::open_with_client(
            EngineConfig {
                data_directory: directory.join("accounts"),
                application_id: "test.connection".into(),
                google_oauth_file: Some(file),
            },
            Some(client),
        )
        .await
        .unwrap();
        crate::accounts::tests::use_isolated_vault(
            &mut Arc::get_mut(&mut engine.inner).unwrap().accounts,
        );
        Self {
            directory,
            engine,
            server,
        }
    }
    async fn begin(&self, account_id: Option<AccountId>) -> BeginGoogleSignInResult {
        self.engine
            .begin_google_sign_in(BeginGoogleSignInRequest { account_id })
            .await
            .unwrap()
    }
    async fn callback(begin: &BeginGoogleSignInResult, query: Option<&str>) -> String {
        let url = reqwest::Url::parse(&begin.authorization_url).unwrap();
        let values: HashMap<_, _> = url.query_pairs().into_owned().collect();
        let redirect = reqwest::Url::parse(&values["redirect_uri"]).unwrap();
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", redirect.port().unwrap()))
            .await
            .unwrap();
        let query = query
            .map(str::to_owned)
            .unwrap_or_else(|| format!("state={}&code=synthetic-code", values["state"]));
        let request = format!(
            "GET {}?{query} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
            redirect.path()
        );
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).await.unwrap();
        String::from_utf8(bytes).unwrap()
    }
    async fn waiting(&self, id: &GoogleSignInId) {
        loop {
            if self.engine.inner.attempts.lock().unwrap()[id].finishing() {
                break;
            }
            tokio::task::yield_now().await;
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
fn replies(subject: &str) -> Vec<Reply> {
    vec![
        Reply::json(
            200,
            r#"{"access_token":"synthetic-access-secret","token_type":"Bearer","expires_in":3600,"refresh_token":"synthetic-refresh-secret"}"#,
        ),
        Reply::json(
            200,
            &format!(r#"{{"sub":"{subject}","email":"user@example.invalid","name":"Original"}}"#),
        ),
    ]
}
#[tokio::test]
async fn valid_callback_persists_via_accounts_and_pkce_stays_internal() {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    use sha2::{Digest, Sha256};
    let f = Fixture::new(replies("stable-user")).await;
    let begin = f.begin(None).await;
    let url = reqwest::Url::parse(&begin.authorization_url).unwrap();
    let params: HashMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(
        params["scope"],
        "openid email profile https://www.googleapis.com/auth/drive.metadata.readonly"
    );
    assert_eq!(params["code_challenge_method"], "S256");
    assert!(!params.contains_key("code_verifier"));
    assert!(f.server.requests.lock().unwrap().is_empty());
    let engine = f.engine.clone();
    let id = begin.connection_id.clone();
    let finish = tokio::spawn(async move {
        engine
            .finish_google_sign_in(FinishGoogleSignInRequest { connection_id: id })
            .await
    });
    assert!(Fixture::callback(&begin, None)
        .await
        .starts_with("HTTP/1.1 200"));
    let result = finish.await.unwrap().unwrap();
    assert!(result.created);
    assert_eq!(result.account.provider_user_id, "stable-user");
    assert_eq!(
        f.engine
            .list_accounts(ListAccountsRequest { provider: None })
            .await
            .unwrap()
            .accounts,
        vec![result.account.clone()]
    );
    let result = f
        .engine
        .update_account(UpdateAccountRequest {
            account_id: result.account.id,
            display_name: Some("Local name".into()),
        })
        .await
        .unwrap();
    assert_eq!(result.account.display_name.as_deref(), Some("Local name"));
    assert_eq!(
        f.engine
            .finish_google_sign_in(FinishGoogleSignInRequest {
                connection_id: begin.connection_id
            })
            .await
            .unwrap_err()
            .kind,
        EngineErrorKind::AttemptNotFound
    );
    let requests = f.server.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].path(), "/token");
    assert_eq!(requests[1].path(), "/v1/userinfo");
    let form = reqwest::Url::parse(&format!(
        "http://localhost/?{}",
        String::from_utf8_lossy(&requests[0].body)
    ))
    .unwrap();
    let fields: HashMap<_, _> = form.query_pairs().into_owned().collect();
    assert_eq!(
        URL_SAFE_NO_PAD.encode(Sha256::digest(fields["code_verifier"].as_bytes())),
        params["code_challenge"]
    );
}
#[tokio::test]
async fn invalid_callback_does_not_exchange_and_concurrent_finish_is_rejected() {
    let f = Fixture::new(replies("stable-user")).await;
    let begin = f.begin(None).await;
    let engine = f.engine.clone();
    let id = begin.connection_id.clone();
    let finish = tokio::spawn(async move {
        engine
            .finish_google_sign_in(FinishGoogleSignInRequest { connection_id: id })
            .await
    });
    f.waiting(&begin.connection_id).await;
    assert_eq!(
        f.engine
            .finish_google_sign_in(FinishGoogleSignInRequest {
                connection_id: begin.connection_id.clone()
            })
            .await
            .unwrap_err()
            .kind,
        EngineErrorKind::AlreadyInProgress
    );
    assert!(
        Fixture::callback(&begin, Some("state=incorrect&code=private-code"))
            .await
            .starts_with("HTTP/1.1 400")
    );
    assert!(f.server.requests.lock().unwrap().is_empty());
    let url = reqwest::Url::parse(&begin.authorization_url).unwrap();
    let values: HashMap<_, _> = url.query_pairs().into_owned().collect();
    assert!(Fixture::callback(
        &begin,
        Some(&format!(
            "state={}&state={}&code=x",
            values["state"], values["state"]
        ))
    )
    .await
    .starts_with("HTTP/1.1 400"));
    assert!(Fixture::callback(&begin, None)
        .await
        .starts_with("HTTP/1.1 200"));
    finish.await.unwrap().unwrap();
}
#[tokio::test]
async fn cancellation_expiration_and_provider_denial_preserve_empty_accounts() {
    let f = Fixture::new(vec![]).await;
    let begin = f.begin(None).await;
    let engine = f.engine.clone();
    let id = begin.connection_id.clone();
    let finish = tokio::spawn(async move {
        engine
            .finish_google_sign_in(FinishGoogleSignInRequest { connection_id: id })
            .await
    });
    f.waiting(&begin.connection_id).await;
    assert!(
        f.engine
            .cancel_google_sign_in(CancelGoogleSignInRequest {
                connection_id: begin.connection_id.clone()
            })
            .cancelled
    );
    assert!(
        !f.engine
            .cancel_google_sign_in(CancelGoogleSignInRequest {
                connection_id: begin.connection_id
            })
            .cancelled
    );
    assert_eq!(
        finish.await.unwrap().unwrap_err().kind,
        EngineErrorKind::Cancelled
    );
    let begin = f.begin(None).await;
    let engine = f.engine.clone();
    let id = begin.connection_id.clone();
    let finish = tokio::spawn(async move {
        engine
            .finish_google_sign_in(FinishGoogleSignInRequest { connection_id: id })
            .await
    });
    f.waiting(&begin.connection_id).await;
    f.engine.inner.attempts.lock().unwrap()[&begin.connection_id].expire();
    assert_eq!(
        finish.await.unwrap().unwrap_err().kind,
        EngineErrorKind::TimedOut
    );
    let begin = f.begin(None).await;
    let engine = f.engine.clone();
    let id = begin.connection_id.clone();
    let finish = tokio::spawn(async move {
        engine
            .finish_google_sign_in(FinishGoogleSignInRequest { connection_id: id })
            .await
    });
    let params: HashMap<_, _> = reqwest::Url::parse(&begin.authorization_url)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect();
    Fixture::callback(
        &begin,
        Some(&format!("state={}&error=access_denied", params["state"])),
    )
    .await;
    assert_eq!(
        finish.await.unwrap().unwrap_err().kind,
        EngineErrorKind::AuthorizationDenied
    );
    assert!(f
        .engine
        .list_accounts(ListAccountsRequest { provider: None })
        .await
        .unwrap()
        .accounts
        .is_empty());
    assert!(f.server.requests.lock().unwrap().is_empty());
}
#[test]
fn boundary_can_be_awaited_without_a_tokio_context() {
    // A small std-only executor drives this future, as GLib would in production.
    struct Wake(std::thread::Thread);
    impl std::task::Wake for Wake {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
        let waker = std::task::Waker::from(Arc::new(Wake(std::thread::current())));
        let mut context = std::task::Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);
        loop {
            match future.as_mut().poll(&mut context) {
                std::task::Poll::Ready(value) => return value,
                std::task::Poll::Pending => std::thread::park(),
            }
        }
    }
    let directory =
        std::env::temp_dir().join(format!("connection-no-runtime-{}", std::process::id()));
    let engine = block_on(Engine::open(EngineConfig {
        data_directory: directory.clone(),
        application_id: "test.no-runtime".into(),
        google_oauth_file: None,
    }))
    .unwrap();
    assert!(
        block_on(engine.list_accounts(ListAccountsRequest { provider: None }))
            .unwrap()
            .accounts
            .is_empty()
    );
    assert_eq!(
        block_on(engine.remote_folder_root(AccountId("absent".into())))
            .unwrap_err()
            .kind,
        EngineErrorKind::Accounts(AccountsError::NotFound)
    );
    drop(engine);
    std::fs::remove_dir_all(directory).unwrap();
}

#[tokio::test]
async fn reauthorization_checks_identity_and_failed_exchange_is_not_retried() {
    let mut sequence = replies("stable-user");
    sequence.extend(replies("different-user"));
    sequence.extend(replies("stable-user"));
    let f = Fixture::new(sequence).await;
    let begin = f.begin(None).await;
    let engine = f.engine.clone();
    let id = begin.connection_id.clone();
    let finish = tokio::spawn(async move {
        engine
            .finish_google_sign_in(FinishGoogleSignInRequest { connection_id: id })
            .await
    });
    Fixture::callback(&begin, None).await;
    let account = finish.await.unwrap().unwrap().account;
    let begin = f.begin(Some(account.id.clone())).await;
    let engine = f.engine.clone();
    let id = begin.connection_id.clone();
    let finish = tokio::spawn(async move {
        engine
            .finish_google_sign_in(FinishGoogleSignInRequest { connection_id: id })
            .await
    });
    Fixture::callback(&begin, None).await;
    assert_eq!(
        finish.await.unwrap().unwrap_err().kind,
        EngineErrorKind::Accounts(AccountsError::IdentityMismatch)
    );
    let begin = f.begin(Some(account.id.clone())).await;
    let engine = f.engine.clone();
    let id = begin.connection_id.clone();
    let finish = tokio::spawn(async move {
        engine
            .finish_google_sign_in(FinishGoogleSignInRequest { connection_id: id })
            .await
    });
    Fixture::callback(&begin, None).await;
    assert_eq!(finish.await.unwrap().unwrap().account.id, account.id);
    assert_eq!(
        f.engine
            .list_accounts(ListAccountsRequest { provider: None })
            .await
            .unwrap()
            .accounts
            .len(),
        1
    );
    let f = Fixture::new(vec![Reply::json(503, "private-provider-secret")]).await;
    let begin = f.begin(None).await;
    let engine = f.engine.clone();
    let id = begin.connection_id.clone();
    let finish = tokio::spawn(async move {
        engine
            .finish_google_sign_in(FinishGoogleSignInRequest { connection_id: id })
            .await
    });
    Fixture::callback(&begin, None).await;
    let error = finish.await.unwrap().unwrap_err();
    assert_eq!(error.kind, EngineErrorKind::ProviderUnavailable);
    assert!(!error.message.contains("private-provider-secret"));
    assert_eq!(f.server.requests.lock().unwrap().len(), 1);
    assert!(f
        .engine
        .list_accounts(ListAccountsRequest { provider: None })
        .await
        .unwrap()
        .accounts
        .is_empty());
}

#[tokio::test]
async fn disconnect_is_local_isolated_idempotent_and_excludes_authorization() {
    let mut sequence = replies("first-user");
    sequence.extend(replies("second-user"));
    let f = Fixture::new(sequence).await;
    let mut accounts = Vec::new();
    for _ in 0..2 {
        let begin = f.begin(None).await;
        let engine = f.engine.clone();
        let id = begin.connection_id.clone();
        let finishing = tokio::spawn(async move {
            engine
                .finish_google_sign_in(FinishGoogleSignInRequest { connection_id: id })
                .await
        });
        Fixture::callback(&begin, None).await;
        accounts.push(finishing.await.unwrap().unwrap().account);
    }
    let user_file = f.directory.join("personal-document.txt");
    std::fs::write(&user_file, b"preserved local content").unwrap();
    // An addition may resolve to an existing account, so even its unknown
    // identity must exclude removal until the attempt is cancelled/completed.
    let pending = f.begin(None).await;
    let failure = f
        .engine
        .disconnect_account(RemoveAccountRequest {
            account_id: accounts[0].id.clone(),
        })
        .await
        .unwrap_err();
    assert_eq!(
        failure.kind,
        EngineErrorKind::Accounts(AccountsError::AccountInUse)
    );
    assert!(
        f.engine
            .cancel_google_sign_in(CancelGoogleSignInRequest {
                connection_id: pending.connection_id
            })
            .cancelled
    );
    assert!(
        f.engine
            .disconnect_account(RemoveAccountRequest {
                account_id: accounts[0].id.clone()
            })
            .await
            .unwrap()
            .removed
    );
    assert!(
        !f.engine
            .disconnect_account(RemoveAccountRequest {
                account_id: accounts[0].id.clone()
            })
            .await
            .unwrap()
            .removed
    );
    assert_eq!(
        f.engine
            .list_accounts(ListAccountsRequest { provider: None })
            .await
            .unwrap()
            .accounts,
        vec![accounts[1].clone()]
    );
    assert_eq!(
        std::fs::read(&user_file).unwrap(),
        b"preserved local content"
    );
    assert_eq!(f.server.requests.lock().unwrap().len(), 4);
    assert_eq!(
        f.engine
            .begin_google_sign_in(BeginGoogleSignInRequest {
                account_id: Some(accounts[0].id.clone())
            })
            .await
            .unwrap_err()
            .kind,
        EngineErrorKind::Accounts(AccountsError::NotFound)
    );
}

impl Fixture {
    async fn sign_in(&self) -> Account {
        let begin = self.begin(None).await;
        let engine = self.engine.clone();
        let id = begin.connection_id.clone();
        let finish = tokio::spawn(async move {
            engine
                .finish_google_sign_in(FinishGoogleSignInRequest { connection_id: id })
                .await
        });
        Self::callback(&begin, None).await;
        finish.await.unwrap().unwrap().account
    }
}
#[tokio::test]
async fn remote_folders_use_account_session_canonical_root_and_page_tokens() {
    let mut sequence = replies("folder-user");
    sequence.extend([
        Reply::json(200, r#"{"id":"canonical-root","name":"My Drive","mimeType":"application/vnd.google-apps.folder","trashed":false}"#),
        Reply::json(200, r#"{"files":[{"id":"child","name":"Folder","mimeType":"application/vnd.google-apps.folder","parents":["canonical-root"],"trashed":false}],"nextPageToken":"opaque+/page","incompleteSearch":false}"#),
        Reply::json(200, r#"{"files":[],"incompleteSearch":false}"#),
    ]);
    let f = Fixture::new(sequence).await;
    let account = f.sign_in().await;
    let root = f
        .engine
        .remote_folder_root(account.id.clone())
        .await
        .unwrap();
    assert_eq!(root.id, "canonical-root");
    assert_eq!(root.parent_id, None);
    let first = f
        .engine
        .list_remote_folders(ListRemoteFoldersRequest {
            account_id: account.id.clone(),
            parent_id: root.id.clone(),
            page_token: None,
        })
        .await
        .unwrap();
    assert_eq!(
        first.folders[0].parent_id.as_deref(),
        Some("canonical-root")
    );
    let last = f
        .engine
        .list_remote_folders(ListRemoteFoldersRequest {
            account_id: account.id.clone(),
            parent_id: root.id,
            page_token: first.next_page_token,
        })
        .await
        .unwrap();
    assert!(last.folders.is_empty());
    assert!(last.next_page_token.is_none());
    let error = f
        .engine
        .remote_folder_root(AccountId("unknown-account".into()))
        .await
        .unwrap_err();
    assert_eq!(
        error.kind,
        EngineErrorKind::Accounts(AccountsError::NotFound)
    );
    let requests = f.server.requests.lock().unwrap();
    assert_eq!(requests.len(), 5);
    assert_eq!(requests[2].path(), "/drive/v3/files/root");
    for request in &requests[2..] {
        assert_eq!(request.method, "GET");
        assert_eq!(
            request.header("authorization"),
            Some("Bearer synthetic-access-secret")
        );
    }
    let query: HashMap<_, _> = requests[3].decoded_query().into_iter().collect();
    assert_eq!(query["q"], "'canonical-root' in parents and mimeType = 'application/vnd.google-apps.folder' and trashed = false");
    assert!(query["fields"].contains("parents"));
    assert!(query["fields"].contains("nextPageToken"));
    let query: HashMap<_, _> = requests[4].decoded_query().into_iter().collect();
    assert_eq!(query["pageToken"], "opaque+/page");
}
#[tokio::test]
async fn remote_errors_do_not_leak_provider_text_or_trigger_retries() {
    let mut sequence = replies("folder-user");
    sequence.extend([
        Reply::json(403, r#"{"error":{"message":"private-secret","details":[{"@type":"type.googleapis.com/google.rpc.ErrorInfo","reason":"ACCESS_TOKEN_SCOPE_INSUFFICIENT"}]}}"#),
        Reply::json(403, r#"{"error":{"message":"private-secret"}}"#),
        Reply::json(404, "private-secret"),
        Reply::json(503, "private-secret"),
    ]);
    let f = Fixture::new(sequence).await;
    let account = f.sign_in().await;
    for expected in [
        RemoteFolderErrorKind::AuthorizationRequired,
        RemoteFolderErrorKind::PermissionDenied,
        RemoteFolderErrorKind::NotFound,
        RemoteFolderErrorKind::Unavailable,
    ] {
        let error = f
            .engine
            .remote_folder_root(account.id.clone())
            .await
            .unwrap_err();
        assert_eq!(error.kind, EngineErrorKind::RemoteFolder(expected));
        assert!(!format!("{error:?}").contains("private-secret"));
    }
    assert_eq!(f.server.requests.lock().unwrap().len(), 6);
}
#[tokio::test]
async fn malformed_and_incomplete_folder_pages_fail_closed_and_ids_are_escaped() {
    let mut sequence = replies("folder-user");
    for json in [
        r#"{"files":[],"incompleteSearch":true}"#,
        r#"{"files":[{"id":"child","name":"Folder","mimeType":"application/vnd.google-apps.folder","parents":["different"],"trashed":false}]}"#,
        r#"{"files":[{"name":"Folder","mimeType":"application/vnd.google-apps.folder","parents":["x'\\"],"trashed":false}]}"#,
        r#"{"files":[],"nextPageToken":""}"#,
    ] {
        sequence.push(Reply::json(200, json));
    }
    let f = Fixture::new(sequence).await;
    let account = f.sign_in().await;
    for _ in 0..4 {
        let error = f
            .engine
            .list_remote_folders(ListRemoteFoldersRequest {
                account_id: account.id.clone(),
                parent_id: "x'\\".into(),
                page_token: None,
            })
            .await
            .unwrap_err();
        assert_eq!(
            error.kind,
            EngineErrorKind::RemoteFolder(RemoteFolderErrorKind::InvalidResponse)
        );
    }
    let requests = f.server.requests.lock().unwrap();
    let query: HashMap<_, _> = requests[2].decoded_query().into_iter().collect();
    assert!(query["q"].starts_with("'x\\'\\\\' in parents"));
}
#[tokio::test]
async fn cancelling_a_remote_query_releases_logout_exclusion() {
    let mut sequence = replies("folder-user");
    let mut reply = Reply::json(
        200,
        r#"{"id":"root","name":"Drive","mimeType":"application/vnd.google-apps.folder","trashed":false}"#,
    );
    reply.delay = std::time::Duration::from_millis(300);
    sequence.push(reply);
    let f = Fixture::new(sequence).await;
    let account = f.sign_in().await;
    let engine = f.engine.clone();
    let id = account.id.clone();
    let query = tokio::spawn(async move { engine.remote_folder_root(id).await });
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while f.server.requests.lock().unwrap().len() < 3 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let error = f
        .engine
        .disconnect_account(RemoveAccountRequest {
            account_id: account.id.clone(),
        })
        .await
        .unwrap_err();
    assert_eq!(
        error.kind,
        EngineErrorKind::Accounts(AccountsError::AccountInUse)
    );
    query.abort();
    let _ = query.await;
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if f.engine.inner.ownership.gate.try_write().is_ok() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(
        f.engine
            .disconnect_account(RemoveAccountRequest {
                account_id: account.id
            })
            .await
            .unwrap()
            .removed
    );
}

#[tokio::test]
async fn remote_queries_keep_account_credentials_separate() {
    let mut sequence = replies("first-user");
    let mut second = replies("second-user");
    second[0] = Reply::json(
        200,
        r#"{"access_token":"second-access-secret","token_type":"Bearer","expires_in":3600,"refresh_token":"second-refresh-secret"}"#,
    );
    sequence.extend(second);
    for _ in 0..2 {
        sequence.push(Reply::json(200, r#"{"id":"root","name":"Drive","mimeType":"application/vnd.google-apps.folder","trashed":false}"#));
    }
    let f = Fixture::new(sequence).await;
    let first = f.sign_in().await;
    let second = f.sign_in().await;
    f.engine.remote_folder_root(first.id).await.unwrap();
    f.engine.remote_folder_root(second.id).await.unwrap();
    let requests = f.server.requests.lock().unwrap();
    assert_eq!(
        requests[4].header("authorization"),
        Some("Bearer synthetic-access-secret")
    );
    assert_eq!(
        requests[5].header("authorization"),
        Some("Bearer second-access-secret")
    );
}
#[tokio::test]
async fn remote_queries_renew_expired_credentials_through_accounts() {
    let mut sequence = replies("folder-user");
    sequence[0] = Reply::json(
        200,
        r#"{"access_token":"old-secret","token_type":"Bearer","expires_in":1,"refresh_token":"refresh-secret"}"#,
    );
    sequence.extend([
        Reply::json(200, r#"{"access_token":"renewed-secret","token_type":"Bearer","expires_in":3600}"#),
        Reply::json(200, r#"{"sub":"folder-user","email":"user@example.invalid"}"#),
        Reply::json(200, r#"{"id":"root","name":"Drive","mimeType":"application/vnd.google-apps.folder","trashed":false}"#),
    ]);
    let f = Fixture::new(sequence).await;
    let account = f.sign_in().await;
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    f.engine.remote_folder_root(account.id).await.unwrap();
    let requests = f.server.requests.lock().unwrap();
    assert_eq!(requests[2].path(), "/token");
    assert_eq!(
        requests[4].header("authorization"),
        Some("Bearer renewed-secret")
    );
}
