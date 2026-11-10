//! Resumable protocol: no success inferred from sent bytes or HTTP 308.
use crate::google_provider::support::{session, Reply, Server};
use drivesync_engine::providers::google::*;

fn upload(server: &Server, total: Option<u64>) -> GoogleUploadSession {
    GoogleUploadSession {
        session_uri: format!(
            "{}/upload/drive/v3/files?upload_id=secret-session",
            server.base
        ),
        total_bytes: total,
    }
}

#[tokio::test]
async fn resumable_create_and_update_use_the_upload_endpoint() {
    for update in [false, true] {
        let server = Server::start_with(|base| {
            let mut reply = Reply::json(200, "");
            reply.headers.push((
                "Location",
                format!("{base}/upload/drive/v3/files?upload_id=secret-session"),
            ));
            vec![reply]
        });
        let client = server.client();
        let request = GoogleFileWrite {
            name: GooglePatch::Set("file".into()),
            ..Default::default()
        };
        let result = if update {
            client
                .start_resumable_update(
                    &session(),
                    &FileId("f".into()),
                    request,
                    FileUpdateOptions::default(),
                    Some(3),
                )
                .await
                .unwrap()
        } else {
            client
                .start_resumable_create(&session(), request, FileCreateOptions::default(), Some(3))
                .await
                .unwrap()
        };
        assert_eq!(result.total_bytes, Some(3));
        assert!(result.session_uri.ends_with("upload_id=secret-session"));
        let reqs = server.recorded();
        assert_eq!(reqs[0].method, if update { "PATCH" } else { "POST" });
        assert_eq!(
            reqs[0].path(),
            if update {
                "/upload/drive/v3/files/f"
            } else {
                "/upload/drive/v3/files"
            }
        );
        assert_eq!(
            reqs[0].decoded_query(),
            vec![("uploadType".into(), "resumable".into())]
        );
        assert_eq!(reqs[0].header("x-upload-content-length"), Some("3"));
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&reqs[0].body).unwrap(),
            serde_json::json!({"name":"file"})
        );
    }
}

