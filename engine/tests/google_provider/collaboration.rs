//! HTTP contracts fixed from Drive v3 Discovery revision 20261106.
use crate::google_provider::support::{session, Reply, Server};
use drivesync_engine::providers::google::*;

fn assert_projection(actual: &serde_json::Value, expected: &serde_json::Value) {
    match expected {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                assert_projection(&actual[key], value);
            }
        }
        serde_json::Value::Array(values) => {
            assert_eq!(actual.as_array().unwrap().len(), values.len());
            for (a, b) in actual.as_array().unwrap().iter().zip(values) {
                assert_projection(a, b);
            }
        }
        _ => assert_eq!(actual, expected),
    }
}

#[tokio::test]
async fn access_proposals_resolve_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#""#)]);
    let client = server.client();
    client
        .access_proposals_resolve(
            &session(),
            &FileId("f".into()),
            &ProposalId("p".into()),
            ResolveAccessProposalRequest::default(),
        )
        .await
        .unwrap();
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(
        requests[0].path(),
        "/drive/v3/files/f/accessproposals/p:resolve"
    );
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer tok-A-secret")
    );
    assert!(!requests[0].body.is_empty());
    assert_eq!(
        requests[0]
            .decoded_query()
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(r#"{}"#).unwrap()
    );
}

#[tokio::test]
async fn access_proposals_resolve_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .access_proposals_resolve(
            &session(),
            &FileId("f".into()),
            &ProposalId("p".into()),
            ResolveAccessProposalRequest::default(),
        )
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn approvals_approve_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .approvals_approve(
            &session(),
            &FileId("f".into()),
            &ApprovalId("a".into()),
            ApproveApprovalRequest::default(),
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/approvals/a:approve");
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer tok-A-secret")
    );
    assert!(!requests[0].body.is_empty());
    assert_eq!(
        requests[0]
            .decoded_query()
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(r#"{}"#).unwrap()
    );
}

#[tokio::test]
async fn approvals_approve_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .approvals_approve(
            &session(),
            &FileId("f".into()),
            &ApprovalId("a".into()),
            ApproveApprovalRequest::default(),
        )
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn approvals_cancel_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .approvals_cancel(
            &session(),
            &FileId("f".into()),
            &ApprovalId("a".into()),
            CancelApprovalRequest::default(),
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/approvals/a:cancel");
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer tok-A-secret")
    );
    assert!(!requests[0].body.is_empty());
    assert_eq!(
        requests[0]
            .decoded_query()
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(r#"{}"#).unwrap()
    );
}

#[tokio::test]
async fn approvals_cancel_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .approvals_cancel(
            &session(),
            &FileId("f".into()),
            &ApprovalId("a".into()),
            CancelApprovalRequest::default(),
        )
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn approvals_comment_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .approvals_comment(
            &session(),
            &FileId("f".into()),
            &ApprovalId("a".into()),
            CommentApprovalRequest::default(),
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/approvals/a:comment");
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer tok-A-secret")
    );
    assert!(!requests[0].body.is_empty());
    assert_eq!(
        requests[0]
            .decoded_query()
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(r#"{}"#).unwrap()
    );
}

#[tokio::test]
async fn approvals_comment_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .approvals_comment(
            &session(),
            &FileId("f".into()),
            &ApprovalId("a".into()),
            CommentApprovalRequest::default(),
        )
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn approvals_decline_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .approvals_decline(
            &session(),
            &FileId("f".into()),
            &ApprovalId("a".into()),
            DeclineApprovalRequest::default(),
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/approvals/a:decline");
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer tok-A-secret")
    );
    assert!(!requests[0].body.is_empty());
    assert_eq!(
        requests[0]
            .decoded_query()
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(r#"{}"#).unwrap()
    );
}

#[tokio::test]
async fn approvals_decline_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .approvals_decline(
            &session(),
            &FileId("f".into()),
            &ApprovalId("a".into()),
            DeclineApprovalRequest::default(),
        )
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn approvals_reassign_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .approvals_reassign(
            &session(),
            &FileId("f".into()),
            &ApprovalId("a".into()),
            ReassignApprovalRequest::default(),
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/approvals/a:reassign");
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer tok-A-secret")
    );
    assert!(!requests[0].body.is_empty());
    assert_eq!(
        requests[0]
            .decoded_query()
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(r#"{}"#).unwrap()
    );
}

