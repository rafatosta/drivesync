//! Decoding of `File` resources (contract: INTERFACES.md §9.1 and §9.3).

use crate::providers::google::{
    parse_file, FileId, GoogleApiError, GoogleFile, MimeType, PermissionId,
};

/// SER-U01: a default `files.get` response (no `fields`) keeps only what was sent.
#[test]
fn minimal_response_leaves_everything_else_unreceived() {
    let file =
        parse_file(br#"{"kind":"drive#file","id":"abc","name":"a.txt","mimeType":"text/plain"}"#)
            .unwrap();
    assert_eq!(
        file,
        GoogleFile {
            id: Some(FileId("abc".into())),
            name: Some("a.txt".into()),
            mime_type: Some(MimeType("text/plain".into())),
            ..Default::default()
        }
    );
    assert_eq!(file.trashed, None, "unreceived must not become false");
    assert_eq!(
        file.parents, None,
        "unreceived must not become an empty list"
    );
}

/// SER-U02: explicit false / empty values are preserved, not confused with absence.
#[test]
fn explicit_false_and_empty_values_are_distinct_from_absence() {
    let file = parse_file(br#"{"id":"x","trashed":false,"parents":[],"starred":false}"#).unwrap();
    assert_eq!(file.trashed, Some(false));
    assert_eq!(file.parents, Some(vec![]));
    assert_eq!(file.starred, Some(false));
    assert_eq!(file.shared, None);
}

/// SER-U03: int64 values stay exact strings (no float round trip).
#[test]
fn int64_fields_are_exact_strings() {
    let file = parse_file(
        br#"{"id":"x","size":"9007199254740993","version":"9223372036854775807","quotaBytesUsed":"0"}"#,
    )
    .unwrap();
    assert_eq!(file.size.as_deref(), Some("9007199254740993"));
    assert_eq!(file.version.as_deref(), Some("9223372036854775807"));
    assert_eq!(file.quota_bytes_used.as_deref(), Some("0"));
}

/// SER-U04: unknown fields added by Google are ignored.
#[test]
fn unknown_fields_are_ignored() {
    let file = parse_file(br#"{"id":"x","futureField":{"a":1},"kind":"drive#file"}"#).unwrap();
    assert_eq!(file.id, Some(FileId("x".into())));
}

/// SER-U05: camelCase names, renamed fields and nested objects.
#[test]
fn rich_response_maps_names_and_nested_types() {
    let json = br#"{
      "id":"f1","name":"Report","mimeType":"application/pdf",
      "parents":["p1","p2"],"trashed":true,"trashedTime":"2024-01-02T03:04:05.678Z",
      "md5Checksum":"d41d8cd98f00b204e9800998ecf8427e","headRevisionId":"rev9",
      "driveId":"d1","modifiedTime":"2024-05-06T07:08:09.000Z","originalFilename":"r.pdf",
      "shortcutDetails":{"targetId":"t1","targetMimeType":"text/plain","targetResourceKey":"k"},
      "capabilities":{"canEdit":true,"canDownload":false,"canMoveItemWithinDrive":true},
      "owners":[{"displayName":"Ana","emailAddress":"ana@example.test","me":true,"permissionId":"pid"}],
      "permissions":[{"id":"perm1","type":"user","role":"writer",
        "permissionDetails":[{"permissionType":"file","role":"writer","inherited":true,"inheritedFrom":"p1"}]}],
      "properties":{"a":"1"},"appProperties":{"b":"2"},
      "imageMediaMetadata":{"width":10,"height":20,"exposureTime":0.5,"location":{"latitude":-1.5}},
      "videoMediaMetadata":{"durationMillis":"1234"},
      "contentRestrictions":[{"readOnly":true,"restrictingUser":{"displayName":"X"}}],
      "downloadRestrictions":{"effectiveDownloadRestrictionWithContext":{"restrictedForReaders":true}},
      "labelInfo":{"labels":[{"id":"l1","revisionId":"r1","fields":{"f":{"valueType":"text","text":["v"]}}}]},
      "spaces":["drive"],"permissionIds":["perm1"]
    }"#;
    let file = parse_file(json).unwrap();
    assert_eq!(
        file.parents,
        Some(vec![FileId("p1".into()), FileId("p2".into())])
    );
    assert_eq!(file.trashed, Some(true));
    assert_eq!(
        file.md5_checksum.as_deref(),
        Some("d41d8cd98f00b204e9800998ecf8427e")
    );
    assert_eq!(file.head_revision_id.unwrap().0, "rev9");
    assert_eq!(file.drive_id.unwrap().0, "d1");
    assert_eq!(file.original_filename.as_deref(), Some("r.pdf"));
    let shortcut = file.shortcut_details.unwrap();
    assert_eq!(shortcut.target_id, Some(FileId("t1".into())));
    assert_eq!(shortcut.target_resource_key.as_deref(), Some("k"));
    let caps = file.capabilities.unwrap();
    assert_eq!(caps.can_edit, Some(true));
    assert_eq!(caps.can_download, Some(false));
    assert_eq!(caps.can_move_item_within_drive, Some(true));
    assert_eq!(caps.can_share, None);
    let owner = &file.owners.unwrap()[0];
    assert_eq!(owner.email_address.as_deref(), Some("ana@example.test"));
    assert_eq!(owner.me, Some(true));
    let perm = &file.permissions.unwrap()[0];
    assert_eq!(perm.id, Some(PermissionId("perm1".into())));
    assert_eq!(perm.type_.as_deref(), Some("user"));
    let detail = &perm.permission_details.as_ref().unwrap()[0];
    assert_eq!(detail.inherited, Some(true));
    assert_eq!(detail.inherited_from, Some(FileId("p1".into())));
    assert_eq!(file.properties.unwrap()["a"], "1");
    assert_eq!(file.app_properties.unwrap()["b"], "2");
    let image = file.image_media_metadata.unwrap();
    assert_eq!(image.width, Some(10));
    assert_eq!(image.exposure_time, Some(0.5));
    assert_eq!(image.location.unwrap().latitude, Some(-1.5));
    assert_eq!(
        file.video_media_metadata
            .unwrap()
            .duration_millis
            .as_deref(),
        Some("1234")
    );
    assert_eq!(file.content_restrictions.unwrap()[0].read_only, Some(true));
    assert_eq!(
        file.download_restrictions
            .unwrap()
            .effective_download_restriction_with_context
            .unwrap()
            .restricted_for_readers,
        Some(true)
    );
    let label = &file.label_info.unwrap().labels.unwrap()[0];
    assert_eq!(label.revision_id.as_deref(), Some("r1"));
    assert_eq!(
        label.fields.as_ref().unwrap()["f"].text,
        Some(vec!["v".into()])
    );
    assert_eq!(file.spaces, Some(vec!["drive".into()]));
}

/// SER-U06: unicode and escapes in names survive decoding.
#[test]
fn unicode_and_escaped_strings_are_preserved() {
    let file =
        parse_file(r#"{"id":"x","name":"relat\u00f3rio \"final\" 📄.txt"}"#.as_bytes()).unwrap();
    assert_eq!(file.name.as_deref(), Some("relatório \"final\" 📄.txt"));
}

/// SER-U07: malformed or structurally wrong payloads fail explicitly.
#[test]
fn invalid_payloads_are_invalid_response_errors() {
    for payload in [
        &b""[..],
        b"not json",
        b"{\"id\":\"x\"",
        b"[]",
        b"null",
        b"\"text\"",
        br#"{"id":"x","trashed":"yes"}"#,
        br#"{"id":"x","parents":"p1"}"#,
    ] {
        let failure = parse_file(payload).unwrap_err();
        assert_eq!(failure.kind, GoogleApiError::InvalidResponse, "{payload:?}");
        assert_eq!(failure.http_status, None);
    }
}

/// SER-U08: a decoding failure does not echo the payload.
#[test]
fn invalid_payload_error_does_not_leak_content() {
    let failure = parse_file(br#"{"id":"secret-id","trashed":"secret-value"#).unwrap_err();
    let rendered = format!("{failure:?}");
    assert!(!rendered.contains("secret"), "{rendered}");
}
