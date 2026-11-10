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
async fn drives_create_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"d","name":"sample"}"#)]);
    let client = server.client();
    let result = client
        .drives_create(
            &session(),
            &RequestId("req".into()),
            GoogleDriveWrite::default(),
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{"id":"d","name":"sample"}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path(), "/drive/v3/drives");
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
        serde_json::from_str::<std::collections::BTreeMap<String, String>>(
            r#"{"requestId": "req"}"#
        )
        .unwrap()
    );
}

#[tokio::test]
async fn drives_create_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .drives_create(
            &session(),
            &RequestId("req".into()),
            GoogleDriveWrite::default(),
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
async fn drives_delete_wire_contract() {
    let server = Server::start(vec![Reply::json(204, r#""#)]);
    let client = server.client();
    client
        .drives_delete(
            &session(),
            &DriveId("d".into()),
            DriveDeleteOptions::default(),
        )
        .await
        .unwrap();
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "DELETE");
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
async fn drives_delete_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .drives_delete(
            &session(),
            &DriveId("d".into()),
            DriveDeleteOptions::default(),
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
async fn drives_delete_option_allow_item_deletion() {
    let server = Server::start(vec![Reply::json(204, r#""#)]);
    let client = server.client();
    client
        .drives_delete(
            &session(),
            &DriveId("d".into()),
            DriveDeleteOptions {
                allow_item_deletion: Some(false),
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
            .find(|(key, _)| key == "allowItemDeletion")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn drives_delete_option_use_domain_admin_access() {
    let server = Server::start(vec![Reply::json(204, r#""#)]);
    let client = server.client();
    client
        .drives_delete(
            &session(),
            &DriveId("d".into()),
            DriveDeleteOptions {
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
async fn drives_hide_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"d","name":"sample"}"#)]);
    let client = server.client();
    let result = client
        .drives_hide(&session(), &DriveId("d".into()))
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{"id":"d","name":"sample"}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path(), "/drive/v3/drives/d/hide");
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
async fn drives_hide_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .drives_hide(&session(), &DriveId("d".into()))
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn drives_unhide_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"d","name":"sample"}"#)]);
    let client = server.client();
    let result = client
        .drives_unhide(&session(), &DriveId("d".into()))
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{"id":"d","name":"sample"}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path(), "/drive/v3/drives/d/unhide");
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
async fn drives_unhide_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .drives_unhide(&session(), &DriveId("d".into()))
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn drives_update_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"d","name":"sample"}"#)]);
    let client = server.client();
    let result = client
        .drives_update(
            &session(),
            &DriveId("d".into()),
            GoogleDriveWrite::default(),
            DriveUpdateOptions::default(),
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{"id":"d","name":"sample"}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "PATCH");
    assert_eq!(requests[0].path(), "/drive/v3/drives/d");
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
async fn drives_update_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .drives_update(
            &session(),
            &DriveId("d".into()),
            GoogleDriveWrite::default(),
            DriveUpdateOptions::default(),
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
async fn drives_update_option_use_domain_admin_access() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"d","name":"sample"}"#)]);
    let client = server.client();
    client
        .drives_update(
            &session(),
            &DriveId("d".into()),
            GoogleDriveWrite::default(),
            DriveUpdateOptions {
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
async fn drives_update_option_fields() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"d","name":"sample"}"#)]);
    let client = server.client();
    client
        .drives_update(
            &session(),
            &DriveId("d".into()),
            GoogleDriveWrite::default(),
            DriveUpdateOptions {
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
async fn files_copy_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"f","name":"sample"}"#)]);
    let client = server.client();
    let result = client
        .files_copy(
            &session(),
            &FileId("f".into()),
            GoogleFileWrite::default(),
            FileCopyOptions::default(),
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{"id":"f","name":"sample"}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/copy");
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
async fn files_copy_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .files_copy(
            &session(),
            &FileId("f".into()),
            GoogleFileWrite::default(),
            FileCopyOptions::default(),
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
async fn files_copy_option_supports_all_drives() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"f","name":"sample"}"#)]);
    let client = server.client();
    client
        .files_copy(
            &session(),
            &FileId("f".into()),
            GoogleFileWrite::default(),
            FileCopyOptions {
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
async fn files_copy_option_ignore_default_visibility() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"f","name":"sample"}"#)]);
    let client = server.client();
    client
        .files_copy(
            &session(),
            &FileId("f".into()),
            GoogleFileWrite::default(),
            FileCopyOptions {
                ignore_default_visibility: Some(false),
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
            .find(|(key, _)| key == "ignoreDefaultVisibility")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn files_copy_option_fields() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"f","name":"sample"}"#)]);
    let client = server.client();
    client
        .files_copy(
            &session(),
            &FileId("f".into()),
            GoogleFileWrite::default(),
            FileCopyOptions {
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
async fn files_create_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"f","name":"sample"}"#)]);
    let client = server.client();
    let result = client
        .files_create(
            &session(),
            GoogleFileWrite::default(),
            FileCreateOptions::default(),
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{"id":"f","name":"sample"}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path(), "/drive/v3/files");
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
async fn files_create_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .files_create(
            &session(),
            GoogleFileWrite::default(),
            FileCreateOptions::default(),
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
async fn files_create_option_supports_all_drives() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"f","name":"sample"}"#)]);
    let client = server.client();
    client
        .files_create(
            &session(),
            GoogleFileWrite::default(),
            FileCreateOptions {
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
async fn files_create_option_ignore_default_visibility() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"f","name":"sample"}"#)]);
    let client = server.client();
    client
        .files_create(
            &session(),
            GoogleFileWrite::default(),
            FileCreateOptions {
                ignore_default_visibility: Some(false),
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
            .find(|(key, _)| key == "ignoreDefaultVisibility")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn files_create_option_use_content_as_indexable_text() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"f","name":"sample"}"#)]);
    let client = server.client();
    client
        .files_create(
            &session(),
            GoogleFileWrite::default(),
            FileCreateOptions {
                use_content_as_indexable_text: Some(false),
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
            .find(|(key, _)| key == "useContentAsIndexableText")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn files_create_option_keep_revision_forever() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"f","name":"sample"}"#)]);
    let client = server.client();
    client
        .files_create(
            &session(),
            GoogleFileWrite::default(),
            FileCreateOptions {
                keep_revision_forever: Some(false),
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
            .find(|(key, _)| key == "keepRevisionForever")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn files_create_option_fields() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"f","name":"sample"}"#)]);
    let client = server.client();
    client
        .files_create(
            &session(),
            GoogleFileWrite::default(),
            FileCreateOptions {
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
async fn files_delete_wire_contract() {
    let server = Server::start(vec![Reply::json(204, r#""#)]);
    let client = server.client();
    client
        .files_delete(
            &session(),
            &FileId("f".into()),
            FileDeleteOptions::default(),
        )
        .await
        .unwrap();
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "DELETE");
    assert_eq!(requests[0].path(), "/drive/v3/files/f");
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
async fn files_delete_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .files_delete(
            &session(),
            &FileId("f".into()),
            FileDeleteOptions::default(),
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
async fn files_delete_option_supports_all_drives() {
    let server = Server::start(vec![Reply::json(204, r#""#)]);
    let client = server.client();
    client
        .files_delete(
            &session(),
            &FileId("f".into()),
            FileDeleteOptions {
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
async fn files_delete_option_enforce_single_parent() {
    let server = Server::start(vec![Reply::json(204, r#""#)]);
    let client = server.client();
    client
        .files_delete(
            &session(),
            &FileId("f".into()),
            FileDeleteOptions {
                enforce_single_parent: Some(false),
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
            .find(|(key, _)| key == "enforceSingleParent")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn files_download_wire_contract() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"name":"operations/op","done":false}"#,
    )]);
    let client = server.client();
    let result = client
        .files_download(
            &session(),
            &FileId("f".into()),
            FileDownloadOptions::default(),
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{"name":"operations/op","done":false}"#)
            .unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path(), "/drive/v3/files/f/download");
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
async fn files_download_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .files_download(
            &session(),
            &FileId("f".into()),
            FileDownloadOptions::default(),
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
async fn files_download_option_mime_type() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"name":"operations/op","done":false}"#,
    )]);
    let client = server.client();
    client
        .files_download(
            &session(),
            &FileId("f".into()),
            FileDownloadOptions {
                mime_type: Some(MimeType("value".into())),
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
            .find(|(key, _)| key == "mimeType")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn files_download_option_revision_id() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"name":"operations/op","done":false}"#,
    )]);
    let client = server.client();
    client
        .files_download(
            &session(),
            &FileId("f".into()),
            FileDownloadOptions {
                revision_id: Some(RevisionId("value".into())),
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
            .find(|(key, _)| key == "revisionId")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn files_empty_trash_wire_contract() {
    let server = Server::start(vec![Reply::json(204, r#""#)]);
    let client = server.client();
    client
        .files_empty_trash(&session(), EmptyTrashOptions::default())
        .await
        .unwrap();
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "DELETE");
    assert_eq!(requests[0].path(), "/drive/v3/files/trash");
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
async fn files_empty_trash_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .files_empty_trash(&session(), EmptyTrashOptions::default())
        .await
        .expect_err("remote error");
    assert_eq!(failure.kind, GoogleApiError::PermissionDenied);
    assert_eq!(failure.http_status, Some(403));
    assert!(!failure.retryable);
    assert!(!format!("{failure:?}").contains("private-body"));
    assert!(!format!("{failure:?}").contains("tok-A-secret"));
}

#[tokio::test]
async fn files_empty_trash_option_drive_id() {
    let server = Server::start(vec![Reply::json(204, r#""#)]);
    let client = server.client();
    client
        .files_empty_trash(
            &session(),
            EmptyTrashOptions {
                drive_id: Some(DriveId("value".into())),
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
async fn files_modify_labels_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .files_modify_labels(
            &session(),
            &FileId("f".into()),
            GoogleModifyLabelsRequest::default(),
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
    assert_eq!(requests[0].path(), "/drive/v3/files/f/modifyLabels");
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
async fn files_modify_labels_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .files_modify_labels(
            &session(),
            &FileId("f".into()),
            GoogleModifyLabelsRequest::default(),
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
async fn files_update_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"f","name":"sample"}"#)]);
    let client = server.client();
    let result = client
        .files_update(
            &session(),
            &FileId("f".into()),
            GoogleFileWrite::default(),
            FileUpdateOptions::default(),
        )
        .await
        .unwrap();
    assert_projection(
        &serde_json::to_value(&result).unwrap(),
        &serde_json::from_str::<serde_json::Value>(r#"{"id":"f","name":"sample"}"#).unwrap(),
    );
    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "PATCH");
    assert_eq!(requests[0].path(), "/drive/v3/files/f");
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
async fn files_update_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .files_update(
            &session(),
            &FileId("f".into()),
            GoogleFileWrite::default(),
            FileUpdateOptions::default(),
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
async fn files_update_option_add_parents() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"f","name":"sample"}"#)]);
    let client = server.client();
    client
        .files_update(
            &session(),
            &FileId("f".into()),
            GoogleFileWrite::default(),
            FileUpdateOptions {
                add_parents: Some(vec![FileId("value".into())]),
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
            .find(|(key, _)| key == "addParents")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn files_update_option_remove_parents() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"f","name":"sample"}"#)]);
    let client = server.client();
    client
        .files_update(
            &session(),
            &FileId("f".into()),
            GoogleFileWrite::default(),
            FileUpdateOptions {
                remove_parents: Some(vec![FileId("value".into())]),
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
            .find(|(key, _)| key == "removeParents")
            .map(|(_, value)| value.as_str()),
        Some("value")
    );
}

#[tokio::test]
async fn files_update_option_supports_all_drives() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"f","name":"sample"}"#)]);
    let client = server.client();
    client
        .files_update(
            &session(),
            &FileId("f".into()),
            GoogleFileWrite::default(),
            FileUpdateOptions {
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
async fn files_update_option_keep_revision_forever() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"f","name":"sample"}"#)]);
    let client = server.client();
    client
        .files_update(
            &session(),
            &FileId("f".into()),
            GoogleFileWrite::default(),
            FileUpdateOptions {
                keep_revision_forever: Some(false),
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
            .find(|(key, _)| key == "keepRevisionForever")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn files_update_option_use_content_as_indexable_text() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"f","name":"sample"}"#)]);
    let client = server.client();
    client
        .files_update(
            &session(),
            &FileId("f".into()),
            GoogleFileWrite::default(),
            FileUpdateOptions {
                use_content_as_indexable_text: Some(false),
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
            .find(|(key, _)| key == "useContentAsIndexableText")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn files_update_option_fields() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"f","name":"sample"}"#)]);
    let client = server.client();
    client
        .files_update(
            &session(),
            &FileId("f".into()),
            GoogleFileWrite::default(),
            FileUpdateOptions {
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
async fn files_watch_wire_contract() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    let result = client
        .files_watch(
            &session(),
            &FileId("f".into()),
            FileWatchOptions::default(),
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
    assert_eq!(requests[0].path(), "/drive/v3/files/f/watch");
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
async fn files_watch_remote_errors_are_sanitized() {
    let server = Server::start(vec![Reply::json(
        403,
        r#"{"error":{"message":"private-body","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
    )]);
    let client = server.client();
    let failure = client
        .files_watch(
            &session(),
            &FileId("f".into()),
            FileWatchOptions::default(),
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
async fn files_watch_option_acknowledge_abuse() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .files_watch(
            &session(),
            &FileId("f".into()),
            FileWatchOptions {
                acknowledge_abuse: Some(false),
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
            .find(|(key, _)| key == "acknowledgeAbuse")
            .map(|(_, value)| value.as_str()),
        Some("false")
    );
}

#[tokio::test]
async fn files_watch_option_supports_all_drives() {
    let server = Server::start(vec![Reply::json(200, r#"{}"#)]);
    let client = server.client();
    client
        .files_watch(
            &session(),
            &FileId("f".into()),
            FileWatchOptions {
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

/// File updates change metadata only and preserve explicit patch intent.
#[tokio::test]
async fn file_patch_sends_only_requested_metadata_and_parent_changes() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"f","name":"renamed"}"#)]);
    let result = server
        .client()
        .files_update(
            &session(),
            &FileId("f".into()),
            GoogleFileWrite {
                name: GooglePatch::Set("renamed".into()),
                description: GooglePatch::Clear,
                starred: GooglePatch::Set(false),
                ..Default::default()
            },
            FileUpdateOptions {
                add_parents: Some(vec![FileId("new".into())]),
                remove_parents: Some(vec![FileId("old1".into()), FileId("old2".into())]),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(result.name.as_deref(), Some("renamed"));
    let requests = server.recorded();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&requests[0].body).unwrap(),
        serde_json::json!({"name":"renamed","description":null,"starred":false})
    );
    assert_eq!(
        requests[0].decoded_query(),
        vec![
            ("addParents".into(), "new".into()),
            ("removeParents".into(), "old1,old2".into())
        ]
    );
    assert_eq!(requests[0].header("content-type"), Some("application/json"));
}

/// A recoverable server failure does not trigger an automatic mutation retry.
#[tokio::test]
async fn failed_mutation_is_not_retried() {
    let server = Server::start(vec![
        Reply::json(503, ""),
        Reply::json(200, r#"{"id":"f"}"#),
    ]);
    let failure = server
        .client()
        .files_create(
            &session(),
            GoogleFileWrite::default(),
            FileCreateOptions::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(failure.kind, GoogleApiError::ProviderUnavailable);
    assert!(failure.retryable);
    assert_eq!(server.recorded().len(), 1);
}
