//! `files_get` contract: `GET /files/{fileId}` (Discovery v3).

use std::time::Duration;

use crate::google_provider::support::{session, Framing, Reply, Server};
use drivesync_engine::providers::google::{
    FieldMask, FileGetOptions, FileId, GoogleApiError, GoogleClientLimits, GoogleSession,
};

fn id(raw: &str) -> FileId {
    FileId(raw.into())
}

fn limits(timeout_ms: u64, max_bytes: usize) -> GoogleClientLimits {
    GoogleClientLimits {
        request_timeout: Duration::from_millis(timeout_ms),
        max_metadata_response_bytes: max_bytes,
    }
}

/// FG-C01: minimal request: verb, path, bearer token, no body, no query.
#[tokio::test]
async fn sends_get_with_bearer_token_and_no_query_or_body() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"abc","name":"a"}"#)]);
    let file = server
        .client()
        .files_get(&session(), &id("abc"), FileGetOptions::default())
        .await
        .unwrap();
    assert_eq!(file.id, Some(id("abc")));
    let req = &server.recorded()[0];
    assert_eq!(req.method, "GET");
    assert_eq!(req.path(), "/drive/v3/files/abc");
    assert_eq!(req.target, "/drive/v3/files/abc");
    assert_eq!(req.header("authorization"), Some("Bearer tok-A-secret"));
    assert!(req.body.is_empty());
    assert!(!req.target.contains("tok-A-secret"), "token sent in the URL");
}

/// FG-C02: every option is sent under its official name, and nothing else is.
#[tokio::test]
async fn options_are_sent_as_official_query_parameters() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"abc"}"#)]);
    let options = FileGetOptions {
        fields: Some(FieldMask("id,name".into())),
        supports_all_drives: Some(true),
        include_labels: Some("l1,l2".into()),
        include_permissions_for_view: Some("published".into()),
    };
    server
        .client()
        .files_get(&session(), &id("abc"), options)
        .await
        .unwrap();
    let mut query = server.recorded()[0].decoded_query();
    query.sort();
    let pair = |k: &str, v: &str| (k.to_owned(), v.to_owned());
    assert_eq!(
        query,
        vec![
            pair("fields", "id,name"),
            pair("includeLabels", "l1,l2"),
            pair("includePermissionsForView", "published"),
            pair("supportsAllDrives", "true"),
        ]
    );
}

/// FG-C03: the file ID is percent-encoded as a single path segment.
#[tokio::test]
async fn file_id_is_percent_encoded_in_the_path() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"x"}"#)]);
    let _ = server
        .client()
        .files_get(&session(), &id("a/b c?d#e"), FileGetOptions::default())
        .await;
    let req = &server.recorded()[0];
    assert_eq!(req.path(), "/drive/v3/files/a%2Fb%20c%3Fd%23e");
    assert_eq!(req.target, req.path(), "reserved characters leaked to query");
}

