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
fn observer(server: &Server) -> GoogleObserver {
    GoogleObserver {
        client: GoogleDriveClient::with_base_urls(
            format!("{}/drive/v3", server.base),
            format!("{}/upload/drive/v3", server.base),
        )
        .unwrap(),
        config: config(),
        state: Mutex::new(State::default()),
    }
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
            r#"{"files":[{"id":"one","name":"a","mimeType":"text/plain","parents":["root"],"trashed":false},{"id":"lock","name":".~lock.b#","mimeType":"application/octet-stream","parents":["root"],"trashed":false}],"incompleteSearch":false,"nextPageToken":"next"}"#,
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
        .observe(
            request(RemoteObserveMode::Initial, 1),
            &auth,
            &token,
            RemoteActivity::default(),
        )
        .await
        .unwrap();
    assert_eq!(first.coverage, ObservationCoverage::Partial);
    assert!(first.observations.is_empty());
    assert!(first.has_more);
    let second = observer
        .observe(
            request(RemoteObserveMode::Incremental, 1),
            &auth,
            &token,
            RemoteActivity::default(),
        )
        .await
        .unwrap();
    assert_eq!(second.coverage, ObservationCoverage::Complete);
    assert_eq!(second.observations.len(), 2);
    let third = observer
        .observe(
            request(RemoteObserveMode::Incremental, 1),
            &auth,
            &token,
            RemoteActivity::default(),
        )
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
            .observe(
                request(RemoteObserveMode::Initial, 1),
                &auth,
                &token,
                RemoteActivity::default(),
            )
            .await;
        if let Err(error) = first {
            assert_eq!(error, RemoteObserverError::IncompleteCoverage);
        } else {
            assert_eq!(
                observer
                    .observe(
                        request(RemoteObserveMode::Incremental, 1),
                        &auth,
                        &token,
                        RemoteActivity::default()
                    )
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
            .observe(
                request(RemoteObserveMode::Initial, 1),
                &session(),
                &token,
                RemoteActivity::default()
            )
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
                &CancellationToken::new(),
                RemoteActivity::default()
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
            .observe(
                request(RemoteObserveMode::Initial, 4),
                &auth,
                &token,
                RemoteActivity::default()
            )
            .await
            .unwrap()
            .observations
            .len(),
        1
    );
    let result = observer
        .observe(
            request(RemoteObserveMode::Incremental, 4),
            &auth,
            &token,
            RemoteActivity::default(),
        )
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
                RemoteActivity::default(),
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
        .observe(
            request(RemoteObserveMode::Initial, 4),
            &auth,
            &token,
            RemoteActivity::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        observer
            .observe(
                request(RemoteObserveMode::Incremental, 4),
                &auth,
                &token,
                RemoteActivity::default()
            )
            .await
            .unwrap_err(),
        RemoteObserverError::InvalidCursor
    );
    assert_eq!(
        observer
            .observe(
                request(RemoteObserveMode::Recovery, 4),
                &auth,
                &token,
                RemoteActivity::default()
            )
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
    let work = observer.observe(
        request(RemoteObserveMode::Initial, 4),
        &auth,
        &token,
        RemoteActivity::default(),
    );
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
            .observe(
                request,
                &session(),
                &CancellationToken::new(),
                RemoteActivity::default(),
            )
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
            .observe(
                request,
                &session(),
                &CancellationToken::new(),
                RemoteActivity::default(),
            )
            .await
            .unwrap();
        assert_eq!(result.observations[0].state, FileState::Unknown);
    }
}

#[tokio::test]
async fn a_soft_item_limit_retries_the_same_unchanged_tree_with_larger_capacity() {
    let page = r#"{"files":[{"id":"one","name":"a","mimeType":"text/plain","parents":["root"],"trashed":false},{"id":"two","name":"b","mimeType":"text/plain","parents":["root"],"trashed":false}],"incompleteSearch":false}"#;
    let server = Server::start(vec![
        Reply::json(200, r#"{"startPageToken":"start"}"#),
        root(),
        Reply::json(200, page),
        Reply::json(200, r#"{"startPageToken":"retry"}"#),
        root(),
        Reply::json(200, page),
        root(),
    ]);
    let observer = GoogleObserver {
        config: config(),
        client: GoogleDriveClient::with_base_urls(
            format!("{}/drive/v3", server.base),
            format!("{}/upload/drive/v3", server.base),
        )
        .unwrap(),
        state: Mutex::new(State {
            item_limit: 1,
            ..Default::default()
        }),
    };
    assert!(matches!(
        observer
            .observe(
                request(RemoteObserveMode::Initial, 16),
                &session(),
                &CancellationToken::new(),
                RemoteActivity::default()
            )
            .await,
        Err(RemoteObserverError::IncompleteCoverage)
    ));
    assert_eq!(observer.state.lock().await.item_limit, 2);
    let retried = observer
        .observe(
            request(RemoteObserveMode::Recovery, 16),
            &session(),
            &CancellationToken::new(),
            RemoteActivity::default(),
        )
        .await
        .unwrap();
    assert_eq!(retried.coverage, ObservationCoverage::Complete);
    assert_eq!(retried.observations.len(), 2);
}

#[tokio::test]
async fn quiet_changes_cursors_still_refresh_current_metadata_without_another_edit() {
    let page = |version: &str, hash: &str| {
        Reply::json(200,&serde_json::json!({"files":[{"id":"one","name":"file","parents":["root"],"mimeType":"text/plain","version":version,"size":"8","sha256Checksum":hash,"trashed":false}],"incompleteSearch":false}).to_string())
    };
    let server = Server::start(vec![
        Reply::json(200, r#"{"startPageToken":"start"}"#),
        root(),
        page("1", &"a".repeat(64)),
        root(),
        root(),
        Reply::json(200, r#"{"changes":[],"newStartPageToken":"cursor1"}"#),
        root(),
        Reply::json(200, r#"{"changes":[],"newStartPageToken":"cursor2"}"#),
        Reply::json(200, r#"{"startPageToken":"recapture"}"#),
        root(),
        page("2", &"b".repeat(64)),
        root(),
    ]);
    let observer = observer(&server);
    let auth = session();
    let token = CancellationToken::new();
    observer
        .observe(
            request(RemoteObserveMode::Initial, 16),
            &auth,
            &token,
            RemoteActivity::default(),
        )
        .await
        .unwrap();
    let cached = observer
        .observe(
            request(RemoteObserveMode::Incremental, 16),
            &auth,
            &token,
            RemoteActivity::default(),
        )
        .await
        .unwrap();
    assert_eq!(cached.observations[0].version, Evidence::Known(1));
    observer.state.lock().await.last_full = Some(Instant::now() - AUDIT_INTERVAL);
    let refreshed = observer
        .observe(
            request(RemoteObserveMode::Incremental, 16),
            &auth,
            &token,
            RemoteActivity::default(),
        )
        .await
        .unwrap();
    assert_eq!(refreshed.coverage, ObservationCoverage::Complete);
    assert_eq!(refreshed.observations[0].version, Evidence::Known(2));
    assert!(
        matches!(&refreshed.observations[0].checksum,Evidence::Known(hash) if hash.value=="b".repeat(64))
    );
}

#[tokio::test]
async fn unavailable_or_denied_roots_recover_from_current_facts_without_another_change() {
    for status in [503, 403] {
        let server = Server::start(vec![
            Reply::json(200, r#"{"startPageToken":"before"}"#),
            Reply::json(status, r#"{"error":{"message":"unavailable"}}"#),
            Reply::json(200, r#"{"startPageToken":"after"}"#),
            root(),
            Reply::json(
                200,
                r#"{"files":[{"id":"one","name":"file","mimeType":"text/plain","parents":["root"],"trashed":false,"version":"1"}],"incompleteSearch":false}"#,
            ),
            root(),
        ]);
        let observer = observer(&server);
        let auth = session();
        let token = CancellationToken::new();
        assert!(observer
            .observe(
                request(RemoteObserveMode::Initial, 16),
                &auth,
                &token,
                RemoteActivity::default()
            )
            .await
            .is_err());
        let after = observer
            .observe(
                request(RemoteObserveMode::Recovery, 16),
                &auth,
                &token,
                RemoteActivity::default(),
            )
            .await
            .unwrap();
        assert_eq!(after.coverage, ObservationCoverage::Complete);
        assert_eq!(after.observations.len(), 1);
        assert_eq!(after.observations[0].state, FileState::Present);
    }
}

#[tokio::test]
async fn office_revision_without_sha256_requires_verified_bytes_and_stable_metadata() {
    use sha2::{Digest, Sha256};
    let mime = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
    let office = serde_json::json!({
        "id": "office", "name": "Minutes.docx", "parents": ["root"], "mimeType": mime,
        "version": "2", "headRevisionId": "revision", "size": "5",
        "md5Checksum": "5d41402abc4b2a76b9719d911017c592", "trashed": false,
        "capabilities": {"canDownload": true},
    });
    for (bytes, version, verified) in [
        ("hello", "2", true),
        ("world", "2", false),
        ("hello", "3", false),
    ] {
        let mut replies = vec![
            Reply::json(200, r#"{"startPageToken":"start"}"#),
            root(),
            Reply::json(
                200,
                &serde_json::json!({"files": [office], "incompleteSearch": false}).to_string(),
            ),
            Reply::json(200, bytes),
        ];
        if bytes == "hello" {
            let mut after = office.clone();
            after["version"] = version.into();
            replies.push(Reply::json(200, &after.to_string()));
        }
        replies.push(root());
        let server = Server::start(replies);
        let observed = observer(&server)
            .observe(
                request(RemoteObserveMode::Initial, 16),
                &session(),
                &CancellationToken::new(),
                RemoteActivity::default(),
            )
            .await
            .unwrap();
        assert_eq!(
            observed.observations[0].checksum,
            if verified {
                Evidence::Known(Checksum {
                    algorithm: "sha256".into(),
                    value: format!("{:x}", Sha256::digest(b"hello")),
                })
            } else {
                Evidence::NotCollected
            }
        );
    }
}

#[tokio::test]
async fn a_leaf_change_publishes_its_revision_without_resetting_the_cursor() {
    let server = Server::start(vec![
        Reply::json(200, r#"{"startPageToken":"initial"}"#),
        root(),
        Reply::json(
            200,
            r#"{"files":[{"id":"one","name":"old","parents":["root"],"mimeType":"text/plain","version":"1","trashed":false}],"incompleteSearch":false}"#,
        ),
        root(),
        root(),
        Reply::json(
            200,
            r#"{"changes":[{"changeType":"file","removed":false,"fileId":"one","file":{"id":"one","name":"new","parents":["root"],"mimeType":"text/plain","version":"2","trashed":false}}],"newStartPageToken":"continued"}"#,
        ),
    ]);
    let observer = observer(&server);
    let auth = session();
    let token = CancellationToken::new();
    observer
        .observe(
            request(RemoteObserveMode::Initial, 16),
            &auth,
            &token,
            RemoteActivity::default(),
        )
        .await
        .unwrap();
    let updated = observer
        .observe(
            request(RemoteObserveMode::Incremental, 16),
            &auth,
            &token,
            RemoteActivity::default(),
        )
        .await
        .unwrap();
    assert_eq!(updated.coverage, ObservationCoverage::Complete);
    assert_eq!(updated.continuation.unwrap().value, "continued");
    assert_eq!(updated.observations[0].version, Evidence::Known(2));
    assert_eq!(
        updated.observations[0].event.as_ref().unwrap().kind,
        ObservationKind::Renamed,
    );
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 6);
    assert!(requests[5].target.contains("pageToken=initial"));
}

#[tokio::test]
async fn changes_for_the_same_binary_revision_reuse_verified_sha256() {
    let office = serde_json::json!({
        "id": "office", "name": "Minutes.docx", "parents": ["root"],
        "mimeType": "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "version": "2", "headRevisionId": "revision", "size": "5",
        "md5Checksum": "5d41402abc4b2a76b9719d911017c592", "trashed": false,
        "capabilities": {"canDownload": true},
    });
    let server = Server::start(vec![
        Reply::json(200, r#"{"startPageToken":"start"}"#),
        root(),
        Reply::json(
            200,
            &serde_json::json!({"files":[office],"incompleteSearch":false}).to_string(),
        ),
        Reply::json(200, "hello"),
        Reply::json(200, &office.to_string()),
        root(),
        root(),
        Reply::json(
            200,
            &serde_json::json!({
                "changes":[{"changeType":"file","removed":false,"fileId":"office","file":office}],
                "newStartPageToken":"next"
            })
            .to_string(),
        ),
    ]);
    let observer = observer(&server);
    let auth = session();
    let cancel = CancellationToken::new();
    let first = observer
        .observe(
            request(RemoteObserveMode::Initial, 16),
            &auth,
            &cancel,
            RemoteActivity::default(),
        )
        .await
        .unwrap();
    let update = observer
        .observe(
            request(RemoteObserveMode::Incremental, 16),
            &auth,
            &cancel,
            RemoteActivity::default(),
        )
        .await
        .unwrap();
    assert!(matches!(
        &first.observations[0].checksum,
        Evidence::Known(_)
    ));
    assert_eq!(
        update.observations[0].checksum,
        first.observations[0].checksum
    );
    assert_eq!(update.continuation.unwrap().value, "next");
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 8);
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.target.contains("alt=media"))
            .count(),
        1
    );
}

#[tokio::test]
async fn paginated_progress_reports_scoped_folder_names_without_publishing_partial_evidence() {
    use crate::{
        detectors::activity::{ActivityChannel, Details, Phase, Scope, Source, State},
        synchronization::SynchronizationWork,
    };
    let server = Server::start(vec![
        Reply::json(200, r#"{"startPageToken":"start"}"#),
        root(),
        Reply::json(
            200,
            r#"{"files":[{"id":"folder-id","name":"Teaching","mimeType":"application/vnd.google-apps.folder","parents":["root"],"trashed":false,"capabilities":{"canListChildren":true}}],"incompleteSearch":false}"#,
        ),
        Reply::json(200, r#"{"files":[],"incompleteSearch":false}"#),
        root(),
    ]);
    let observer = observer(&server);
    let channel = ActivityChannel::new(Scope {
        account: AccountId("account".into()),
        root: SyncRootId("scope".into()),
    });
    let scoped = channel.open(Source::Remote);
    let remote = scoped.publisher().begin(
        Source::Remote.origin(),
        None,
        Phase::Observe,
        State::Running,
        Details::default(),
    );
    let req = request(RemoteObserveMode::Initial, 1);
    let result = observer
        .observe(
            req,
            &session(),
            &CancellationToken::new(),
            RemoteActivity::new(&remote),
        )
        .await
        .unwrap();
    assert!(result.has_more);
    assert!(result.observations.is_empty());
    let req = request(RemoteObserveMode::Incremental, 1);
    let result = observer
        .observe(
            req,
            &session(),
            &CancellationToken::new(),
            RemoteActivity::new(&remote),
        )
        .await
        .unwrap();
    assert_eq!(result.coverage, ObservationCoverage::Complete);
    let mut work = SynchronizationWork::default();
    channel.inbox().project(&mut work);
    let scan = work.remote.unwrap();
    assert_eq!(scan.path.as_deref(), Some(std::path::Path::new("Teaching")));
    assert_eq!((scan.items, scan.pages), (1, 2));
}

#[tokio::test]
async fn an_outstanding_provider_call_is_visible_and_cancellation_retires_it() {
    use crate::detectors::activity::{ActivityChannel, Details, Phase, Scope, Source, State, Wait};
    let mut deferred = Reply::json(200, r#"{"startPageToken":"private-cursor"}"#);
    deferred.delay = Duration::from_secs(1);
    let server = Server::start(vec![deferred]);
    let observer = observer(&server);
    let channel = ActivityChannel::with_diagnostics(
        Scope {
            account: config().account_id,
            root: config().sync_root_id,
        },
        true,
    );
    let remote = channel.open(Source::Remote);
    let collection = remote.publisher().begin(
        Source::Remote.origin(),
        None,
        Phase::Collect,
        State::Running,
        Details::default(),
    );
    let token = CancellationToken::new();
    let auth = session();
    let mut call = observer.observe(
        request(RemoteObserveMode::Initial, 1),
        &auth,
        &token,
        RemoteActivity::new(&collection),
    );
    tokio::select! {
        biased;
        _ = &mut call => panic!("delayed checkpoint returned prematurely"),
        _ = std::future::ready(()) => {},
    }
    assert!(channel.inbox().current().iter().any(|activity| {
        activity.phase == Phase::Checkpoint && activity.state == State::Waiting(Wait::Provider)
    }));
    token.cancel();
    assert_eq!(call.await, Err(RemoteObserverError::Cancelled));
    collection.finish(State::Cancelled);
    assert!(channel.inbox().current().is_empty());
    let log = channel.inbox().test_diagnostics();
    assert!(log.contains("phase=Checkpoint event=Started state=Waiting(Provider)"));
    assert!(log.contains("phase=Checkpoint event=Finished state=Cancelled"));
    assert!(!log.contains("private-cursor"));
    assert!(!log.contains("synthetic"));
}

#[tokio::test]
async fn incremental_pages_track_actual_changes_across_rounds_and_project_round_pages() {
    use crate::{
        detectors::activity::{ActivityChannel, Details, Phase, Scope, Source, State, Unit, Wait},
        synchronization::SynchronizationWork,
    };
    let server = Server::start(vec![
        Reply::json(200, r#"{"startPageToken":"private-start"}"#),
        root(),
        Reply::json(
            200,
            r#"{"files":[{"id":"one","name":"private-file","parents":["root"],"mimeType":"text/plain","version":"1","sha256Checksum":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","trashed":false},{"id":"two","name":"private-file-two","parents":["root"],"mimeType":"text/plain","version":"1","sha256Checksum":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","trashed":false}],"incompleteSearch":false}"#,
        ),
        root(),
        root(),
        Reply::json(
            200,
            r#"{"changes":[{"changeType":"file","removed":false,"fileId":"one","file":{"id":"one","name":"private-file","parents":["root"],"mimeType":"text/plain","version":"2","sha256Checksum":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","trashed":false}}],"nextPageToken":"private-page"}"#,
        ),
        root(),
        Reply::json(200, r#"{"changes":[],"newStartPageToken":"private-end"}"#),
    ]);
    let observer = observer(&server);
    let auth = session();
    let token = CancellationToken::new();
    observer
        .observe(
            request(RemoteObserveMode::Initial, 1),
            &auth,
            &token,
            RemoteActivity::default(),
        )
        .await
        .unwrap();
    let channel = ActivityChannel::with_diagnostics(
        Scope {
            account: config().account_id,
            root: config().sync_root_id,
        },
        true,
    );
    let remote = channel.open(Source::Remote);
    let collection = remote.publisher().begin(
        Source::Remote.origin(),
        None,
        Phase::Collect,
        State::Running,
        Details::default(),
    );
    let activity = RemoteActivity::new(&collection);
    let first = observer
        .observe(
            request(RemoteObserveMode::Incremental, 1),
            &auth,
            &token,
            activity,
        )
        .await
        .unwrap();
    assert!(first.has_more);
    assert!(first.observations.is_empty());
    assert_eq!(first.coverage, ObservationCoverage::Partial);
    collection.set_state(State::Waiting(Wait::Continuation));
    let inbox = channel.inbox();
    let before = inbox.current().pop().unwrap();
    assert_eq!(before.state, State::Waiting(Wait::Continuation));
    let second = observer
        .observe(
            request(RemoteObserveMode::Incremental, 1),
            &auth,
            &token,
            activity,
        )
        .await
        .unwrap();
    assert_eq!(second.coverage, ObservationCoverage::Complete);
    assert_eq!(second.observations.len(), 2);
    let after = inbox.current().pop().unwrap();
    assert_eq!(before.id, after.id);
    assert!(after.sequence > before.sequence);
    let counted = |unit| {
        after
            .details
            .progress
            .iter()
            .flatten()
            .find(|metric| metric.unit == unit)
            .copied()
            .unwrap()
    };
    assert_eq!(counted(Unit::PagesReceived).completed, 2);
    assert_eq!(counted(Unit::Changes).completed, 1);
    assert_eq!(counted(Unit::SnapshotItems).completed, 2);
    assert_eq!(counted(Unit::PagesReceived).total, None);
    let mut work = SynchronizationWork::default();
    inbox.project(&mut work);
    let legacy = work.remote.unwrap();
    assert_eq!((legacy.items, legacy.pages), (2, 1));
    collection.finish(State::Complete);
    let log = inbox.test_diagnostics();
    assert!(log.contains("Changes=1 SnapshotItems=2"));
    assert!(!log.contains("private-"));
}

#[tokio::test]
#[ignore = "benchmarks 6,400 synthetic Google entries on loopback"]
async fn benchmark_remote_capture_with_optional_activity() {
    use crate::detectors::activity::{ActivityChannel, Details, Phase, Scope, Source, State};
    let page_count = 64;
    let items_per_page = 100;
    let mut replies = vec![Reply::json(200, r#"{"startPageToken":"start"}"#), root()];
    for page in 0..page_count {
        let files = (0..items_per_page).map(|index| {
            let name = format!("file-{page}-{index}");
            serde_json::json!({"id": name, "name": name, "mimeType":"text/plain","parents":["root"],"trashed":false})
        }).collect::<Vec<_>>();
        let mut response = serde_json::json!({"files":files,"incompleteSearch":false});
        if page + 1 != page_count {
            response["nextPageToken"] = serde_json::Value::String(format!("page-{}", page + 1));
        }
        replies.push(Reply::json(200, &response.to_string()));
    }
    replies.push(root());

    let server = Server::start(replies);
    let observer = observer(&server);
    let channel = ActivityChannel::with_diagnostics(
        Scope {
            account: config().account_id,
            root: config().sync_root_id,
        },
        false,
    );
    let session = channel.open(Source::Remote);
    let collection = session.publisher().begin(
        Source::Remote.origin(),
        None,
        Phase::Collect,
        State::Running,
        Details::default(),
    );
    let observed = std::env::var_os("DRIVESYNC_BENCH_ACTIVITY").is_some_and(|value| value == "1");
    let diagnostics = if observed {
        RemoteActivity::new(&collection)
    } else {
        RemoteActivity::default()
    };
    let auth = self::session();
    let token = CancellationToken::new();
    let start = Instant::now();
    let mut mode = RemoteObserveMode::Initial;
    let result = loop {
        let result = observer
            .observe(request(mode, 16), &auth, &token, diagnostics)
            .await
            .unwrap();
        if !result.has_more {
            break result;
        }
        mode = RemoteObserveMode::Incremental;
        assert_eq!(result.coverage, ObservationCoverage::Partial);
        assert!(result.observations.is_empty());
    };
    assert_eq!(result.coverage, ObservationCoverage::Complete);
    assert_eq!(result.observations.len(), page_count * items_per_page);
    assert_eq!(server.requests.lock().unwrap().len(), page_count + 3);
    println!(
        "remote pages={page_count} items={} observed={observed} duration={:?} {}",
        result.observations.len(),
        start.elapsed(),
        super::super::super::scanner::tests::peak_memory()
    );
}