#[tokio::test]
async fn approvals_reassign_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .approvals_reassign(
            &session(),
            &FileId("f".into()),
            &ApprovalId("a".into()),
            ReassignApprovalRequest::default(),
        )
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn approvals_start_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .approvals_start(
            &session(),
            &FileId("f".into()),
            StartApprovalRequest::default(),
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/approvals:start");
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer tok-A-secret")
    );
    assert!(!requests[0].body.is_empty());
    assert_eq!(
        requests[0]
            .decoded_query()
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(r#"{}"#).unwrap()
    );
}

#[tokio::test]
async fn approvals_start_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .approvals_start(
            &session(),
            &FileId("f".into()),
            StartApprovalRequest::default(),
        )
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn changes_watch_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .changes_watch(
            &session(),
            ChangeWatchOptions {
                page_token: "cursor".into(),
                ..Default::default()
            },
            GoogleChannelRequest {
                id: "channel-id".into(),
                type_: "web_hook".into(),
                address: "https://example.test/notify".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path(), "/drive/v3/changes/watch");
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer tok-A-secret")
    );
    assert!(!requests[0].body.is_empty());
    assert_eq!(
        requests[0]
            .decoded_query()
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(
            r#"{"pageToken": "cursor"}"#
        )
        .unwrap()
    );
}

#[tokio::test]
async fn changes_watch_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .changes_watch(
            &session(),
            ChangeWatchOptions {
                page_token: "cursor".into(),
                ..Default::default()
            },
            GoogleChannelRequest {
                id: "channel-id".into(),
                type_: "web_hook".into(),
                address: "https://example.test/notify".into(),
                ..Default::default()
            },
        )
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn changes_watch_option_drive_id() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .changes_watch(
            &session(),
            ChangeWatchOptions {
                page_token: "cursor".into(),
                drive_id: Some(DriveId("value".into())),
                ..Default::default()
            },
            GoogleChannelRequest {
                id: "channel-id".into(),
                type_: "web_hook".into(),
                address: "https://example.test/notify".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "driveId")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn changes_watch_option_page_size() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .changes_watch(
            &session(),
            ChangeWatchOptions {
                page_token: "cursor".into(),
                page_size: Some(2),
                ..Default::default()
            },
            GoogleChannelRequest {
                id: "channel-id".into(),
                type_: "web_hook".into(),
                address: "https://example.test/notify".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "pageSize")
            .map(|(_, value)| value.as_str()),
        Some("2")
    );
}