/// FG-C04: each session sends its own token (no cross-account leakage).
#[tokio::test]
async fn each_session_uses_its_own_token() {
    let server = Server::start(vec![
        Reply::json(200, r#"{"id":"1"}"#),
        Reply::json(200, r#"{"id":"2"}"#),
    ]);
    let client = server.client();
    let f = id("f");
    client
        .files_get(
            &GoogleSession::new("a", "TOKEN-A"),
            &f,
            FileGetOptions::default(),
        )
        .await
        .unwrap();
    client
        .files_get(
            &GoogleSession::new("b", "TOKEN-B"),
            &f,
            FileGetOptions::default(),
        )
        .await
        .unwrap();
    let reqs = server.recorded();
    assert_eq!(reqs[0].header("authorization"), Some("Bearer TOKEN-A"));
    assert_eq!(reqs[1].header("authorization"), Some("Bearer TOKEN-B"));
}

/// FG-C05: HTTP errors are never returned as success.
#[tokio::test]
async fn http_errors_are_translated() {
    let cases = [
        (
            404,
            r#"{"error":{"errors":[{"reason":"notFound"}]}}"#,
            GoogleApiError::NotFoundOrInaccessible,
        ),
        (
            401,
            r#"{"error":{"errors":[{"reason":"authError"}]}}"#,
            GoogleApiError::AuthenticationRequired,
        ),
        (
            403,
            r#"{"error":{"errors":[{"reason":"insufficientFilePermissions"}]}}"#,
            GoogleApiError::PermissionDenied,
        ),
        (400, "", GoogleApiError::InvalidArgument),
        (503, "", GoogleApiError::ProviderUnavailable),
    ];
    for (status, body, kind) in cases {
        let server = Server::start(vec![Reply::json(status, body)]);
        let err = server
            .client()
            .files_get(&session(), &id("x"), FileGetOptions::default())
            .await
            .unwrap_err();
        assert_eq!(err.kind, kind, "status {status}");
        assert_eq!(err.http_status, Some(status));
        assert_eq!(server.recorded().len(), 1);
    }
}

/// FG-C06: 429 preserves `Retry-After` (seconds) once and is retryable.
#[tokio::test]
async fn rate_limit_preserves_retry_after() {
    let mut reply = Reply::json(
        429,
        r#"{"error":{"errors":[{"reason":"rateLimitExceeded"}]}}"#,
    );
    reply.headers.push(("Retry-After", "30".into()));
    let server = Server::start(vec![reply]);
    let err = server
        .client()
        .files_get(&session(), &id("x"), FileGetOptions::default())
        .await
        .unwrap_err();
    assert_eq!(err.kind, GoogleApiError::RateLimited);
    assert_eq!(err.retry_after_seconds, Some(30));
    assert!(err.retryable);
}

/// FG-C07: a 200 with an invalid body is a failure, not a partial success.
#[tokio::test]
async fn invalid_json_on_success_status_is_invalid_response() {
    let server = Server::start(vec![Reply::json(200, "{\"id\":")]);
    let err = server
        .client()
        .files_get(&session(), &id("x"), FileGetOptions::default())
        .await
        .unwrap_err();
    assert_eq!(err.kind, GoogleApiError::InvalidResponse);
    assert_eq!(server.recorded().len(), 1);
}

/// FG-C08: unreachable server yields `Network` without a status.
#[tokio::test]
async fn connection_failure_is_network_error() {
    let server = Server::start(vec![]);
    let client = server.client();
    drop(server);
    let err = client
        .files_get(&session(), &id("x"), FileGetOptions::default())
        .await
        .unwrap_err();
    assert_eq!(err.kind, GoogleApiError::Network);
    assert_eq!(err.http_status, None);
}

/// FG-C09: neither token nor provider text appears in errors.
#[tokio::test]
async fn errors_do_not_leak_token_or_body() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-name.pdf","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let err = server
        .client()
        .files_get(&session(), &id("x"), FileGetOptions::default())
        .await
        .unwrap_err();
    assert_eq!(server.recorded().len(), 1);
    let text = format!("{err:?}");
    assert!(
        !text.contains("tok-A-secret") && !text.contains("private-name"),
        "{text}"
    );
}

/// FG-C10: the session's Debug output never reveals the token.
#[test]
fn session_debug_redacts_token() {
    let text = format!("{:?}", session());
    assert!(!text.contains("tok-A-secret"), "{text}");
    assert!(text.contains("account-a"), "{text}");
}

/// FG-C11: a redirect is neither followed nor exposed; the credentials reach
/// only the original host.
#[tokio::test]
async fn redirects_are_not_followed_and_are_invalid_responses() {
    for status in [301u16, 302, 303, 307, 308] {
        let other = Server::start(vec![Reply::json(200, r#"{"id":"x"}"#)]);
        let mut redirect = Reply::json(status, "");
        redirect
            .headers
            .push(("Location", format!("{}/steal-me", other.base)));
        let server = Server::start(vec![redirect]);
        let err = server
            .client()
            .files_get(&session(), &id("x"), FileGetOptions::default())
            .await
            .unwrap_err();
        assert_eq!(err.kind, GoogleApiError::InvalidResponse, "{status}");
        assert_eq!(err.http_status, Some(status));
        assert!(!err.retryable);
        assert_eq!(server.recorded().len(), 1, "request was not sent");
        assert!(
            other.recorded().is_empty(),
            "credentials reached another host"
        );
        assert!(!format!("{err:?}").contains("steal-me"));
    }
}

/// FG-C12: IDs that would change the endpoint are rejected before any request.
#[tokio::test]
async fn unsafe_file_ids_are_rejected_without_a_request() {
    for raw in ["", ".", ".."] {
        let server = Server::start(vec![Reply::json(200, r#"{"id":"x"}"#)]);
        let err = server
            .client()
            .files_get(&session(), &id(raw), FileGetOptions::default())
            .await
            .unwrap_err();
        assert_eq!(err.kind, GoogleApiError::InvalidArgument, "{raw:?}");
        assert_eq!(err.http_status, None);
        assert!(server.recorded().is_empty(), "{raw:?} reached the server");
    }
}

/// FG-C13: a slow server produces `Timeout`, not a hang or a partial result.
#[tokio::test]
async fn slow_response_is_a_timeout() {
    let mut slow = Reply::json(200, r#"{"id":"x"}"#);
    slow.delay = Duration::from_millis(1500);
    let server = Server::start(vec![slow]);
    let err = server
        .client_with_limits(limits(200, 1024))
        .files_get(&session(), &id("x"), FileGetOptions::default())
        .await
        .unwrap_err();
    assert_eq!(err.kind, GoogleApiError::Timeout);
    assert_eq!(err.http_status, None);
}

/// FG-C14: a body above the limit is rejected, with or without `Content-Length`.
#[tokio::test]
async fn oversized_body_is_rejected() {
    let big = format!(r#"{{"id":"x","description":"{}"}}"#, "a".repeat(4096));
    for framing in [Framing::Exact, Framing::UntilClose] {
        let mut reply = Reply::json(200, &big);
        reply.framing = framing;
        let server = Server::start(vec![reply]);
        let err = server
            .client_with_limits(limits(5000, 1024))
            .files_get(&session(), &id("x"), FileGetOptions::default())
            .await
            .unwrap_err();
        assert_eq!(err.kind, GoogleApiError::InvalidResponse);
    }
}

/// FG-C15: a truncated body is never decoded as a complete resource.
#[tokio::test]
async fn truncated_body_is_a_network_failure() {
    let mut reply = Reply::json(200, r#"{"id":"x","name":"half"#);
    reply.framing = Framing::Declared(500);
    let server = Server::start(vec![reply]);
    let err = server
        .client()
        .files_get(&session(), &id("x"), FileGetOptions::default())
        .await
        .unwrap_err();
    assert_eq!(err.kind, GoogleApiError::Network);
}

/// FG-C16: an oversized error body does not hide the HTTP status.
#[tokio::test]
async fn oversized_error_body_is_still_translated_by_status() {
    let server = Server::start(vec![Reply::json(500, &"x".repeat(4096))]);
    let err = server
        .client_with_limits(limits(5000, 1024))
        .files_get(&session(), &id("x"), FileGetOptions::default())
        .await
        .unwrap_err();
    assert_eq!(err.kind, GoogleApiError::ProviderUnavailable);
    assert_eq!(err.http_status, Some(500));
}

/// FG-C17: a past `Retry-After` HTTP-date means "now".
#[tokio::test]
async fn retry_after_http_date_is_supported() {
    let mut reply = Reply::json(503, "");
    reply
        .headers
        .push(("Retry-After", "Thu, 01 Jan 1970 00:00:00 GMT".into()));
    let server = Server::start(vec![reply]);
    let err = server
        .client()
        .files_get(&session(), &id("x"), FileGetOptions::default())
        .await
        .unwrap_err();
    assert_eq!(err.kind, GoogleApiError::ProviderUnavailable);
    assert_eq!(err.retry_after_seconds, Some(0));
}

/// FG-C18: a token that cannot be a header value is rejected locally and never echoed.
#[tokio::test]
async fn invalid_token_is_rejected_before_sending() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"x"}"#)]);
    let bad = GoogleSession::new("a", "tok\r\nX-Injected: 1");
    let err = server
        .client()
        .files_get(&bad, &id("x"), FileGetOptions::default())
        .await
        .unwrap_err();
    assert_eq!(err.kind, GoogleApiError::AuthenticationRequired);
    assert_eq!(err.http_status, None);
    assert!(server.recorded().is_empty());
    assert!(!format!("{err:?}").contains("Injected"));
}

/// FG-C19: concurrent calls with different sessions never mix credentials.
#[tokio::test]
async fn concurrent_calls_keep_sessions_isolated() {
    let server = Server::start(vec![
        Reply::json(200, r#"{"id":"1"}"#),
        Reply::json(200, r#"{"id":"2"}"#),
    ]);
    let client = server.client();
    let (a, b) = (GoogleSession::new("a", "TOKEN-A"), GoogleSession::new("b", "TOKEN-B"));
    let (first, second) = tokio::join!(
        client.files_get(&a, &id("one"), FileGetOptions::default()),
        client.files_get(&b, &id("two"), FileGetOptions::default()),
    );
    first.unwrap();
    second.unwrap();
    for req in server.recorded() {
        let expected = if req.path().ends_with("/one") {
            "Bearer TOKEN-A"
        } else {
            "Bearer TOKEN-B"
        };
        assert_eq!(req.header("authorization"), Some(expected), "{}", req.target);
    }
}

/// FG-C20: dropping the future cancels the call and leaves the client usable.
#[tokio::test]
async fn dropped_call_does_not_poison_the_client() {
    let mut slow = Reply::json(200, r#"{"id":"slow"}"#);
    slow.delay = Duration::from_millis(800);
    let slow_server = Server::start(vec![slow]);
    let cancelled = tokio::time::timeout(
        Duration::from_millis(100),
        slow_server
            .client()
            .files_get(&session(), &id("x"), FileGetOptions::default()),
    )
    .await;
    assert!(cancelled.is_err(), "the call should still have been pending");

    let ok_server = Server::start(vec![Reply::json(200, r#"{"id":"ok"}"#)]);
    let file = ok_server
        .client()
        .files_get(&session(), &id("x"), FileGetOptions::default())
        .await
        .unwrap();
    assert_eq!(file.id, Some(id("ok")));
}
