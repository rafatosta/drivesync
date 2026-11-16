//! Developer inspection uses existing SQLite state without creating or mutating it.
use drivesync_engine::{
    inventory::{inspection::*, *},
    models::{AccountId, SyncId},
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "inventory-inspection-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn opening(&self) -> OpenInventoryRequest {
        OpenInventoryRequest {
            local_root: self.0.clone(),
            expected: SyncIdentity {
                sync_id: SyncId("actual-inventory-sync".into()),
                account_id: AccountId("account".into()),
                remote_root_id: "remote-root".into(),
            },
            create_if_missing: true,
        }
    }
    fn request(&self, table: InventoryTable) -> InspectInventoryRequest {
        InspectInventoryRequest {
            local_root: self.0.clone(),
            account_id: AccountId("account".into()),
            remote_root_id: "remote-root".into(),
            table,
            after_rowid: 0,
        }
    }
    fn database(&self) -> PathBuf {
        self.0.join(".drivesync/inventory.sqlite3")
    }
    fn initialize(&self) {
        drop(InventoryManager::open(self.opening()).unwrap());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn missing_inventory_remains_missing_and_existing_readonly_tables_remain_unchanged() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    assert_eq!(
        inspect(fixture.request(InventoryTable::Items))
            .unwrap_err()
            .kind,
        InventoryError::NotFound
    );
    assert!(!fixture.0.join(".drivesync").exists());
    fixture.initialize();
    let before = std::fs::read(fixture.database()).unwrap();
    std::fs::set_permissions(fixture.database(), std::fs::Permissions::from_mode(0o400)).unwrap();
    for table in InventoryTable::ALL {
        let page = inspect(fixture.request(table)).unwrap();
        assert!(!page.columns.is_empty());
        assert_eq!(page.info.sync_id, SyncId("actual-inventory-sync".into()));
        if matches!(
            table,
            InventoryTable::SyncInfo | InventoryTable::PlanningClock
        ) {
            assert_eq!(page.rows.len(), 1);
        } else {
            assert!(page.rows.is_empty());
        }
    }
    assert_eq!(std::fs::read(fixture.database()).unwrap(), before);
    assert!(!fixture
        .0
        .join(".drivesync/inventory.sqlite3-journal")
        .exists());
}
#[test]
fn inspection_validates_account_root_and_schema_without_replacing_state() {
    let fixture = Fixture::new();
    fixture.initialize();
    let mut request = fixture.request(InventoryTable::Items);
    request.account_id = AccountId("other".into());
    assert_eq!(
        inspect(request).unwrap_err().kind,
        InventoryError::InvalidIdentity
    );
    let mut request = fixture.request(InventoryTable::Items);
    request.remote_root_id = "other".into();
    assert_eq!(
        inspect(request).unwrap_err().kind,
        InventoryError::InvalidIdentity
    );
    let db = rusqlite::Connection::open(fixture.database()).unwrap();
    db.pragma_update(None, "user_version", 900).unwrap();
    drop(db);
    let before = std::fs::read(fixture.database()).unwrap();
    assert_eq!(
        inspect(fixture.request(InventoryTable::Items))
            .unwrap_err()
            .kind,
        InventoryError::UnsupportedSchema
    );
    assert_eq!(std::fs::read(fixture.database()).unwrap(), before);
}
#[test]
fn inspection_bounds_pages_and_preserves_null_binary_and_truncated_utf8() {
    let fixture = Fixture::new();
    fixture.initialize();
    let mut db = rusqlite::Connection::open(fixture.database()).unwrap();
    let tx = db.transaction().unwrap();
    for n in 0..=PAGE_SIZE {
        tx.execute("INSERT INTO items(id,kind,local_presence,remote_presence,status,revision) VALUES(?1,'Other','Unknown','Unknown','PendingReconciliation',1)",[format!("item-{n}")]).unwrap();
    }
    tx.execute(
        "INSERT INTO local_files VALUES('item-0',?1,?2)",
        rusqlite::params![vec![b'x', 255], Vec::<u8>::new()],
    )
    .unwrap();
    tx.execute(
        "INSERT INTO metadata_fields VALUES('item-0',0,'sample',?1,NULL)",
        ["é".repeat(5000)],
    )
    .unwrap();
    tx.commit().unwrap();
    drop(db);
    let page = inspect(fixture.request(InventoryTable::Items)).unwrap();
    assert_eq!(page.rows.len(), PAGE_SIZE);
    let null = page
        .columns
        .iter()
        .position(|s| s == "conflict_reason")
        .unwrap();
    assert_eq!(page.rows[0][null].kind, CellKind::Null);
    let mut request = fixture.request(InventoryTable::Items);
    request.after_rowid = page.next_after.unwrap();
    let next = inspect(request).unwrap();
    assert_eq!(next.rows.len(), 1);
    assert!(next.next_after.is_none());
    let binary = inspect(fixture.request(InventoryTable::LocalFiles)).unwrap();
    assert_eq!(binary.rows[0][1].kind, CellKind::BlobHex);
    assert_eq!(binary.rows[0][1].display, "78ff");
    let text = inspect(fixture.request(InventoryTable::MetadataFields)).unwrap();
    let latest = &text.rows[0][3];
    assert_eq!(latest.byte_length, 10000);
    assert!(latest.truncated);
    assert!(latest.display.len() <= 4096);
    assert!(latest.display.chars().all(|c| c == 'é'));
}

#[test]
fn inaccessible_inventory_directory_is_not_reported_as_a_missing_database() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    fixture.initialize();
    let directory = fixture.0.join(".drivesync");
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o000)).unwrap();
    let result = inspect(fixture.request(InventoryTable::Items));
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(result.unwrap_err().kind, InventoryError::PermissionDenied);
}
