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
async fn about_get_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .about_get(&session(), AboutOptions::default())
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path(), "/drive/v3/about");
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
async fn about_get_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .about_get(&session(), AboutOptions::default())
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn about_get_option_fields() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .about_get(
            &session(),
            AboutOptions {
                fields: Some(FieldMask("value".into())),
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
async fn access_proposals_get_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .access_proposals_get(&session(), &FileId("f".into()), &ProposalId("p".into()))
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/accessproposals/p");
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
async fn access_proposals_get_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .access_proposals_get(&session(), &FileId("f".into()), &ProposalId("p".into()))
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn access_proposals_list_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .access_proposals_list(&session(), &FileId("f".into()), PageOptions::default())
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/accessproposals");
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
async fn access_proposals_list_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .access_proposals_list(&session(), &FileId("f".into()), PageOptions::default())
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn access_proposals_list_option_page_size() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .access_proposals_list(
            &session(),
            &FileId("f".into()),
            PageOptions {
                page_size: Some(2),
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
async fn access_proposals_list_option_page_token() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .access_proposals_list(
            &session(),
            &FileId("f".into()),
            PageOptions {
                page_token: Some("value".into()),
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
            .find(|(key, _)| key == "pageToken")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn access_proposals_list_option_fields() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .access_proposals_list(
            &session(),
            &FileId("f".into()),
            PageOptions {
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
async fn approvals_get_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .approvals_get(&session(), &FileId("f".into()), &ApprovalId("a".into()))
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/approvals/a");
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
async fn approvals_get_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .approvals_get(&session(), &FileId("f".into()), &ApprovalId("a".into()))
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn approvals_list_wire_contract() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"items":[{"approvalId":"a"}],"nextPageToken":"next"}"#,
    )]);
    let client = server.client();
    let result = client
        .approvals_list(&session(), &FileId("f".into()), PageOptions::default())
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(
            r#"{"items":[{"approvalId":"a"}],"nextPageToken":"next"}"#,
        )
        .unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/approvals");
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
async fn approvals_list_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .approvals_list(&session(), &FileId("f".into()), PageOptions::default())
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn approvals_list_option_page_size() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"items":[{"approvalId":"a"}],"nextPageToken":"next"}"#,
    )]);
    let client = server.client();
    client
        .approvals_list(
            &session(),
            &FileId("f".into()),
            PageOptions {
                page_size: Some(2),
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
async fn approvals_list_option_page_token() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"items":[{"approvalId":"a"}],"nextPageToken":"next"}"#,
    )]);
    let client = server.client();
    client
        .approvals_list(
            &session(),
            &FileId("f".into()),
            PageOptions {
                page_token: Some("value".into()),
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
            .find(|(key, _)| key == "pageToken")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn approvals_list_option_fields() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"items":[{"approvalId":"a"}],"nextPageToken":"next"}"#,
    )]);
    let client = server.client();
    client
        .approvals_list(
            &session(),
            &FileId("f".into()),
            PageOptions {
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
async fn apps_get_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .apps_get(&session(), &AppId("app".into()))
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path(), "/drive/v3/apps/app");
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
async fn apps_get_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .apps_get(&session(), &AppId("app".into()))
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn apps_list_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .apps_list(&session(), AppListOptions::default())
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path(), "/drive/v3/apps");
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
async fn apps_list_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .apps_list(&session(), AppListOptions::default())
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn apps_list_option_app_filter_extensions() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .apps_list(
            &session(),
            AppListOptions {
                app_filter_extensions: Some(vec!["value".into()]),
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
            .find(|(key, _)| key == "appFilterExtensions")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn apps_list_option_app_filter_mime_types() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .apps_list(
            &session(),
            AppListOptions {
                app_filter_mime_types: Some(vec!["value".into()]),
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
            .find(|(key, _)| key == "appFilterMimeTypes")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn apps_list_option_language_code() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .apps_list(
            &session(),
            AppListOptions {
                language_code: Some("value".into()),
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
            .find(|(key, _)| key == "languageCode")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn apps_list_option_fields() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .apps_list(
            &session(),
            AppListOptions {
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
async fn changes_get_start_page_token_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{"startPageToken":"cursor"}"#)]);
    let client = server.client();
    let result = client
        .changes_get_start_page_token(&session(), ChangeScopeOptions::default())
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{"startPageToken":"cursor"}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path(), "/drive/v3/changes/startPageToken");
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
async fn changes_get_start_page_token_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .changes_get_start_page_token(&session(), ChangeScopeOptions::default())
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn changes_get_start_page_token_option_drive_id() {
    let server = Server::start(vec![Reply::json(200, r#"{"startPageToken":"cursor"}"#)]);
    let client = server.client();
    client
        .changes_get_start_page_token(
            &session(),
            ChangeScopeOptions {
                drive_id: Some(DriveId("value".into())),
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
async fn changes_get_start_page_token_option_supports_all_drives() {
    let server = Server::start(vec![Reply::json(200, r#"{"startPageToken":"cursor"}"#)]);
    let client = server.client();
    client
        .changes_get_start_page_token(
            &session(),
            ChangeScopeOptions {
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
async fn changes_list_wire_contract() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"changes":[{"fileId":"f","removed":true}],"newStartPageToken":"end"}"#,
    )]);
    let client = server.client();
    let result = client
        .changes_list(
            &session(),
            ChangeListOptions {
                page_token: "cursor".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(
            r#"{"changes":[{"fileId":"f","removed":true}],"newStartPageToken":"end"}"#,
        )
        .unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path(), "/drive/v3/changes");
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
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(
            r#"{"pageToken": "cursor"}"#
        )
        .unwrap()
    );
}

#[tokio::test]
async fn changes_list_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .changes_list(
            &session(),
            ChangeListOptions {
                page_token: "cursor".into(),
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
async fn changes_list_option_drive_id() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"changes":[{"fileId":"f","removed":true}],"newStartPageToken":"end"}"#,
    )]);
    let client = server.client();
    client
        .changes_list(
            &session(),
            ChangeListOptions {
                page_token: "cursor".into(),
                drive_id: Some(DriveId("value".into())),
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
async fn changes_list_option_page_size() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"changes":[{"fileId":"f","removed":true}],"newStartPageToken":"end"}"#,
    )]);
    let client = server.client();
    client
        .changes_list(
            &session(),
            ChangeListOptions {
                page_token: "cursor".into(),
                page_size: Some(2),
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
async fn changes_list_option_spaces() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"changes":[{"fileId":"f","removed":true}],"newStartPageToken":"end"}"#,
    )]);
    let client = server.client();
    client
        .changes_list(
            &session(),
            ChangeListOptions {
                page_token: "cursor".into(),
                spaces: Some("value".into()),
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
async fn changes_list_option_include_removed() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"changes":[{"fileId":"f","removed":true}],"newStartPageToken":"end"}"#,
    )]);
    let client = server.client();
    client
        .changes_list(
            &session(),
            ChangeListOptions {
                page_token: "cursor".into(),
                include_removed: Some(false),
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
async fn changes_list_option_include_corpus_removals() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"changes":[{"fileId":"f","removed":true}],"newStartPageToken":"end"}"#,
    )]);
    let client = server.client();
    client
        .changes_list(
            &session(),
            ChangeListOptions {
                page_token: "cursor".into(),
                include_corpus_removals: Some(false),
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
            .find(|(key, _)| key == "includeCorpusRemovals")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn changes_list_option_include_items_from_all_drives() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"changes":[{"fileId":"f","removed":true}],"newStartPageToken":"end"}"#,
    )]);
    let client = server.client();
    client
        .changes_list(
            &session(),
            ChangeListOptions {
                page_token: "cursor".into(),
                include_items_from_all_drives: Some(false),
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
async fn changes_list_option_supports_all_drives() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"changes":[{"fileId":"f","removed":true}],"newStartPageToken":"end"}"#,
    )]);
    let client = server.client();
    client
        .changes_list(
            &session(),
            ChangeListOptions {
                page_token: "cursor".into(),
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
async fn changes_list_option_fields() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"changes":[{"fileId":"f","removed":true}],"newStartPageToken":"end"}"#,
    )]);
    let client = server.client();
    client
        .changes_list(
            &session(),
            ChangeListOptions {
                page_token: "cursor".into(),
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
async fn comments_get_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .comments_get(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
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
    assert_eq!(requests[0].method, "GET");
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
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(r#"{"fields": "id"}"#)
            .unwrap()
    );
}

#[tokio::test]
async fn comments_get_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .comments_get(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
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
async fn comments_get_option_fields() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .comments_get(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
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
async fn comments_get_option_include_deleted() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .comments_get(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
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
        Some("false")
    );
}

#[tokio::test]
async fn comments_list_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .comments_list(
            &session(),
            &FileId("f".into()),
            CommentListOptions {
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
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/comments");
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
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(r#"{"fields": "id"}"#)
            .unwrap()
    );
}

#[tokio::test]
async fn comments_list_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .comments_list(
            &session(),
            &FileId("f".into()),
            CommentListOptions {
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
async fn comments_list_option_fields() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .comments_list(
            &session(),
            &FileId("f".into()),
            CommentListOptions {
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
async fn comments_list_option_page_size() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .comments_list(
            &session(),
            &FileId("f".into()),
            CommentListOptions {
                fields: Some(FieldMask("id".into())),
                page_size: Some(2),
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
async fn comments_list_option_page_token() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .comments_list(
            &session(),
            &FileId("f".into()),
            CommentListOptions {
                fields: Some(FieldMask("id".into())),
                page_token: Some("value".into()),
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
            .find(|(key, _)| key == "pageToken")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn comments_list_option_include_deleted() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .comments_list(
            &session(),
            &FileId("f".into()),
            CommentListOptions {
                fields: Some(FieldMask("id".into())),
                include_deleted: Some(false),
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
            .find(|(key, _)| key == "includeDeleted")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn comments_list_option_start_modified_time() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .comments_list(
            &session(),
            &FileId("f".into()),
            CommentListOptions {
                fields: Some(FieldMask("id".into())),
                start_modified_time: Some("value".into()),
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
            .find(|(key, _)| key == "startModifiedTime")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn drives_get_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"d","name":"sample"}"#)]);
    let client = server.client();
    let result = client
        .drives_get(&session(), &DriveId("d".into()), DriveGetOptions::default())
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{"id":"d","name":"sample"}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path(), "/drive/v3/drives/d");
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
async fn drives_get_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .drives_get(&session(), &DriveId("d".into()), DriveGetOptions::default())
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn drives_get_option_use_domain_admin_access() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"d","name":"sample"}"#)]);
    let client = server.client();
    client
        .drives_get(
            &session(),
            &DriveId("d".into()),
            DriveGetOptions {
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
async fn drives_get_option_fields() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"d","name":"sample"}"#)]);
    let client = server.client();
    client
        .drives_get(
            &session(),
            &DriveId("d".into()),
            DriveGetOptions {
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
async fn drives_list_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .drives_list(&session(), DriveListOptions::default())
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path(), "/drive/v3/drives");
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
async fn drives_list_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .drives_list(&session(), DriveListOptions::default())
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn drives_list_option_page_size() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .drives_list(
            &session(),
            DriveListOptions {
                page_size: Some(2),
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
async fn drives_list_option_page_token() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .drives_list(
            &session(),
            DriveListOptions {
                page_token: Some("value".into()),
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
            .find(|(key, _)| key == "pageToken")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn drives_list_option_q() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .drives_list(
            &session(),
            DriveListOptions {
                q: Some("value".into()),
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
            .find(|(key, _)| key == "q")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn drives_list_option_use_domain_admin_access() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .drives_list(
            &session(),
            DriveListOptions {
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
async fn drives_list_option_fields() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .drives_list(
            &session(),
            DriveListOptions {
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
async fn files_generate_cse_token_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .files_generate_cse_token(&session(), CseTokenOptions::default())
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path(), "/drive/v3/files/generateCseToken");
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
async fn files_generate_cse_token_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .files_generate_cse_token(&session(), CseTokenOptions::default())
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn files_generate_cse_token_option_file_id() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .files_generate_cse_token(
            &session(),
            CseTokenOptions {
                file_id: Some(FileId("value".into())),
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
            .find(|(key, _)| key == "fileId")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn files_generate_cse_token_option_parent() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .files_generate_cse_token(
            &session(),
            CseTokenOptions {
                parent: Some(FileId("value".into())),
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
            .find(|(key, _)| key == "parent")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn files_generate_ids_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .files_generate_ids(&session(), GenerateIdsOptions::default())
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path(), "/drive/v3/files/generateIds");
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
async fn files_generate_ids_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .files_generate_ids(&session(), GenerateIdsOptions::default())
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn files_generate_ids_option_count() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .files_generate_ids(
            &session(),
            GenerateIdsOptions {
                count: Some(2),
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
            .find(|(key, _)| key == "count")
            .map(|(_, value)| value.as_str()),
        Some("2")
    );
}

#[tokio::test]
async fn files_generate_ids_option_space() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .files_generate_ids(
            &session(),
            GenerateIdsOptions {
                space: Some("value".into()),
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
            .find(|(key, _)| key == "space")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn files_generate_ids_option_type_() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .files_generate_ids(
            &session(),
            GenerateIdsOptions {
                type_: Some("value".into()),
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
            .find(|(key, _)| key == "type")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn files_list_wire_contract() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"files":[{"id":"f","name":"sample"}],"nextPageToken":"next","incompleteSearch":false}"#,
    )]);
    let client = server.client();
    let result = client
        .files_list(&session(), FileListOptions::default())
        .await
        .unwrap();
    assert_projection(&serde_json::to_value(&result).unwrap(), &serde_json::from_str::<serde_json::Value>(r#"{"files":[{"id":"f","name":"sample"}],"nextPageToken":"next","incompleteSearch":false}"#).unwrap());
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path(), "/drive/v3/files");
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
async fn files_list_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .files_list(&session(), FileListOptions::default())
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn files_list_option_q() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"files":[{"id":"f","name":"sample"}],"nextPageToken":"next","incompleteSearch":false}"#,
    )]);
    let client = server.client();
    client
        .files_list(
            &session(),
            FileListOptions {
                q: Some("value".into()),
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
            .find(|(key, _)| key == "q")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn files_list_option_corpora() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"files":[{"id":"f","name":"sample"}],"nextPageToken":"next","incompleteSearch":false}"#,
    )]);
    let client = server.client();
    client
        .files_list(
            &session(),
            FileListOptions {
                corpora: Some("value".into()),
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
            .find(|(key, _)| key == "corpora")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn files_list_option_drive_id() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"files":[{"id":"f","name":"sample"}],"nextPageToken":"next","incompleteSearch":false}"#,
    )]);
    let client = server.client();
    client
        .files_list(
            &session(),
            FileListOptions {
                drive_id: Some(DriveId("value".into())),
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
async fn files_list_option_page_size() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"files":[{"id":"f","name":"sample"}],"nextPageToken":"next","incompleteSearch":false}"#,
    )]);
    let client = server.client();
    client
        .files_list(
            &session(),
            FileListOptions {
                page_size: Some(2),
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
async fn files_list_option_page_token() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"files":[{"id":"f","name":"sample"}],"nextPageToken":"next","incompleteSearch":false}"#,
    )]);
    let client = server.client();
    client
        .files_list(
            &session(),
            FileListOptions {
                page_token: Some("value".into()),
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
            .find(|(key, _)| key == "pageToken")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn files_list_option_spaces() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"files":[{"id":"f","name":"sample"}],"nextPageToken":"next","incompleteSearch":false}"#,
    )]);
    let client = server.client();
    client
        .files_list(
            &session(),
            FileListOptions {
                spaces: Some("value".into()),
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
async fn files_list_option_order_by() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"files":[{"id":"f","name":"sample"}],"nextPageToken":"next","incompleteSearch":false}"#,
    )]);
    let client = server.client();
    client
        .files_list(
            &session(),
            FileListOptions {
                order_by: Some("value".into()),
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
            .find(|(key, _)| key == "orderBy")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn files_list_option_fields() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"files":[{"id":"f","name":"sample"}],"nextPageToken":"next","incompleteSearch":false}"#,
    )]);
    let client = server.client();
    client
        .files_list(
            &session(),
            FileListOptions {
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
async fn files_list_option_include_items_from_all_drives() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"files":[{"id":"f","name":"sample"}],"nextPageToken":"next","incompleteSearch":false}"#,
    )]);
    let client = server.client();
    client
        .files_list(
            &session(),
            FileListOptions {
                include_items_from_all_drives: Some(false),
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
async fn files_list_option_supports_all_drives() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"files":[{"id":"f","name":"sample"}],"nextPageToken":"next","incompleteSearch":false}"#,
    )]);
    let client = server.client();
    client
        .files_list(
            &session(),
            FileListOptions {
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
async fn files_list_option_include_permissions_for_view() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"files":[{"id":"f","name":"sample"}],"nextPageToken":"next","incompleteSearch":false}"#,
    )]);
    let client = server.client();
    client
        .files_list(
            &session(),
            FileListOptions {
                include_permissions_for_view: Some("value".into()),
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
            .find(|(key, _)| key == "includePermissionsForView")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn files_list_labels_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .files_list_labels(&session(), &FileId("f".into()), LabelListOptions::default())
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/listLabels");
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
async fn files_list_labels_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .files_list_labels(&session(), &FileId("f".into()), LabelListOptions::default())
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn files_list_labels_option_page_size() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .files_list_labels(
            &session(),
            &FileId("f".into()),
            LabelListOptions {
                page_size: Some(2),
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
            .find(|(key, _)| key == "maxResults")
            .map(|(_, value)| value.as_str()),
        Some("2")
    );
}

#[tokio::test]
async fn files_list_labels_option_page_token() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .files_list_labels(
            &session(),
            &FileId("f".into()),
            LabelListOptions {
                page_token: Some("value".into()),
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
            .find(|(key, _)| key == "pageToken")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn operations_get_wire_contract() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"name":"operations/op","done":false}"#,
    )]);
    let client = server.client();
    let result = client
        .operations_get(&session(), &OperationName("op".into()))
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{"name":"operations/op","done":false}"#)
            .unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path(), "/drive/v3/operations/op");
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
async fn operations_get_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .operations_get(&session(), &OperationName("op".into()))
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn permissions_get_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"p","role":"reader"}"#)]);
    let client = server.client();
    let result = client
        .permissions_get(
            &session(),
            &FileId("f".into()),
            &PermissionId("p".into()),
            PermissionGetOptions::default(),
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{"id":"p","role":"reader"}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
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
async fn permissions_get_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .permissions_get(
            &session(),
            &FileId("f".into()),
            &PermissionId("p".into()),
            PermissionGetOptions::default(),
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
async fn permissions_get_option_supports_all_drives() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"p","role":"reader"}"#)]);
    let client = server.client();
    client
        .permissions_get(
            &session(),
            &FileId("f".into()),
            &PermissionId("p".into()),
            PermissionGetOptions {
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
async fn permissions_get_option_use_domain_admin_access() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"p","role":"reader"}"#)]);
    let client = server.client();
    client
        .permissions_get(
            &session(),
            &FileId("f".into()),
            &PermissionId("p".into()),
            PermissionGetOptions {
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
async fn permissions_get_option_fields() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"p","role":"reader"}"#)]);
    let client = server.client();
    client
        .permissions_get(
            &session(),
            &FileId("f".into()),
            &PermissionId("p".into()),
            PermissionGetOptions {
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
async fn permissions_list_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .permissions_list(
            &session(),
            &FileId("f".into()),
            PermissionListOptions::default(),
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/permissions");
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
async fn permissions_list_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .permissions_list(
            &session(),
            &FileId("f".into()),
            PermissionListOptions::default(),
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
async fn permissions_list_option_supports_all_drives() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .permissions_list(
            &session(),
            &FileId("f".into()),
            PermissionListOptions {
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
async fn permissions_list_option_use_domain_admin_access() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .permissions_list(
            &session(),
            &FileId("f".into()),
            PermissionListOptions {
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
async fn permissions_list_option_page_size() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .permissions_list(
            &session(),
            &FileId("f".into()),
            PermissionListOptions {
                page_size: Some(2),
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
async fn permissions_list_option_page_token() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .permissions_list(
            &session(),
            &FileId("f".into()),
            PermissionListOptions {
                page_token: Some("value".into()),
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
            .find(|(key, _)| key == "pageToken")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn permissions_list_option_fields() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .permissions_list(
            &session(),
            &FileId("f".into()),
            PermissionListOptions {
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
async fn replies_get_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .replies_get(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
            &ReplyId("r".into()),
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
    assert_eq!(requests[0].method, "GET");
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
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(r#"{"fields": "id"}"#)
            .unwrap()
    );
}

#[tokio::test]
async fn replies_get_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .replies_get(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
            &ReplyId("r".into()),
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
async fn replies_get_option_fields() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .replies_get(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
            &ReplyId("r".into()),
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
async fn replies_get_option_include_deleted() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .replies_get(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
            &ReplyId("r".into()),
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
        Some("false")
    );
}

#[tokio::test]
async fn replies_list_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .replies_list(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
            ReplyListOptions {
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
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/comments/c/replies");
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
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(r#"{"fields": "id"}"#)
            .unwrap()
    );
}

#[tokio::test]
async fn replies_list_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .replies_list(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
            ReplyListOptions {
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
async fn replies_list_option_fields() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .replies_list(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
            ReplyListOptions {
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
async fn replies_list_option_page_size() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .replies_list(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
            ReplyListOptions {
                fields: Some(FieldMask("id".into())),
                page_size: Some(2),
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
async fn replies_list_option_page_token() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .replies_list(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
            ReplyListOptions {
                fields: Some(FieldMask("id".into())),
                page_token: Some("value".into()),
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
            .find(|(key, _)| key == "pageToken")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn replies_list_option_include_deleted() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .replies_list(
            &session(),
            &FileId("f".into()),
            &CommentId("c".into()),
            ReplyListOptions {
                fields: Some(FieldMask("id".into())),
                include_deleted: Some(false),
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
            .find(|(key, _)| key == "includeDeleted")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn revisions_get_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .revisions_get(
            &session(),
            &FileId("f".into()),
            &RevisionId("v".into()),
            RevisionGetOptions::default(),
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
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
async fn revisions_get_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .revisions_get(
            &session(),
            &FileId("f".into()),
            &RevisionId("v".into()),
            RevisionGetOptions::default(),
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
async fn revisions_get_option_fields() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .revisions_get(
            &session(),
            &FileId("f".into()),
            &RevisionId("v".into()),
            RevisionGetOptions {
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
async fn revisions_get_option_acknowledge_abuse() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .revisions_get(
            &session(),
            &FileId("f".into()),
            &RevisionId("v".into()),
            RevisionGetOptions {
                acknowledge_abuse: Some(false),
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
            .find(|(key, _)| key == "acknowledgeAbuse")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn revisions_list_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .revisions_list(
            &session(),
            &FileId("f".into()),
            RevisionListOptions::default(),
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/revisions");
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
async fn revisions_list_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .revisions_list(
            &session(),
            &FileId("f".into()),
            RevisionListOptions::default(),
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
async fn revisions_list_option_fields() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .revisions_list(
            &session(),
            &FileId("f".into()),
            RevisionListOptions {
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
async fn revisions_list_option_page_size() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .revisions_list(
            &session(),
            &FileId("f".into()),
            RevisionListOptions {
                page_size: Some(2),
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
async fn revisions_list_option_page_token() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .revisions_list(
            &session(),
            &FileId("f".into()),
            RevisionListOptions {
                page_token: Some("value".into()),
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
            .find(|(key, _)| key == "pageToken")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn expired_change_cursor_is_distinct_from_missing_metadata() {
    let server = Server::start(vec![Reply::json(
        410,
        r#"{"error":{"message":"expired private cursor"}}"#,
    )]);
    let err = server
        .client()
        .changes_list(
            &session(),
            ChangeListOptions {
                page_token: "old-cursor".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
    assert_eq!(err.kind, GoogleApiError::ExpiredCursor);
    assert_eq!(err.http_status, Some(410));
    assert!(!err.retryable);
    assert_eq!(server.recorded().len(), 1);
}

#[tokio::test]
async fn malformed_metadata_retains_its_received_http_status() {
    let server = Server::start(vec![Reply::json(200, "not-json")]);
    let err = server
        .client()
        .about_get(&session(), AboutOptions::default())
        .await
        .unwrap_err();
    assert_eq!(err.kind, GoogleApiError::InvalidResponse);
    assert_eq!(err.http_status, Some(200));
}
