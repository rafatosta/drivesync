//! `files.get` request mapping (official parameter names, Discovery v3).

use crate::providers::google::{file_get_query, FieldMask, FileGetOptions};

fn pairs(query: Vec<(String, String)>) -> Vec<(String, String)> {
    let mut query = query;
    query.sort();
    query
}

fn kv(k: &str, v: &str) -> (String, String) {
    (k.to_owned(), v.to_owned())
}

/// FG-U01: default options send no parameter at all.
#[test]
fn default_options_send_nothing() {
    assert!(file_get_query(&FileGetOptions::default()).is_empty());
}

/// FG-U02: each option maps to exactly its official query name, alone.
#[test]
fn each_option_maps_to_its_official_name() {
    let cases = [
        (
            FileGetOptions {
                fields: Some(FieldMask("id,name".into())),
                ..Default::default()
            },
            kv("fields", "id,name"),
        ),
        (
            FileGetOptions {
                supports_all_drives: Some(true),
                ..Default::default()
            },
            kv("supportsAllDrives", "true"),
        ),
        (
            FileGetOptions {
                supports_all_drives: Some(false),
                ..Default::default()
            },
            kv("supportsAllDrives", "false"),
        ),
        (
            FileGetOptions {
                include_labels: Some("l1,l2".into()),
                ..Default::default()
            },
            kv("includeLabels", "l1,l2"),
        ),
        (
            FileGetOptions {
                include_permissions_for_view: Some("published".into()),
                ..Default::default()
            },
            kv("includePermissionsForView", "published"),
        ),
    ];
    for (options, expected) in cases {
        assert_eq!(file_get_query(&options), vec![expected]);
    }
}

/// FG-U03: combined options do not overwrite each other.
#[test]
fn combined_options_are_all_present() {
    let options = FileGetOptions {
        fields: Some(FieldMask("id".into())),
        supports_all_drives: Some(true),
        include_labels: Some("a".into()),
        include_permissions_for_view: Some("published".into()),
    };
    assert_eq!(
        pairs(file_get_query(&options)),
        vec![
            kv("fields", "id"),
            kv("includeLabels", "a"),
            kv("includePermissionsForView", "published"),
            kv("supportsAllDrives", "true"),
        ]
    );
}

/// FG-U04: the field mask is forwarded verbatim (no trimming or rewriting).
#[test]
fn field_mask_is_forwarded_verbatim() {
    let mask = "files(id,name),nextPageToken,capabilities/canEdit";
    let options = FileGetOptions {
        fields: Some(FieldMask(mask.into())),
        ..Default::default()
    };
    assert_eq!(file_get_query(&options), vec![kv("fields", mask)]);
}

/// FG-U05: metadata queries never carry `acknowledgeAbuse` or `alt`, even with
/// every option set (`acknowledgeAbuse` is only valid with `alt=media`).
#[test]
fn metadata_query_never_contains_download_only_parameters() {
    let options = FileGetOptions {
        fields: Some(FieldMask("id".into())),
        supports_all_drives: Some(true),
        include_labels: Some("a".into()),
        include_permissions_for_view: Some("published".into()),
    };
    for (name, _) in file_get_query(&options) {
        assert!(
            !["acknowledgeAbuse", "alt", "access_token", "key"].contains(&name.as_str()),
            "{name}"
        );
    }
}
