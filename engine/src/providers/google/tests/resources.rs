//! Contract-first tests for partial writes and projected lists.
use crate::providers::google::*;

#[test]
fn partial_writes_omit_unchanged_and_preserve_clear() {
    let request = GoogleFileWrite {
        name: GooglePatch::Set("renamed".into()),
        description: GooglePatch::Clear,
        starred: GooglePatch::Set(false),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"name":"renamed","description":null,"starred":false})
    );
    assert_eq!(
        serde_json::to_value(GoogleFileWrite::default()).unwrap(),
        serde_json::json!({})
    );
}

#[test]
fn lists_and_changes_preserve_projection_and_cursors() {
    let list: GoogleFileList =
        serde_json::from_str(r#"{"nextPageToken":"next","incompleteSearch":true}"#).unwrap();
    assert!(list.files.is_empty());
    assert_eq!(list.next_page_token.as_deref(), Some("next"));
    assert_eq!(list.incomplete_search, Some(true));
    let changes: GoogleChangeList = serde_json::from_str(
        r#"{"changes":[{"fileId":"f","removed":true}],"newStartPageToken":"end"}"#,
    )
    .unwrap();
    assert_eq!(changes.changes[0].file_id, Some(FileId("f".into())));
    assert_eq!(changes.changes[0].removed, Some(true));
    assert_eq!(changes.new_start_page_token.as_deref(), Some("end"));
    assert!(changes.changes[0].file.is_none());
}

#[test]
fn approval_models_use_discovery_fields() {
    let list: GoogleApprovalList = serde_json::from_str(r#"{"items":[{"approvalId":"a","reviewerResponses":[{"reviewer":{"emailAddress":"u@example.test"},"response":"APPROVED"}]}]}"#).unwrap();
    assert_eq!(list.items[0].approval_id, Some(ApprovalId("a".into())));
    assert_eq!(
        list.items[0].reviewer_responses.as_ref().unwrap()[0]
            .response
            .as_deref(),
        Some("APPROVED")
    );
    let request = ReassignApprovalRequest {
        add_reviewers: Some(vec![GoogleAddReviewer {
            added_reviewer_email: "new@example.test".into(),
        }]),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"addReviewers":[{"addedReviewerEmail":"new@example.test"}]})
    );
}

#[test]
fn generic_public_pages_preserve_typed_items_and_cursor() {
    let page: GooglePage<FileId> =
        serde_json::from_str(r#"{"items":["f"],"nextPageToken":"next"}"#).unwrap();
    assert_eq!(page.items, vec![FileId("f".into())]);
    assert_eq!(page.next_page_token.as_deref(), Some("next"));
    assert_eq!(
        serde_json::to_value(page).unwrap(),
        serde_json::json!({"items":["f"],"nextPageToken":"next"})
    );
}
