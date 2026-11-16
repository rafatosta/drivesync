//! Conditional metadata writes retain server validators and reject stale versions.
use super::support::{session, Reply, Server};
use drivesync_engine::providers::google::*;
#[tokio::test]
async fn metadata_snapshot_and_conditional_update_preserve_etag_and_412() {
    let mut get = Reply::json(200, r#"{"id":"file","version":"7"}"#);
    get.headers.push(("ETag", "\"revision-7\"".into()));
    let server = Server::start(vec![get, Reply::json(412, r#"{"error":{"code":412}}"#)]);
    let snapshot = server
        .client()
        .files_get_snapshot(
            &session(),
            &FileId("file".into()),
            FileGetOptions::default(),
        )
        .await
        .unwrap();
    assert_eq!(snapshot.etag.as_deref(), Some("\"revision-7\""));
    let error = server
        .client()
        .files_update_conditional(
            &session(),
            &FileId("file".into()),
            GoogleFileWrite {
                name: GooglePatch::Set("new".into()),
                ..Default::default()
            },
            FileUpdateOptions::default(),
            snapshot.etag.as_deref().unwrap(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind, GoogleApiError::PreconditionFailed);
    assert_eq!(
        server.recorded()[1].header("if-match"),
        Some("\"revision-7\"")
    );
}
#[tokio::test]
async fn unsafe_conditional_validators_fail_without_http() {
    let server = Server::start(vec![]);
    for etag in ["", "*", "W/\"weak\"", "bad\r\nheader"] {
        assert_eq!(
            server
                .client()
                .files_update_conditional(
                    &session(),
                    &FileId("file".into()),
                    GoogleFileWrite::default(),
                    FileUpdateOptions::default(),
                    etag
                )
                .await
                .unwrap_err()
                .kind,
            GoogleApiError::InvalidArgument
        );
    }
    assert!(server.recorded().is_empty());
}
#[tokio::test]
async fn conditional_upload_start_rejects_stale_validator() {
    let server = Server::start(vec![Reply::json(412, r#"{"error":{"code":412}}"#)]);
    let error = server
        .client()
        .start_resumable_update_conditional(
            &session(),
            &FileId("file".into()),
            GoogleFileWrite::default(),
            FileUpdateOptions::default(),
            Some(3),
            "\"old\"",
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind, GoogleApiError::PreconditionFailed);
    assert_eq!(server.recorded()[0].header("if-match"), Some("\"old\""));
    assert_eq!(server.recorded()[0].method, "PATCH");
}
