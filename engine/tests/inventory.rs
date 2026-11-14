//! Public inventory persistence contracts using real SQLite and synthetic state.
use drivesync_engine::{
    inventory::*,
    models::{AccountId, SyncId},
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "inventory-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn request(&self) -> OpenInventoryRequest {
        OpenInventoryRequest {
            local_root: self.0.clone(),
            expected: SyncIdentity {
                sync_id: SyncId("sync".into()),
                account_id: AccountId("account".into()),
                remote_root_id: "root".into(),
            },
            create_if_missing: true,
        }
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn open_reopen_identity_and_initial_materialization_are_persistent() {
    let tree = Tree::new();
    let result = InventoryManager::open(tree.request()).unwrap();
    assert!(result.created);
    assert_eq!(
        result
            .inventory
            .status(InventoryStatusRequest {})
            .availability,
        InventoryAvailability::Ready
    );
    result
        .inventory
        .mark_initial_download_complete(MarkInitialDownloadCompleteRequest {
            expected_generation: 1,
            expected_complete: false,
        })
        .unwrap();
    drop(result);
    let reopened = InventoryManager::open(tree.request()).unwrap();
    assert!(!reopened.created);
    assert!(reopened.info.initial_download_complete);
    assert_eq!(
        reopened
            .inventory
            .mark_initial_download_complete(MarkInitialDownloadCompleteRequest {
                expected_generation: 1,
                expected_complete: false
            })
            .unwrap_err()
            .kind,
        InventoryError::VersionMismatch
    );
    let mut other = tree.request();
    other.expected.account_id = AccountId("other".into());
    assert!(
        matches!(InventoryManager::open(other),Err(error)if error.kind==InventoryError::InvalidIdentity)
    );
}
#[test]
fn occupied_roots_and_incompatible_databases_are_never_rebuilt() {
    let tree = Tree::new();
    std::fs::write(tree.0.join("user-file"), b"untouched").unwrap();
    assert!(
        matches!(InventoryManager::open(tree.request()),Err(error)if error.kind==InventoryError::AlreadyExists)
    );
    assert!(!tree.0.join(".drivesync").exists());
    std::fs::remove_file(tree.0.join("user-file")).unwrap();
    drop(InventoryManager::open(tree.request()).unwrap());
    let path = tree.0.join(".drivesync/inventory.sqlite3");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.pragma_update(None, "user_version", 900).unwrap();
    drop(db);
    let before = std::fs::read(&path).unwrap();
    assert!(
        matches!(InventoryManager::open(tree.request()),Err(error)if error.kind==InventoryError::UnsupportedSchema)
    );
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn missing_creation_permission_symlinks_and_hardlinks_do_not_open_a_database() {
    use std::os::unix::fs::symlink;
    let tree = Tree::new();
    let mut request = tree.request();
    request.create_if_missing = false;
    assert!(
        matches!(InventoryManager::open(request),Err(error)if error.kind==InventoryError::NotFound)
    );
    assert!(!tree.0.join(".drivesync").exists());
    let outside = Tree::new();
    symlink(&outside.0, tree.0.join("link")).unwrap();
    let mut request = tree.request();
    request.local_root = tree.0.join("link");
    assert!(InventoryManager::open(request).is_err());
    assert!(!outside.0.join(".drivesync").exists());
    std::fs::remove_file(tree.0.join("link")).unwrap();
    drop(InventoryManager::open(tree.request()).unwrap());
    let path = tree.0.join(".drivesync/inventory.sqlite3");
    std::fs::hard_link(&path, outside.0.join("linked.sqlite3")).unwrap();
    assert!(
        matches!(InventoryManager::open(tree.request()),Err(error)if error.kind==InventoryError::PermissionDenied)
    );
}
