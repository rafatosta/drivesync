//! Read-only saved-directory availability checks, without Google or credentials.
use drivesync_engine::DriveSyncEngine;
use std::{fs, os::unix::fs::symlink};

#[test]
fn removed_replaced_and_inaccessible_targets_are_not_recreated() {
    let root = std::env::temp_dir().join(format!(
        "drivesync-local-check-{}",
        oauth2::CsrfToken::new_random().secret()
    ));
    fs::create_dir(&root).unwrap();
    let folder = root.join("folder");
    fs::create_dir(&folder).unwrap();
    let alias = root.join("alias");
    symlink(&folder, &alias).unwrap();
    let engine = DriveSyncEngine::new();
    assert!(engine.check_local_folder(&folder).is_ok());
    assert!(engine.check_local_folder(&alias).is_ok());
    assert_eq!(fs::read_dir(&folder).unwrap().count(), 0);
    fs::remove_dir(&folder).unwrap();
    assert!(engine.check_local_folder(&folder).is_err());
    assert!(engine.check_local_folder(&alias).is_err());
    assert!(!folder.exists());
    fs::write(&folder, b"keep user data").unwrap();
    assert!(engine.check_local_folder(&folder).is_err());
    assert_eq!(fs::read(&folder).unwrap(), b"keep user data");
    assert!(engine
        .check_local_folder(std::path::Path::new("relative"))
        .is_err());
    fs::remove_dir_all(root).unwrap();
}
