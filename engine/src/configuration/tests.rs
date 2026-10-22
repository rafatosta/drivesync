//! Unit tests for Drive IDs, canonical path overlap and portal aliases.
use super::{
    models::valid_id,
    paths::{overlap, portal_origin},
};
use std::path::Path;

#[test]
fn path_overlap_uses_components_not_text_prefixes() {
    assert!(overlap(Path::new("/a/b"), Path::new("/a/b/c")));
    assert!(overlap(Path::new("/a/b/c"), Path::new("/a/b")));
    assert!(overlap(Path::new("/a/b"), Path::new("/a/b")));
    assert!(!overlap(Path::new("/a/b"), Path::new("/a/b-other")));
}
#[test]
fn portal_exports_use_host_paths_for_overlap() {
    let parent = portal_origin(
        Path::new("/run/user/1000/doc/aaa/Projects"),
        Path::new("/run/user/1000/doc"),
        Path::new("/home/alex/Projects"),
    )
    .unwrap();
    let child = portal_origin(
        Path::new("/run/user/1000/doc/bbb/Child"),
        Path::new("/run/user/1000/doc"),
        Path::new("/home/alex/Projects/Child"),
    )
    .unwrap();
    assert!(overlap(&parent, &child));
    assert!(portal_origin(
        Path::new("/elsewhere/doc/aaa/Projects"),
        Path::new("/run/user/1000/doc"),
        Path::new("/home/alex/Projects")
    )
    .is_err());
}
#[test]
fn remote_ids_reject_query_injection_and_paths() {
    for id in ["", "../root", "a' or true", "x/y", "x?key=y"] {
        assert!(!valid_id(id));
    }
    assert!(valid_id("root"));
    assert!(valid_id("ab-CD_123"));
}
