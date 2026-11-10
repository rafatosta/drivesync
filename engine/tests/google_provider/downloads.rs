//! Incremental download, range and export HTTP contracts.
use crate::google_provider::support::{session, Framing, Reply, Server};
use drivesync_engine::providers::google::*;

async fn collect(mut stream: GoogleByteStream) -> GoogleResult<Vec<u8>> {
    let mut body = Vec::new();
    while let Some(chunk) = stream.next_chunk().await? {
        body.extend(chunk);
    }
    Ok(body)
}

#[tokio::test]
async fn file_media_is_streamed_with_binary_bytes_and_metadata() {
    let mut reply = Reply::json(200, "");
    reply.body = vec![0, 255, 1, 2, 128];
    reply.headers = vec![
        ("Content-Type", "application/octet-stream".into()),
        ("ETag", "\"v1\"".into()),
    ];
    let server = Server::start(vec![reply]);
    let result = server
        .client()
        .get_file_bytes(&session(), &FileId("a/b".into()), None)
        .await
        .unwrap();
    assert_eq!(result.http_status, 200);
    assert_eq!(result.content_length, Some(5));
    assert_eq!(result.etag.as_deref(), Some("\"v1\""));
    assert_eq!(
        result.mime_type,
        Some(MimeType("application/octet-stream".into()))
    );
    assert_eq!(collect(result.body).await.unwrap(), vec![0, 255, 1, 2, 128]);
    let reqs = server.recorded();
    assert_eq!(reqs[0].method, "GET");
    assert_eq!(reqs[0].path(), "/drive/v3/files/a%2Fb");
    assert_eq!(
        reqs[0].decoded_query(),
        vec![("alt".into(), "media".into())]
    );
    assert_eq!(reqs[0].header("authorization"), Some("Bearer tok-A-secret"));
}

#[tokio::test]
async fn revision_ranges_preserve_content_range_and_offsets() {
    let mut reply = Reply::json(206, "abc");
    reply.headers.push(("Content-Range", "bytes 5-7/20".into()));
    let server = Server::start(vec![reply]);
    let result = server
        .client()
        .get_revision_bytes(
            &session(),
            &FileId("f".into()),
            &RevisionId("rev".into()),
            Some(GoogleByteRange {
                start: 5,
                end_inclusive: Some(7),
            }),
        )
        .await
        .unwrap();
    assert_eq!(result.http_status, 206);
    assert_eq!(result.content_range.as_deref(), Some("bytes 5-7/20"));
    assert_eq!(collect(result.body).await.unwrap(), b"abc");
    let reqs = server.recorded();
    assert_eq!(reqs[0].path(), "/drive/v3/files/f/revisions/rev");
    assert_eq!(reqs[0].header("range"), Some("bytes=5-7"));
    assert_eq!(
        reqs[0].decoded_query(),
        vec![("alt".into(), "media".into())]
    );
}

#[tokio::test]
async fn open_ended_range_uses_the_confirmed_length() {
    let mut reply = Reply::json(206, "abc");
    reply.headers.push(("Content-Range", "bytes 5-7/8".into()));
    let server = Server::start(vec![reply]);
    let result = server
        .client()
        .get_file_bytes(
            &session(),
            &FileId("f".into()),
            Some(GoogleByteRange {
                start: 5,
                end_inclusive: None,
            }),
        )
        .await
        .unwrap();
    assert_eq!(collect(result.body).await.unwrap(), b"abc");
    assert_eq!(server.recorded()[0].header("range"), Some("bytes=5-"));
}

#[tokio::test]
async fn both_export_entry_points_use_mime_type_and_not_alt_media() {
    for raw in [true, false] {
        let server = Server::start(vec![Reply::json(200, "exported")]);
        let client = server.client();
        let body = if raw {
            client
                .files_export(
                    &session(),
                    &FileId("f".into()),
                    &MimeType("application/pdf".into()),
                )
                .await
                .unwrap()
        } else {
            client
                .export_file_bytes(
                    &session(),
                    &FileId("f".into()),
                    &MimeType("application/pdf".into()),
                )
                .await
                .unwrap()
                .body
        };
        assert_eq!(collect(body).await.unwrap(), b"exported");
        let reqs = server.recorded();
        assert_eq!(reqs[0].path(), "/drive/v3/files/f/export");
        assert_eq!(
            reqs[0].decoded_query(),
            vec![("mimeType".into(), "application/pdf".into())]
        );
    }
}

#[tokio::test]
async fn reversed_ranges_are_rejected_locally() {
    let server = Server::start(vec![]);
    let err = server
        .client()
        .get_file_bytes(
            &session(),
            &FileId("f".into()),
            Some(GoogleByteRange {
                start: 8,
                end_inclusive: Some(7),
            }),
        )
        .await
        .unwrap_err();
    assert_eq!(err.kind, GoogleApiError::InvalidArgument);
    assert!(server.recorded().is_empty());
}

#[tokio::test]
async fn inconsistent_range_responses_are_not_success() {
    for (status, range) in [
        (200, "bytes 5-7/8"),
        (206, "bytes 0-2/8"),
        (206, "garbage"),
        (206, "bytes 5-8/8"),
        (206, "bytes 5-7/8"),
    ] {
        let mut reply = Reply::json(
            status,
            if range == "bytes 5-7/8" && status == 206 {
                "too long"
            } else {
                "abc"
            },
        );
        reply.headers.push(("Content-Range", range.into()));
        let server = Server::start(vec![reply]);
        let err = server
            .client()
            .get_file_bytes(
                &session(),
                &FileId("f".into()),
                Some(GoogleByteRange {
                    start: 5,
                    end_inclusive: Some(7),
                }),
            )
            .await
            .unwrap_err();
        assert_eq!(
            err.kind,
            GoogleApiError::InvalidResponse,
            "{status} {range}"
        );
    }
}

#[tokio::test]
async fn truncated_media_is_an_incomplete_transfer() {
    let mut reply = Reply::json(200, "abc");
    reply.framing = Framing::Declared(100);
    let server = Server::start(vec![reply]);
    let result = server
        .client()
        .get_file_bytes(&session(), &FileId("f".into()), None)
        .await
        .unwrap();
    assert_eq!(
        collect(result.body).await.unwrap_err().kind,
        GoogleApiError::IncompleteTransfer
    );
}

#[tokio::test]
async fn media_without_length_is_read_until_eof() {
    let mut reply = Reply::json(200, "abc");
    reply.framing = Framing::UntilClose;
    let server = Server::start(vec![reply]);
    let result = server
        .client()
        .get_file_bytes(&session(), &FileId("f".into()), None)
        .await
        .unwrap();
    assert_eq!(result.content_length, None);
    assert_eq!(collect(result.body).await.unwrap(), b"abc");
}

#[tokio::test]
async fn download_errors_do_not_expose_body_or_redirect_urls() {
    for status in [403, 404, 429, 503, 302] {
        let mut reply = Reply::json(status, "private-content");
        reply
            .headers
            .push(("Location", "http://attacker.test/private".into()));
        let server = Server::start(vec![reply]);
        let err = server
            .client()
            .get_file_bytes(&session(), &FileId("f".into()), None)
            .await
            .unwrap_err();
        assert_eq!(err.http_status, Some(status));
        assert!(!format!("{err:?}").contains("private"));
    }
}