#[tokio::test]
async fn changes_watch_option_spaces() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .changes_watch(
            &session(),
            ChangeWatchOptions {
                page_token: "cursor".into(),
                spaces: Some("value".into()),
                ..Default::default()
            },
            GoogleChannelRequest {
                id: "channel-id".into(),
                type_: "web_hook".into(),
                address: "https://example.test/notify".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "spaces")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn changes_watch_option_include_removed() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .changes_watch(
            &session(),
            ChangeWatchOptions {
                page_token: "cursor".into(),
                include_removed: Some(false),
                ..Default::default()
            },
            GoogleChannelRequest {
                id: "channel-id".into(),
                type_: "web_hook".into(),
                address: "https://example.test/notify".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "includeRemoved")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn changes_watch_option_include_items_from_all_drives() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .changes_watch(
            &session(),
            ChangeWatchOptions {
                page_token: "cursor".into(),
                include_items_from_all_drives: Some(false),
                ..Default::default()
            },
            GoogleChannelRequest {
                id: "channel-id".into(),
                type_: "web_hook".into(),
                address: "https://example.test/notify".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "includeItemsFromAllDrives")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn changes_watch_option_supports_all_drives() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .changes_watch(
            &session(),
            ChangeWatchOptions {
                page_token: "cursor".into(),
                supports_all_drives: Some(false),
                ..Default::default()
            },
            GoogleChannelRequest {
                id: "channel-id".into(),
                type_: "web_hook".into(),
                address: "https://example.test/notify".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "supportsAllDrives")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn changes_watch_option_fields() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .changes_watch(
            &session(),
            ChangeWatchOptions {
                page_token: "cursor".into(),
                fields: Some(FieldMask("value".into())),
                ..Default::default()
            },
            GoogleChannelRequest {
                id: "channel-id".into(),
                type_: "web_hook".into(),
                address: "https://example.test/notify".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "fields")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn channels_stop_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#""#)]);
    let client = server.client();
    client
        .channels_stop(
            &session(),
            GoogleChannelStopRequest {
                id: ChannelId("channel-id".into()),
                resource_id: "resource-id".into(),
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path(), "/drive/v3/channels/stop");
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer tok-A-secret")
    );
    assert!(!requests[0].body.is_empty());
    assert_eq!(
        requests[0]
            .decoded_query()
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(r#"{}"#).unwrap()
    );
}

#[tokio::test]
async fn channels_stop_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .channels_stop(
            &session(),
            GoogleChannelStopRequest {
                id: ChannelId("channel-id".into()),
                resource_id: "resource-id".into(),
            },
        )
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn comments_create_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .comments_create(
            &session(),
            &FileId("f".into()),
            GoogleCommentWrite {
                content: GooglePatch::Set("comment".into()),
                ..Default::default()
            },
            CommentOptions {
                fields: Some(FieldMask("id".into())),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/comments");
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer tok-A-secret")
    );
    assert!(!requests[0].body.is_empty());
    assert_eq!(
        requests[0]
            .decoded_query()
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(r#"{"fields": "id"}"#)
            .unwrap()
    );
}

#[tokio::test]
async fn comments_create_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .comments_create(
            &session(),
            &FileId("f".into()),
            GoogleCommentWrite {
                content: GooglePatch::Set("comment".into()),
                ..Default::default()
            },
            CommentOptions {
                fields: Some(FieldMask("id".into())),
                ..Default::default()
            },
        )
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn comments_create_option_fields() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .comments_create(
            &session(),
            &FileId("f".into()),
            GoogleCommentWrite {
                content: GooglePatch::Set("comment".into()),
                ..Default::default()
            },
            CommentOptions {
                fields: Some(FieldMask("value".into())),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "fields")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn comments_create_option_include_deleted() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .comments_create(
            &session(),
            &FileId("f".into()),
            GoogleCommentWrite {
                content: GooglePatch::Set("comment".into()),
                ..Default::default()
            },
            CommentOptions {
                fields: Some(FieldMask("id".into())),
                include_deleted: Some(false),
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "includeDeleted")
            .map(|(_, value)| value.as_str()),
        None
    );
}

#[tokio::test]
async fn comments_delete_wire_contract() {
    let server = Server::start(vec![Reply::json(204, r#""#)]);
    let client = server.client();
    client
        .comments_delete(&session(), &FileId("f".into()), &CommentId("c".into()))
        .await
        .unwrap();
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "DELETE");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/comments/c");
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer tok-A-secret")
    );
    assert!(requests[0].body.is_empty());
    assert_eq!(
        requests[0]
            .decoded_query()
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(r#"{}"#).unwrap()
    );
}

#[tokio::test]
async fn comments_delete_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .comments_delete(&session(), &FileId("f".into()), &CommentId("c".into()))
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn comments_update_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .comments_update(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
            GoogleCommentWrite {
                content: GooglePatch::Set("comment".into()),
                ..Default::default()
            },
            CommentOptions {
                fields: Some(FieldMask("id".into())),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "PATCH");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/comments/c");
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer tok-A-secret")
    );
    assert!(!requests[0].body.is_empty());
    assert_eq!(
        requests[0]
            .decoded_query()
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(r#"{"fields": "id"}"#)
            .unwrap()
    );
}

#[tokio::test]
async fn comments_update_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .comments_update(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
            GoogleCommentWrite {
                content: GooglePatch::Set("comment".into()),
                ..Default::default()
            },
            CommentOptions {
                fields: Some(FieldMask("id".into())),
                ..Default::default()
            },
        )
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn comments_update_option_fields() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .comments_update(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
            GoogleCommentWrite {
                content: GooglePatch::Set("comment".into()),
                ..Default::default()
            },
            CommentOptions {
                fields: Some(FieldMask("value".into())),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "fields")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn comments_update_option_include_deleted() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .comments_update(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
            GoogleCommentWrite {
                content: GooglePatch::Set("comment".into()),
                ..Default::default()
            },
            CommentOptions {
                fields: Some(FieldMask("id".into())),
                include_deleted: Some(false),
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "includeDeleted")
            .map(|(_, value)| value.as_str()),
        None
    );
}

#[tokio::test]
async fn permissions_create_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"p","role":"reader"}"#)]);
    let client = server.client();
    let result = client
        .permissions_create(
            &session(),
            &FileId("f".into()),
            GooglePermissionWrite::default(),
            PermissionCreateOptions::default(),
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{"id":"p","role":"reader"}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/permissions");
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer tok-A-secret")
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&requests[0].body).unwrap(),
        serde_json::json!({})
    );
    assert_eq!(
        requests[0]
            .decoded_query()
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(r#"{}"#).unwrap()
    );
}

#[tokio::test]
async fn permissions_create_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .permissions_create(
            &session(),
            &FileId("f".into()),
            GooglePermissionWrite::default(),
            PermissionCreateOptions::default(),
        )
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn permissions_create_option_send_notification_email() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"p","role":"reader"}"#)]);
    let client = server.client();
    client
        .permissions_create(
            &session(),
            &FileId("f".into()),
            GooglePermissionWrite::default(),
            PermissionCreateOptions {
                send_notification_email: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "sendNotificationEmail")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn permissions_create_option_email_message() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"p","role":"reader"}"#)]);
    let client = server.client();
    client
        .permissions_create(
            &session(),
            &FileId("f".into()),
            GooglePermissionWrite::default(),
            PermissionCreateOptions {
                email_message: Some("value".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "emailMessage")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn permissions_create_option_supports_all_drives() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"p","role":"reader"}"#)]);
    let client = server.client();
    client
        .permissions_create(
            &session(),
            &FileId("f".into()),
            GooglePermissionWrite::default(),
            PermissionCreateOptions {
                supports_all_drives: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "supportsAllDrives")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn permissions_create_option_transfer_ownership() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"p","role":"reader"}"#)]);
    let client = server.client();
    client
        .permissions_create(
            &session(),
            &FileId("f".into()),
            GooglePermissionWrite::default(),
            PermissionCreateOptions {
                transfer_ownership: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "transferOwnership")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn permissions_create_option_use_domain_admin_access() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"p","role":"reader"}"#)]);
    let client = server.client();
    client
        .permissions_create(
            &session(),
            &FileId("f".into()),
            GooglePermissionWrite::default(),
            PermissionCreateOptions {
                use_domain_admin_access: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "useDomainAdminAccess")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn permissions_create_option_fields() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"p","role":"reader"}"#)]);
    let client = server.client();
    client
        .permissions_create(
            &session(),
            &FileId("f".into()),
            GooglePermissionWrite::default(),
            PermissionCreateOptions {
                fields: Some(FieldMask("value".into())),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "fields")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn permissions_delete_wire_contract() {
    let server = Server::start(vec![Reply::json(204, r#""#)]);
    let client = server.client();
    client
        .permissions_delete(
            &session(),
            &FileId("f".into()),
            &PermissionId("p".into()),
            PermissionDeleteOptions::default(),
        )
        .await
        .unwrap();
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "DELETE");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/permissions/p");
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer tok-A-secret")
    );
    assert!(requests[0].body.is_empty());
    assert_eq!(
        requests[0]
            .decoded_query()
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(r#"{}"#).unwrap()
    );
}

#[tokio::test]
async fn permissions_delete_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .permissions_delete(
            &session(),
            &FileId("f".into()),
            &PermissionId("p".into()),
            PermissionDeleteOptions::default(),
        )
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn permissions_delete_option_supports_all_drives() {
    let server = Server::start(vec![Reply::json(204, r#""#)]);
    let client = server.client();
    client
        .permissions_delete(
            &session(),
            &FileId("f".into()),
            &PermissionId("p".into()),
            PermissionDeleteOptions {
                supports_all_drives: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "supportsAllDrives")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn permissions_delete_option_use_domain_admin_access() {
    let server = Server::start(vec![Reply::json(204, r#""#)]);
    let client = server.client();
    client
        .permissions_delete(
            &session(),
            &FileId("f".into()),
            &PermissionId("p".into()),
            PermissionDeleteOptions {
                use_domain_admin_access: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "useDomainAdminAccess")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn permissions_update_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"p","role":"reader"}"#)]);
    let client = server.client();
    let result = client
        .permissions_update(
            &session(),
            &FileId("f".into()),
            &PermissionId("p".into()),
            GooglePermissionWrite::default(),
            PermissionUpdateOptions::default(),
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{"id":"p","role":"reader"}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "PATCH");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/permissions/p");
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer tok-A-secret")
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&requests[0].body).unwrap(),
        serde_json::json!({})
    );
    assert_eq!(
        requests[0]
            .decoded_query()
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(r#"{}"#).unwrap()
    );
}

#[tokio::test]
async fn permissions_update_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .permissions_update(
            &session(),
            &FileId("f".into()),
            &PermissionId("p".into()),
            GooglePermissionWrite::default(),
            PermissionUpdateOptions::default(),
        )
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn permissions_update_option_supports_all_drives() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"p","role":"reader"}"#)]);
    let client = server.client();
    client
        .permissions_update(
            &session(),
            &FileId("f".into()),
            &PermissionId("p".into()),
            GooglePermissionWrite::default(),
            PermissionUpdateOptions {
                supports_all_drives: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "supportsAllDrives")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn permissions_update_option_use_domain_admin_access() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"p","role":"reader"}"#)]);
    let client = server.client();
    client
        .permissions_update(
            &session(),
            &FileId("f".into()),
            &PermissionId("p".into()),
            GooglePermissionWrite::default(),
            PermissionUpdateOptions {
                use_domain_admin_access: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "useDomainAdminAccess")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn permissions_update_option_remove_expiration() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"p","role":"reader"}"#)]);
    let client = server.client();
    client
        .permissions_update(
            &session(),
            &FileId("f".into()),
            &PermissionId("p".into()),
            GooglePermissionWrite::default(),
            PermissionUpdateOptions {
                remove_expiration: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "removeExpiration")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn permissions_update_option_transfer_ownership() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"p","role":"reader"}"#)]);
    let client = server.client();
    client
        .permissions_update(
            &session(),
            &FileId("f".into()),
            &PermissionId("p".into()),
            GooglePermissionWrite::default(),
            PermissionUpdateOptions {
                transfer_ownership: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "transferOwnership")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn permissions_update_option_fields() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"p","role":"reader"}"#)]);
    let client = server.client();
    client
        .permissions_update(
            &session(),
            &FileId("f".into()),
            &PermissionId("p".into()),
            GooglePermissionWrite::default(),
            PermissionUpdateOptions {
                fields: Some(FieldMask("value".into())),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "fields")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn replies_create_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .replies_create(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
            GoogleReplyWrite {
                content: GooglePatch::Set("comment".into()),
                ..Default::default()
            },
            ReplyOptions {
                fields: Some(FieldMask("id".into())),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/comments/c/replies");
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer tok-A-secret")
    );
    assert!(!requests[0].body.is_empty());
    assert_eq!(
        requests[0]
            .decoded_query()
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(r#"{"fields": "id"}"#)
            .unwrap()
    );
}

#[tokio::test]
async fn replies_create_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .replies_create(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
            GoogleReplyWrite {
                content: GooglePatch::Set("comment".into()),
                ..Default::default()
            },
            ReplyOptions {
                fields: Some(FieldMask("id".into())),
                ..Default::default()
            },
        )
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn replies_create_option_fields() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .replies_create(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
            GoogleReplyWrite {
                content: GooglePatch::Set("comment".into()),
                ..Default::default()
            },
            ReplyOptions {
                fields: Some(FieldMask("value".into())),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "fields")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn replies_create_option_include_deleted() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .replies_create(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
            GoogleReplyWrite {
                content: GooglePatch::Set("comment".into()),
                ..Default::default()
            },
            ReplyOptions {
                fields: Some(FieldMask("id".into())),
                include_deleted: Some(false),
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "includeDeleted")
            .map(|(_, value)| value.as_str()),
        None
    );
}

#[tokio::test]
async fn replies_delete_wire_contract() {
    let server = Server::start(vec![Reply::json(204, r#""#)]);
    let client = server.client();
    client
        .replies_delete(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
            &ReplyId("r".into()),
        )
        .await
        .unwrap();
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "DELETE");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/comments/c/replies/r");
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer tok-A-secret")
    );
    assert!(requests[0].body.is_empty());
    assert_eq!(
        requests[0]
            .decoded_query()
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(r#"{}"#).unwrap()
    );
}

#[tokio::test]
async fn replies_delete_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .replies_delete(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
            &ReplyId("r".into()),
        )
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn replies_update_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .replies_update(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
            &ReplyId("r".into()),
            GoogleReplyWrite {
                content: GooglePatch::Set("comment".into()),
                ..Default::default()
            },
            ReplyOptions {
                fields: Some(FieldMask("id".into())),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "PATCH");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/comments/c/replies/r");
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer tok-A-secret")
    );
    assert!(!requests[0].body.is_empty());
    assert_eq!(
        requests[0]
            .decoded_query()
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(r#"{"fields": "id"}"#)
            .unwrap()
    );
}

#[tokio::test]
async fn replies_update_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .replies_update(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
            &ReplyId("r".into()),
            GoogleReplyWrite {
                content: GooglePatch::Set("comment".into()),
                ..Default::default()
            },
            ReplyOptions {
                fields: Some(FieldMask("id".into())),
                ..Default::default()
            },
        )
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn replies_update_option_fields() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .replies_update(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
            &ReplyId("r".into()),
            GoogleReplyWrite {
                content: GooglePatch::Set("comment".into()),
                ..Default::default()
            },
            ReplyOptions {
                fields: Some(FieldMask("value".into())),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "fields")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn replies_update_option_include_deleted() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .replies_update(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
            &ReplyId("r".into()),
            GoogleReplyWrite {
                content: GooglePatch::Set("comment".into()),
                ..Default::default()
            },
            ReplyOptions {
                fields: Some(FieldMask("id".into())),
                include_deleted: Some(false),
            },
        )
        .await
        .unwrap();
    let requests = server.recorded();
    let query = requests[0].decoded_query();
    assert_eq!(
        query
            .iter()
            .find(|(key, _)| key == "includeDeleted")
            .map(|(_, value)| value.as_str()),
        None
    );
}

#[tokio::test]
async fn revisions_delete_wire_contract() {
    let server = Server::start(vec![Reply::json(204, r#""#)]);
    let client = server.client();
    client
        .revisions_delete(&session(), &FileId("f".into()), &RevisionId("v".into()))
        .await
        .unwrap();
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "DELETE");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/revisions/v");
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer tok-A-secret")
    );
    assert!(requests[0].body.is_empty());
    assert_eq!(
        requests[0]
            .decoded_query()
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(r#"{}"#).unwrap()
    );
}

#[tokio::test]
async fn revisions_delete_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .revisions_delete(&session(), &FileId("f".into()), &RevisionId("v".into()))
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn revisions_update_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .revisions_update(
            &session(),
            &FileId("f".into()),
            &RevisionId("v".into()),
            GoogleRevisionWrite::default(),
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "PATCH");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/revisions/v");
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer tok-A-secret")
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&requests[0].body).unwrap(),
        serde_json::json!({})
    );
    assert_eq!(
        requests[0]
            .decoded_query()
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(r#"{}"#).unwrap()
    );
}

#[tokio::test]
async fn revisions_update_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .revisions_update(
            &session(),
            &FileId("f".into()),
            &RevisionId("v".into()),
            GoogleRevisionWrite::default(),
        )
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}
