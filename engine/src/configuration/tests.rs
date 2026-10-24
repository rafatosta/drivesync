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

/// Reads only portal path metadata. No engine, credentials or file contents.
#[test]
#[ignore = "requires explicit document-portal test paths inside the installed Flatpak"]
fn runtime_portal_alias_preserves_host_origin_and_inode() {
    use std::os::unix::fs::MetadataExt;
    let selected = std::path::PathBuf::from(
        std::env::var_os("DRIVESYNC_PORTAL_TEST_PATH").expect("explicit portal path"),
    );
    let expected = std::path::PathBuf::from(
        std::env::var_os("DRIVESYNC_PORTAL_TEST_ORIGIN").expect("explicit host origin"),
    );
    assert!(Path::new("/.flatpak-info").is_file());
    assert!(selected.starts_with("/run/user"));
    let access = super::paths::access_path(&selected).unwrap();
    assert!(access.starts_with("/run/flatpak/doc"));
    assert_eq!(std::fs::canonicalize(&selected).unwrap(), access);
    let a = std::fs::metadata(&selected).unwrap();
    let b = std::fs::metadata(&access).unwrap();
    assert_eq!((a.dev(), a.ino()), (b.dev(), b.ino()));
    assert_eq!(super::paths::local_origin(&selected).unwrap(), expected);
    assert_eq!(super::paths::local_origin(&access).unwrap(), expected);
}
