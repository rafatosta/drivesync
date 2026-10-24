//! In-memory provider fixtures; no credentials, filesystem writes or network.
use super::*;
use serde_json::{json, Value};
fn file(id: &str, parent: &str) -> Value {
    json!({"id":id,"name":"same.txt","mimeType":"text/plain","parents":[parent],"trashed":false,"version":"7","size":"3","md5Checksum":"900150983cd24fb0d6963f7d28e17f72","capabilities":{"canDownload":true}})
}
fn page(files: Vec<Value>, token: Option<&str>) -> Vec<u8> {
    serde_json::to_vec(&json!({"files":files,"nextPageToken":token})).unwrap()
}
fn account() -> AccountId {
    AccountId("google:test".into())
}
#[test]
fn traverses_nested_pages_and_preserves_same_names_by_identity() {
    let mut calls = Vec::new();
    let result = scan(
        &account(),
        "root-id",
        &Cancellation::new(),
        |parent, token| {
            calls.push((parent.to_string(), token.map(String::from)));
            Ok(match (parent, token) {
                ("root-id", None) => page(vec![], Some("next")),
                ("root-id", Some("next")) => {
                    let mut folder = file("child", "root-id");
                    folder["mimeType"] = json!(super::super::models::MIME);
                    page(
                        vec![file("a", "root-id"), file("b", "root-id"), folder],
                        None,
                    )
                }
                ("child", None) => page(vec![file("c", "child")], None),
                _ => panic!("unexpected request"),
            })
        },
    )
    .unwrap();
    assert_eq!(calls.len(), 3);
    assert_eq!(result.entries.len(), 4);
    assert_eq!(result.account_id, account());
    assert_eq!(result.root_id, "root-id");
    assert_eq!(result.entries[0].kind, InventoryKind::Binary);
    assert_eq!(result.entries[0].size, Some(3));
    assert_eq!(result.entries[0].version, Some(7));
    assert_eq!(result.entries[3].parent_id, "child");
}
#[test]
fn skipped_types_and_unsafe_folders_are_not_traversed() {
    let mut items = Vec::new();
    for (id, key, value) in [
        (
            "doc",
            "mimeType",
            json!("application/vnd.google-apps.document"),
        ),
        (
            "shortcut",
            "mimeType",
            json!("application/vnd.google-apps.shortcut"),
        ),
        ("unsafe", "name", json!("../escape")),
        ("shared", "driveId", json!("shared-drive")),
        ("denied", "capabilities", json!({"canDownload":false})),
        ("missing", "md5Checksum", Value::Null),
    ] {
        let mut item = file(id, "root-id");
        item[key] = value;
        items.push(item);
    }
    let mut count = 0;
    let result = scan(&account(), "root-id", &Cancellation::new(), |_, _| {
        count += 1;
        Ok(page(items.clone(), None))
    })
    .unwrap();
    assert_eq!(count, 1);
    assert!(result
        .entries
        .iter()
        .all(|e| matches!(e.kind, InventoryKind::Skipped(_))));
}
#[test]
fn rejects_invalid_metadata_and_partial_searches() {
    for (key, value) in [
        ("id", json!("bad/id")),
        ("parents", json!(["foreign"])),
        ("trashed", json!(true)),
        ("version", json!("oops")),
        ("size", json!("-1")),
        ("md5Checksum", json!("bad")),
    ] {
        let mut item = file("a", "root-id");
        item[key] = value;
        assert!(scan(&account(), "root-id", &Cancellation::new(), |_, _| Ok(
            page(vec![item.clone()], None)
        ))
        .is_err());
    }
    for data in [
        br#"{"files":[],"incompleteSearch":true}"#.as_slice(),
        br#"{}"#,
        br#"not-json"#,
    ] {
        assert!(scan(&account(), "root-id", &Cancellation::new(), |_, _| Ok(
            data.to_vec()
        ))
        .is_err());
    }
}
#[test]
fn refuses_duplicate_ids_and_repeated_page_tokens() {
    assert!(scan(&account(), "root-id", &Cancellation::new(), |_, _| Ok(
        page(vec![file("a", "root-id"), file("a", "root-id")], None)
    ))
    .is_err());
    assert!(scan(&account(), "root-id", &Cancellation::new(), |_, _| Ok(
        page(vec![], Some("repeat"))
    ))
    .is_err());
    assert!(scan(&account(), "root-id", &Cancellation::new(), |_, _| Ok(
        page(vec![file("root-id", "root-id")], None)
    ))
    .is_err());
}
#[test]
fn errors_cancellation_and_limits_never_publish_partial_results() {
    let cancel = Cancellation::new();
    cancel.cancel();
    assert_eq!(
        scan(&account(), "root-id", &cancel, |_, _| panic!(
            "cancelled request"
        ))
        .unwrap_err(),
        EngineError::Authentication(AuthError::Cancelled)
    );
    let cancel = Cancellation::new();
    assert!(scan(&account(), "root-id", &cancel, |_, _| {
        cancel.cancel();
        Ok(page(vec![file("a", "root-id")], None))
    })
    .is_err());
    assert_eq!(
        scan(&account(), "root-id", &Cancellation::new(), |_, _| Err(
            EngineError::RemoteForbidden
        ))
        .unwrap_err(),
        EngineError::RemoteForbidden
    );
    assert!(matches!(
        scan_bounded(
            &account(),
            "root-id",
            &Cancellation::new(),
            &mut |_, _| Ok(page(vec![file("a", "root-id")], None)),
            0,
            10,
            &mut |_, _, _| {}
        ),
        Err(EngineError::InventoryLimit)
    ));
    assert!(matches!(
        scan_bounded(
            &account(),
            "root-id",
            &Cancellation::new(),
            &mut |_, _| Ok(page(vec![], Some("next"))),
            10,
            1,
            &mut |_, _, _| {}
        ),
        Err(EngineError::InventoryLimit)
    ));
}
#[test]
fn resolves_root_alias_and_rejects_shared_or_mismatched_roots() {
    let body =
        br#"{"id":"actual","mimeType":"application/vnd.google-apps.folder","trashed":false}"#;
    assert_eq!(decode_root(body, "root").unwrap(), "actual");
    assert!(decode_root(body, "other").is_err());
    assert!(decode_root(br#"{"id":"actual","mimeType":"application/vnd.google-apps.folder","trashed":false,"driveId":"shared"}"#,"root").is_err());
}

#[test]
fn reports_real_page_counts_and_folder_paths_before_waiting_for_requests() {
    let mut observations = Vec::new();
    let result = scan_with_progress(
        &account(),
        "root-id",
        &Cancellation::new(),
        |parent, token| {
            Ok(match (parent, token) {
                ("root-id", None) => {
                    let mut folder = file("child", "root-id");
                    folder["mimeType"] = json!(super::super::models::MIME);
                    folder["name"] = json!("Documents");
                    page(vec![folder, file("a", "root-id")], Some("next"))
                }
                ("root-id", Some("next")) => page(vec![], None),
                ("child", None) => page(vec![file("b", "child")], None),
                _ => panic!("unexpected request"),
            })
        },
        |files, folders, path| observations.push((files, folders, path.to_path_buf())),
    )
    .unwrap();
    assert_eq!(result.entries.len(), 3);
    assert_eq!(
        observations,
        vec![
            (0, 0, "".into()),
            (1, 1, "".into()),
            (1, 1, "".into()),
            (1, 1, "".into()),
            (1, 1, "Documents".into()),
            (2, 1, "Documents".into()),
        ]
    );
}

#[test]
fn preparation_observations_do_not_bypass_cancellation_or_provider_errors() {
    let cancel = Cancellation::new();
    let mut requests = 0;
    assert!(matches!(
        scan_with_progress(
            &account(),
            "root-id",
            &cancel,
            |_, _| {
                requests += 1;
                Ok(page(vec![], None))
            },
            |_, _, _| cancel.cancel()
        ),
        Err(EngineError::Authentication(AuthError::Cancelled))
    ));
    assert_eq!(requests, 0);
    let mut observations = 0;
    assert!(matches!(
        scan_with_progress(
            &account(),
            "root-id",
            &Cancellation::new(),
            |_, _| Err(EngineError::RemoteRateLimited),
            |_, _, _| observations += 1
        ),
        Err(EngineError::RemoteRateLimited)
    ));
    assert_eq!(observations, 1);
}
