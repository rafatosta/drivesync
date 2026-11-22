use super::*;
use crate::accounts::tests::http_support::{Reply, Server};
fn config() -> RemoteObserverConfig {
    RemoteObserverConfig {
        account_id: AccountId("account".into()),
        provider_id: ProviderId("google".into()),
        sync_root_id: SyncRootId("scope".into()),
        remote_root_id: RemoteId("root".into()),
    }
}
fn root() -> Reply {
    Reply::json(
        200,
        r#"{"id":"root","mimeType":"application/vnd.google-apps.folder","trashed":false,"capabilities":{"canListChildren":true}}"#,
    )
}
fn observer(server: &Server) -> Box<dyn RemoteObserver> {
    let provider = GoogleObserverProvider {
        client: GoogleDriveClient::with_base_urls(
            format!("{}/drive/v3", server.base),
            format!("{}/upload/drive/v3", server.base),
        )
        .unwrap(),
    };
    provider.create(config()).unwrap()
}
fn request(mode: RemoteObserveMode, budget: u64) -> RemoteObserveRequest {
    RemoteObserveRequest {
        scope: RemoteScope {
            account_id: AccountId("account".into()),
            provider_id: ProviderId("google".into()),
            sync_root_id: SyncRootId("scope".into()),
            remote_root_id: RemoteId("root".into()),
        },
        mode,
        page_budget: Some(budget),
        known_remote_ids: Vec::new(),
    }
}
fn session() -> GoogleObserverSession {
    GoogleObserverSession {
        account_id: AccountId("account".into()),
        provider_id: ProviderId("google".into()),
        session: GoogleSession::new("account", "synthetic"),
    }
}
#[tokio::test]
async fn paginated_capture_publishes_only_complete_coverage_then_uses_changes_cursor() {
    let server = Server::start(vec![
        Reply::json(200, r#"{"startPageToken":"start"}"#),
        root(),
        Reply::json(
            200,
            r#"{"files":[{"id":"one","name":"a","mimeType":"text/plain","parents":["root"],"trashed":false}],"incompleteSearch":false,"nextPageToken":"next"}"#,
        ),
        Reply::json(
            200,
            r#"{"files":[{"id":"two","name":"b","mimeType":"text/plain","parents":["root"],"trashed":false}],"incompleteSearch":false}"#,
        ),
        root(),
        root(),
        Reply::json(200, r#"{"changes":[],"newStartPageToken":"latest"}"#),
    ]);
    let observer = observer(&server);
    let token = CancellationToken::new();
    let auth = session();
    let first = observer
        .observe(request(RemoteObserveMode::Initial, 1), &auth, &token)
        .await
        .unwrap();
    assert_eq!(first.coverage, ObservationCoverage::Partial);
    assert!(first.observations.is_empty());
    assert!(first.has_more);
    let second = observer
        .observe(request(RemoteObserveMode::Incremental, 1), &auth, &token)
        .await
        .unwrap();
    assert_eq!(second.coverage, ObservationCoverage::Complete);
    assert_eq!(second.observations.len(), 2);
    let third = observer
        .observe(request(RemoteObserveMode::Incremental, 1), &auth, &token)
        .await
        .unwrap();
    assert_eq!(third.observations.len(), 2);
    assert_eq!(third.continuation.unwrap().value, "latest");
    assert!(server
        .requests
        .lock()
        .unwrap()
        .iter()
        .all(|r| r.method == "GET"));
}
#[tokio::test]
async fn incomplete_search_and_cursor_cycles_never_confirm_absence() {
    for page in [
        r#"{"files":[],"incompleteSearch":true}"#,
        r#"{"files":[],"incompleteSearch":false,"nextPageToken":"repeat"}"#,
    ] {
        let server = Server::start(vec![
            Reply::json(200, r#"{"startPageToken":"start"}"#),
            root(),
            Reply::json(200, page),
            Reply::json(200, page),
        ]);
        let observer = observer(&server);
        let token = CancellationToken::new();
        let auth = session();
        let first = observer
            .observe(request(RemoteObserveMode::Initial, 1), &auth, &token)
            .await;
        if let Err(error) = first {
            assert_eq!(error, RemoteObserverError::IncompleteCoverage);
        } else {
            assert_eq!(
                observer
                    .observe(request(RemoteObserveMode::Incremental, 1), &auth, &token)
                    .await
                    .unwrap_err(),
                RemoteObserverError::InvalidCursor
            );
        }
    }
}
#[tokio::test]
async fn cancelled_and_cross_account_requests_do_not_access_the_provider() {
    let server = Server::start(vec![]);
    let observer = observer(&server);
    let token = CancellationToken::new();
    token.cancel();
    assert_eq!(
        observer
            .observe(request(RemoteObserveMode::Initial, 1), &session(), &token)
            .await
            .unwrap_err(),
        RemoteObserverError::Cancelled
    );
    let mut auth = session();
    auth.account_id = AccountId("other".into());
    assert_eq!(
        observer
            .observe(
                request(RemoteObserveMode::Initial, 1),
                &auth,
                &CancellationToken::new()
            )
            .await
            .unwrap_err(),
        RemoteObserverError::AuthorizationRequired
    );
    assert!(server.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn changes_recapture_moves_out_of_scope_without_claiming_deletion() {
    let server = Server::start(vec![
        Reply::json(200, r#"{"startPageToken":"start"}"#),
        root(),
        Reply::json(
            200,
            r#"{"files":[{"id":"one","name":"a","mimeType":"text/plain","parents":["root"],"trashed":false}],"incompleteSearch":false}"#,
        ),
        root(),
        root(),
        Reply::json(
            200,
            r#"{"changes":[{"fileId":"one","removed":true}],"newStartPageToken":"change"}"#,
        ),
        Reply::json(200, r#"{"startPageToken":"recapture"}"#),
        root(),
        Reply::json(200, r#"{"files":[],"incompleteSearch":false}"#),
        root(),
        Reply::json(
            200,
            r#"{"id":"one","name":"a","mimeType":"text/plain","parents":["outside"],"trashed":false}"#,
        ),
        Reply::json(200, r#"{"id":"outside","parents":[],"trashed":false}"#),
    ]);
    let observer = observer(&server);
    let auth = session();
    let token = CancellationToken::new();
    assert_eq!(
        observer
            .observe(request(RemoteObserveMode::Initial, 4), &auth, &token)
            .await
            .unwrap()
            .observations
            .len(),
        1
    );
    let result = observer
        .observe(request(RemoteObserveMode::Incremental, 4), &auth, &token)
        .await
        .unwrap();
    assert_eq!(result.coverage, ObservationCoverage::Complete);
    assert_eq!(result.observations.len(), 1);
    assert_eq!(result.observations[0].state, FileState::OutsideScope);
    assert_eq!(result.observations[0].checksum, Evidence::NotCollected);
    assert_eq!(result.continuation.unwrap().value, "recapture");
}
#[tokio::test]
async fn nested_folders_are_traversed_and_unrelated_parent_entries_are_rejected() {
    let folder = r#"{"files":[{"id":"folder","name":"nested","mimeType":"application/vnd.google-apps.folder","parents":["root"],"trashed":false,"capabilities":{"canListChildren":true}}],"incompleteSearch":false}"#;
    for parent in ["folder", "outside"] {
        let page=serde_json::json!({"files":[{"id":"child","name":"file","mimeType":"text/plain","parents":[parent],"trashed":false}],"incompleteSearch":false}).to_string();
        let server = Server::start(vec![
            Reply::json(200, r#"{"startPageToken":"start"}"#),
            root(),
            Reply::json(200, folder),
            Reply::json(200, &page),
            root(),
        ]);
        let observer = observer(&server);
        let result = observer
            .observe(
                request(RemoteObserveMode::Initial, 4),
                &session(),
                &CancellationToken::new(),
            )
            .await;
        if parent == "folder" {
            assert_eq!(result.unwrap().observations.len(), 2);
            let requests = server.requests.lock().unwrap();
            assert!(requests[3]
                .decoded_query()
                .iter()
                .any(|(key, value)| key == "q" && value.contains("'folder' in parents")));
        } else {
            assert_eq!(result.unwrap_err(), RemoteObserverError::IncompleteCoverage);
        }
    }
}
#[tokio::test]
async fn expired_cursor_recovers_by_a_new_capture_and_cancellation_interrupts_reads() {
    let server = Server::start(vec![
        Reply::json(200, r#"{"startPageToken":"start"}"#),
        root(),
        Reply::json(200, r#"{"files":[],"incompleteSearch":false}"#),
        root(),
        root(),
        Reply::json(410, r#"{"error":{"code":410,"message":"gone"}}"#),
        Reply::json(200, r#"{"startPageToken":"new"}"#),
        root(),
        Reply::json(200, r#"{"files":[],"incompleteSearch":false}"#),
        root(),
    ]);
    let observer = observer(&server);
    let auth = session();
    let token = CancellationToken::new();
    observer
        .observe(request(RemoteObserveMode::Initial, 4), &auth, &token)
        .await
        .unwrap();
    assert_eq!(
        observer
            .observe(request(RemoteObserveMode::Incremental, 4), &auth, &token)
            .await
            .unwrap_err(),
        RemoteObserverError::InvalidCursor
    );
    assert_eq!(
        observer
            .observe(request(RemoteObserveMode::Recovery, 4), &auth, &token)
            .await
            .unwrap()
            .continuation
            .unwrap()
            .value,
        "new"
    );
    let mut delayed = Reply::json(200, r#"{"startPageToken":"start"}"#);
    delayed.delay = std::time::Duration::from_secs(2);
    let server = Server::start(vec![delayed]);
    let observer = super::tests::observer(&server);
    let token = CancellationToken::new();
    let work = observer.observe(request(RemoteObserveMode::Initial, 4), &auth, &token);
    tokio::pin!(work);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(40), &mut work)
            .await
            .is_err()
    );
    token.cancel();
    assert_eq!(
        tokio::time::timeout(std::time::Duration::from_millis(500), work)
            .await
            .unwrap()
            .unwrap_err(),
        RemoteObserverError::Cancelled
    );
}

#[tokio::test]
async fn persisted_missing_ids_are_probed_after_restart_without_inventing_absence() {
    for (reply, expected, permission) in [
        (
            Reply::json(
                404,
                r#"{"error":{"code":404,"message":"not found","errors":[{"reason":"notFound"}]}}"#,
            ),
            FileState::Unknown,
            false,
        ),
        (
            Reply::json(
                403,
                r#"{"error":{"code":403,"message":"denied","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
            ),
            FileState::Unavailable,
            true,
        ),
        (
            Reply::json(
                200,
                r#"{"id":"persisted","name":"file","parents":["root"],"mimeType":"text/plain","trashed":true}"#,
            ),
            FileState::Absent,
            false,
        ),
        (
            Reply::json(
                200,
                r#"{"id":"persisted","name":"file","parents":[],"mimeType":"text/plain","trashed":false}"#,
            ),
            FileState::OutsideScope,
            false,
        ),
        // A readable item allegedly still in the subtree contradicts the completed listing.
        (
            Reply::json(
                200,
                r#"{"id":"persisted","name":"file","parents":["root"],"mimeType":"text/plain","trashed":false}"#,
            ),
            FileState::Unknown,
            false,
        ),
    ] {
        let server = Server::start(vec![
            Reply::json(200, r#"{"startPageToken":"start"}"#),
            root(),
            Reply::json(200, r#"{"files":[],"incompleteSearch":false}"#),
            root(),
            reply,
        ]);
        let observer = observer(&server);
        let mut request = request(RemoteObserveMode::Initial, 4);
        request.known_remote_ids = vec![RemoteId("persisted".into())];
        let result = observer
            .observe(request, &session(), &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(result.observations.len(), 1);
        let observation = &result.observations[0];
        assert_eq!(observation.state, expected);
        if expected == FileState::Absent {
            assert_eq!(observation.trashed, Evidence::Known(true));
        }
        if permission {
            assert_eq!(observation.capabilities, Evidence::PermissionDenied);
        }
        assert!(server
            .requests
            .lock()
            .unwrap()
            .iter()
            .all(|request| request.method == "GET"));
        assert!(server
            .requests
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .target
            .contains("persisted"));
    }
}

#[tokio::test]
async fn missing_item_ancestry_errors_cycles_and_wrong_ids_remain_inconclusive() {
    for response in [
        Reply::json(404, r#"{"error":{"code":404,"message":"missing"}}"#),
        Reply::json(
            200,
            r#"{"id":"outside","parents":["outside"],"trashed":false}"#,
        ),
        Reply::json(200, r#"{"id":"wrong","parents":[],"trashed":false}"#),
    ] {
        let server = Server::start(vec![
            Reply::json(200, r#"{"startPageToken":"start"}"#),
            root(),
            Reply::json(200, r#"{"files":[],"incompleteSearch":false}"#),
            root(),
            Reply::json(
                200,
                r#"{"id":"persisted","parents":["outside"],"trashed":false}"#,
            ),
            response,
        ]);
        let observer = observer(&server);
        let mut request = request(RemoteObserveMode::Initial, 4);
        request.known_remote_ids = vec![RemoteId("persisted".into())];
        let result = observer
            .observe(request, &session(), &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(result.observations[0].state, FileState::Unknown);
    }
}