#[tokio::test]
async fn uploads_use_explicit_content_range_and_preserve_confirmed_offsets() {
    let mut incomplete = Reply::json(308, "");
    incomplete.headers.push(("Range", "bytes=0-1".into()));
    let server = Server::start(vec![
        incomplete,
        Reply::json(200, r#"{"id":"f","size":"3"}"#),
    ]);
    let client = server.client();
    let upload = upload(&server, Some(3));
    let result = client
        .upload_chunk(
            &session(),
            &upload,
            GoogleUploadChunk {
                start: 0,
                bytes: b"abc".to_vec(),
                total_bytes: Some(3),
            },
        )
        .await
        .unwrap();
    match result {
        GoogleUploadProgress::Incomplete {
            received_range: Some(range),
        } => assert_eq!(
            range,
            GoogleByteRange {
                start: 0,
                end_inclusive: Some(1)
            }
        ),
        _ => panic!("308 was treated as complete"),
    }
    let result = client
        .upload_chunk(
            &session(),
            &upload,
            GoogleUploadChunk {
                start: 2,
                bytes: b"c".to_vec(),
                total_bytes: Some(3),
            },
        )
        .await
        .unwrap();
    match result {
        GoogleUploadProgress::Complete { file } => assert_eq!(file.id, Some(FileId("f".into()))),
        _ => panic!("missing final confirmation"),
    }
    let reqs = server.recorded();
    assert_eq!(reqs[0].method, "PUT");
    assert_eq!(reqs[0].header("content-range"), Some("bytes 0-2/3"));
    assert_eq!(
        reqs[0].header("content-type"),
        Some("application/octet-stream")
    );
    assert_eq!(reqs[0].body, b"abc");
    assert_eq!(reqs[1].header("content-range"), Some("bytes 2-2/3"));
    assert!(!format!("{upload:?}").contains("secret-session"));
}

#[tokio::test]
async fn progress_queries_accept_missing_range_but_never_call_it_complete() {
    for total in [None, Some(10)] {
        let server = Server::start(vec![Reply::json(308, "")]);
        let result = server
            .client()
            .query_upload_progress(&session(), &upload(&server, total))
            .await
            .unwrap();
        assert!(matches!(
            result,
            GoogleUploadProgress::Incomplete {
                received_range: None
            }
        ));
        let reqs = server.recorded();
        assert!(reqs[0].body.is_empty());
        assert_eq!(
            reqs[0].header("content-range"),
            Some(if total.is_some() {
                "bytes */10"
            } else {
                "bytes */*"
            })
        );
    }
}

#[tokio::test]
async fn invalid_chunk_offsets_totals_and_alignment_fail_locally() {
    let server = Server::start(vec![]);
    let client = server.client();
    let upload = upload(&server, Some(10));
    for chunk in [
        GoogleUploadChunk {
            start: u64::MAX,
            bytes: vec![1],
            total_bytes: None,
        },
        GoogleUploadChunk {
            start: 0,
            bytes: vec![1],
            total_bytes: Some(20),
        },
        GoogleUploadChunk {
            start: 10,
            bytes: vec![1],
            total_bytes: Some(10),
        },
        GoogleUploadChunk {
            start: 0,
            bytes: vec![1],
            total_bytes: Some(10),
        },
    ] {
        assert_eq!(
            client
                .upload_chunk(&session(), &upload, chunk)
                .await
                .unwrap_err()
                .kind,
            GoogleApiError::InvalidArgument
        );
    }
    assert!(server.recorded().is_empty());
}

#[tokio::test]
async fn hostile_session_uris_are_rejected_before_credentials_are_sent() {
    let server = Server::start(vec![]);
    let client = server.client();
    for uri in [
        "http://attacker.test/upload/drive/v3/files?upload_id=secret".to_string(),
        format!("{}/drive/v3/files?upload_id=secret", server.base),
        format!("{}/upload/drive/v3/files#secret", server.base),
    ] {
        let err = client
            .query_upload_progress(
                &session(),
                &GoogleUploadSession {
                    session_uri: uri,
                    total_bytes: None,
                },
            )
            .await
            .unwrap_err();
        assert_eq!(err.kind, GoogleApiError::InvalidArgument);
        assert!(!format!("{err:?}").contains("secret"));
    }
    assert!(server.recorded().is_empty());
}

#[tokio::test]
async fn expired_sessions_are_distinct_from_file_metadata_404() {
    for status in [404, 410] {
        let server = Server::start(vec![Reply::json(status, "private-session")]);
        let err = server
            .client()
            .query_upload_progress(&session(), &upload(&server, None))
            .await
            .unwrap_err();
        assert_eq!(err.kind, GoogleApiError::ExpiredUploadSession);
        assert_eq!(err.http_status, Some(status));
        assert!(!err.retryable);
        assert!(!format!("{err:?}").contains("private"));
    }
}

#[tokio::test]
async fn malformed_upload_ranges_are_not_guessed() {
    for range in [
        "garbage",
        "bytes=1-2",
        "bytes=0-100",
        "bytes=0-18446744073709551615",
    ] {
        let mut reply = Reply::json(308, "");
        reply.headers.push(("Range", range.into()));
        let server = Server::start(vec![reply]);
        let err = server
            .client()
            .query_upload_progress(&session(), &upload(&server, Some(10)))
            .await
            .unwrap_err();
        assert_eq!(err.kind, GoogleApiError::InvalidResponse);
    }
}

#[tokio::test]
async fn conveniences_patch_metadata_without_uploading_content() {
    let server = Server::start(vec![
        Reply::json(200, r#"{"id":"f"}"#),
        Reply::json(200, r#"{"id":"f"}"#),
        Reply::json(200, r#"{"id":"f"}"#),
    ]);
    let client = server.client();
    let id = FileId("f".into());
    client.trash_file(&session(), &id).await.unwrap();
    client
        .rename_file(&session(), &id, "renamed")
        .await
        .unwrap();
    client
        .move_file(
            &session(),
            &id,
            &FileId("new".into()),
            &[FileId("old1".into()), FileId("old2".into())],
        )
        .await
        .unwrap();
    let reqs = server.recorded();
    assert!(reqs
        .iter()
        .all(|r| r.method == "PATCH" && r.path() == "/drive/v3/files/f"));
    for (req, expected) in reqs.iter().zip([
        serde_json::json!({"trashed":true}),
        serde_json::json!({"name":"renamed"}),
        serde_json::json!({}),
    ]) {
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&req.body).unwrap(),
            expected
        );
        assert!(req
            .decoded_query()
            .contains(&("supportsAllDrives".into(), "true".into())));
    }
    let q = reqs[2].decoded_query();
    assert!(q.contains(&("addParents".into(), "new".into())));
    assert!(q.contains(&("removeParents".into(), "old1,old2".into())));
}

#[tokio::test]
async fn premature_completion_cannot_override_a_known_total() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"f","size":"524288"}"#)]);
    let err = server
        .client()
        .upload_chunk(
            &session(),
            &upload(&server, Some(524288)),
            GoogleUploadChunk {
                start: 0,
                bytes: vec![0; 262144],
                total_bytes: None,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(err.kind, GoogleApiError::InvalidResponse);
}

#[tokio::test]
async fn final_metadata_size_must_match_the_known_total_when_received() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"f","size":"2"}"#)]);
    let err = server
        .client()
        .upload_chunk(
            &session(),
            &upload(&server, Some(3)),
            GoogleUploadChunk {
                start: 0,
                bytes: b"abc".to_vec(),
                total_bytes: None,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(err.kind, GoogleApiError::InvalidResponse);
}

#[tokio::test]
async fn empty_files_and_unknown_totals_preserve_protocol_states() {
    let server = Server::start(vec![Reply::json(201, r#"{"id":"empty","size":"0"}"#)]);
    let result = server
        .client()
        .upload_chunk(
            &session(),
            &upload(&server, Some(0)),
            GoogleUploadChunk {
                start: 0,
                bytes: vec![],
                total_bytes: None,
            },
        )
        .await
        .unwrap();
    assert!(matches!(result, GoogleUploadProgress::Complete { .. }));
    assert_eq!(
        server.recorded()[0].header("content-range"),
        Some("bytes */0")
    );
    let server = Server::start(vec![Reply::json(308, "")]);
    let result = server
        .client()
        .upload_chunk(
            &session(),
            &upload(&server, None),
            GoogleUploadChunk {
                start: 0,
                bytes: vec![1; 262144],
                total_bytes: None,
            },
        )
        .await
        .unwrap();
    assert!(matches!(result, GoogleUploadProgress::Incomplete { .. }));
    assert_eq!(
        server.recorded()[0].header("content-range"),
        Some("bytes 0-262143/*")
    );
}

#[tokio::test]
async fn hostile_or_missing_initial_session_location_is_not_accepted() {
    for location in [
        None,
        Some("http://attacker.test/upload/drive/v3/files?upload_id=private"),
    ] {
        let mut reply = Reply::json(200, "");
        if let Some(location) = location {
            reply.headers.push(("Location", location.into()));
        }
        let server = Server::start(vec![reply]);
        let err = server
            .client()
            .start_resumable_create(
                &session(),
                GoogleFileWrite::default(),
                FileCreateOptions::default(),
                None,
            )
            .await
            .unwrap_err();
        assert_eq!(err.kind, GoogleApiError::InvalidResponse);
        assert!(!format!("{err:?}").contains("private"));
        assert_eq!(server.recorded().len(), 1);
    }
}

#[tokio::test]
async fn dropped_upload_does_not_replay_or_query_its_uncertain_effect() {
    let mut slow = Reply::json(200, r#"{"id":"f","size":"3"}"#);
    slow.delay = std::time::Duration::from_millis(500);
    let server = Server::start(vec![slow]);
    let client = server.client();
    let upload = upload(&server, Some(3));
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        client.upload_chunk(
            &session(),
            &upload,
            GoogleUploadChunk {
                start: 0,
                bytes: b"abc".to_vec(),
                total_bytes: None,
            },
        ),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(server.recorded().len(), 1);
}

#[tokio::test]
async fn unknown_total_blocks_cannot_be_announced_as_final() {
    let server = Server::start(vec![Reply::json(200, r#"{"id":"f","size":"262144"}"#)]);
    let err = server
        .client()
        .upload_chunk(
            &session(),
            &upload(&server, None),
            GoogleUploadChunk {
                start: 0,
                bytes: vec![0; 262144],
                total_bytes: None,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(err.kind, GoogleApiError::InvalidResponse);
}
