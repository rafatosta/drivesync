//! Unit tests for Drive IDs, canonical path overlap and portal aliases.
use super::{
    models::valid_id,
    paths::{overlap, portal_origin},
};
use std::path::Path;

/// Requires a freshly exported disposable directory with direct sandbox access.
/// Does not authenticate or configure an account.
#[test]
#[ignore = "requires an explicit fresh disposable portal export and scoped direct grant"]
fn fresh_portal_folder_selects_direct_access() {
    let selected = std::path::PathBuf::from(
        std::env::var_os("DRIVESYNC_PORTAL_TEST_PATH").expect("portal path"),
    );
    let origin = std::path::PathBuf::from(
        std::env::var_os("DRIVESYNC_PORTAL_TEST_ORIGIN").expect("host origin"),
    );
    assert!(Path::new("/.flatpak-info").is_file());
    assert!(!selected.join(".drivesync").exists());
    assert_eq!(super::paths::local_origin(&selected).unwrap(), origin);
    assert_eq!(super::paths::local_folder(&selected).unwrap(), origin);
    assert!(!selected.join(".drivesync").exists());
    std::fs::create_dir(selected.join(".drivesync")).unwrap();
    std::fs::write(selected.join(".drivesync/evidence"), b"preserve").unwrap();
    assert_eq!(super::paths::local_folder(&selected).unwrap(), origin);
    assert_eq!(
        std::fs::read(selected.join(".drivesync/evidence")).unwrap(),
        b"preserve"
    );
    assert!(!std::fs::read_dir(&selected).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".drivesync-access-")));
    std::fs::remove_file(selected.join(".drivesync/evidence")).unwrap();
    std::fs::remove_dir(selected.join(".drivesync")).unwrap();
}

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

/// Requires a fresh disposable export and a read-only direct sandbox grant.
#[test]
#[ignore = "requires explicit disposable portal export and read-only direct grant"]
fn saved_portal_observation_uses_read_only_direct_access() {
    let selected = std::path::PathBuf::from(
        std::env::var_os("DRIVESYNC_PORTAL_TEST_PATH").expect("portal path"),
    );
    let origin = std::path::PathBuf::from(
        std::env::var_os("DRIVESYNC_PORTAL_TEST_ORIGIN").expect("host origin"),
    );
    assert!(Path::new("/.flatpak-info").is_file());
    let before = std::fs::read_dir(&origin).unwrap().count();
    assert_eq!(
        super::paths::observation_root(&selected, &origin).unwrap(),
        origin
    );
    assert!(super::paths::observation_root(&selected, &origin.join("different")).is_err());
    assert_eq!(std::fs::read_dir(&origin).unwrap().count(), before);
}

#[test]
fn observation_rejects_changed_origins_without_touching_content() {
    let root = std::env::temp_dir().join(format!(
        "drivesync-origin-check-{}",
        oauth2::CsrfToken::new_random().secret()
    ));
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("preserved"), b"preserved").unwrap();
    assert_eq!(super::paths::observation_root(&root, &root).unwrap(), root);
    assert!(super::paths::observation_root(&root, &root.join("different")).is_err());
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
    assert_eq!(std::fs::read(root.join("preserved")).unwrap(), b"preserved");
    std::fs::remove_dir_all(root).unwrap();
}
