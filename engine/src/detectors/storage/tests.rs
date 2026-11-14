//! Durable queue contracts use real SQLite files, actual metadata and a controlled lease clock.
use super::*;
use crate::{detectors::*, models::*};
use std::{
    fs,
    os::unix::{
        ffi::OsStrExt,
        fs::{symlink, DirBuilderExt},
    },
    path::PathBuf,
    sync::{
        atomic::{AtomicI64, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "drivesync-queue-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => {
                    fs::create_dir(path.join("root")).unwrap();
                    return Self(path);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(e) => panic!("fixture: {e}"),
            }
        }
    }
    fn scope(&self) -> LocalDetectorScope {
        LocalDetectorScope {
            account_id: AccountId("account".into()),
            sync_id: SyncId("sync".into()),
            local_root: self.0.join("root"),
        }
    }
    fn store(&self, clock: &Arc<AtomicI64>) -> Store {
        let clock = clock.clone();
        let store = Store::open_clock(
            &self.0.join("state"),
            &self.scope(),
            Arc::new(move || clock.load(Ordering::SeqCst)),
        )
        .unwrap();
        store.set_delivery(DeliveryState::Enabled).unwrap();
        store
    }
    fn event(&self, id: &str) -> File {
        File {
            id: FileId(id.into()),
            sync_id: self.scope().sync_id,
            observed_at: Timestamp { unix_millis: -10 },
            scope: FileScope::WholeSync,
            coverage: ObservationCoverage::Partial,
            data: None,
            event: FileEvent {
                remote: None,
                kind: FileEventKind::RecheckRequired,
                previous: None,
                current: None,
                uncertainty: Some(ObservationUncertainty::CaptureStarted),
            },
        }
    }
    fn db(&self) -> PathBuf {
        fs::read_dir(self.0.join("state"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path()
            .join("detector.sqlite3")
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn request() -> TakeEventsRequest {
    TakeEventsRequest {
        max_events: 64,
        lease_duration: Duration::from_millis(200),
    }
}
#[test]
fn enqueue_reopen_complete_payload_bytes_order_and_idempotent_ack() {
    let tree = Tree::new();
    let clock = Arc::new(AtomicI64::new(1000));
    let name = b"link-\xff";
    symlink(
        std::ffi::OsStr::from_bytes(b"missing-\xfe"),
        tree.0.join("root").join(std::ffi::OsStr::from_bytes(name)),
    )
    .unwrap();
    let result = LocalMetadataCollector::new(tree.scope())
        .unwrap()
        .collect(LocalMetadataRequest {
            relative_path: LocalRelativePath::new(name.to_vec()).unwrap(),
            cancellation: LocalCancellation::new(),
        })
        .unwrap();
    let mut local = result.file.unwrap();
    local.storage.inode = MetadataValue::Known(u64::MAX);
    let mut file = tree.event("one");
    file.scope = FileScope::Local(local.relative_path.clone());
    file.coverage = match result.coverage {
        LocalScanCoverage::Complete => ObservationCoverage::Complete,
        LocalScanCoverage::Partial => ObservationCoverage::Partial,
        LocalScanCoverage::Unavailable => ObservationCoverage::Unavailable,
    };
    file.event = FileEvent {
        remote: None,
        kind: FileEventKind::Renamed,
        previous: Some(FileLocation::Local(
            LocalRelativePath::new(b"old".to_vec()).unwrap(),
        )),
        current: Some(FileLocation::Local(local.relative_path.clone())),
        uncertainty: None,
    };
    file.data = Some(FileData::Local(local));
    let store = tree.store(&clock);
    assert!(store.enqueue(&file).unwrap());
    assert!(!store.enqueue(&file).unwrap());
    let second = tree.event("two");
    store.enqueue(&second).unwrap();
    drop(store);
    let store = tree.store(&clock);
    let batch = store.claim(request()).unwrap();
    assert_eq!(batch.files, vec![file.clone(), second]);
    let id = batch.batch_id.unwrap();
    assert_eq!(
        store.acknowledge(&id, EventDisposition::Processed).unwrap(),
        2
    );
    assert_eq!(
        store.acknowledge(&id, EventDisposition::Processed).unwrap(),
        2
    );
    drop(store);
    let store = tree.store(&clock);
    assert_eq!(store.snapshot().confirmed, 2);
    assert_eq!(store.snapshot().pending, 0);
    assert!(!store.enqueue(&file).unwrap());
    let mut divergent = file;
    divergent.observed_at.unix_millis += 1;
    assert_eq!(
        store.enqueue(&divergent).unwrap_err().kind,
        QueueError::Conflict
    );
    assert_eq!(store.claim(request()).unwrap().files.len(), 0);
}
#[test]
fn leases_survive_reopen_expire_release_and_reject_foreign_or_wrong_ack() {
    let tree = Tree::new();
    let clock = Arc::new(AtomicI64::new(1000));
    let store = tree.store(&clock);
    let file = tree.event("event");
    store.enqueue(&file).unwrap();
    let batch = store.claim(request()).unwrap();
    let old = batch.batch_id.unwrap();
    drop(store);
    let store = tree.store(&clock);
    assert!(store.claim(request()).unwrap().files.is_empty());
    clock.store(1300, Ordering::SeqCst);
    assert_eq!(
        store
            .acknowledge(&old, EventDisposition::Processed)
            .unwrap_err()
            .kind,
        QueueError::LeaseExpired
    );
    let batch = store.claim(request()).unwrap();
    assert_eq!(batch.files, vec![file.clone()]);
    let id = batch.batch_id.unwrap();
    assert_ne!(old, id);
    assert_eq!(store.release(&id).unwrap(), 1);
    assert_eq!(store.release(&id).unwrap(), 1);
    let next = store.claim(request()).unwrap().batch_id.unwrap();
    assert_eq!(
        store
            .acknowledge(&id, EventDisposition::Processed)
            .unwrap_err()
            .kind,
        QueueError::InvalidLease
    );
    assert_eq!(
        store
            .acknowledge(&BatchId("foreign".into()), EventDisposition::Processed)
            .unwrap_err()
            .kind,
        QueueError::InvalidLease
    );
    assert_eq!(
        store
            .acknowledge(&next, EventDisposition::Discarded)
            .unwrap(),
        1
    );
    assert!(store
        .acknowledge(&next, EventDisposition::Processed)
        .is_err());
}
#[test]
fn independent_connections_cannot_claim_the_same_ordered_batch() {
    let tree = Tree::new();
    let clock = Arc::new(AtomicI64::new(1000));
    let first = Arc::new(tree.store(&clock));
    first.enqueue(&tree.event("event")).unwrap();
    let second = Arc::new(tree.store(&clock));
    let gate = Arc::new(std::sync::Barrier::new(3));
    let joins: Vec<_> = [first, second]
        .into_iter()
        .map(|store| {
            let gate = gate.clone();
            std::thread::spawn(move || {
                gate.wait();
                store.claim(request()).unwrap().files.len()
            })
        })
        .collect();
    gate.wait();
    assert_eq!(
        joins
            .into_iter()
            .map(|join| join.join().unwrap())
            .sum::<usize>(),
        1
    );
}
#[test]
fn payload_corruption_and_future_schema_are_not_replaced() {
    let tree = Tree::new();
    let clock = Arc::new(AtomicI64::new(1000));
    let store = tree.store(&clock);
    store.enqueue(&tree.event("event")).unwrap();
    let connection = rusqlite::Connection::open(tree.db()).unwrap();
    connection
        .execute("UPDATE events SET payload=?1", [b"damaged".as_slice()])
        .unwrap();
    assert_eq!(
        store.claim(request()).unwrap_err().kind,
        QueueError::Corrupt
    );
    assert_eq!(store.snapshot().pending, 1);
    drop(store);
    connection.pragma_update(None, "user_version", 900).unwrap();
    drop(connection);
    assert_eq!(
        Store::open(&tree.0.join("state"), &tree.scope())
            .err()
            .unwrap()
            .kind,
        QueueError::SchemaUnsupported
    );
    let connection = rusqlite::Connection::open(tree.db()).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM events", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
}
#[test]
fn private_paths_identity_and_rollback_are_enforced() {
    let tree = Tree::new();
    let clock = Arc::new(AtomicI64::new(1000));
    let store = tree.store(&clock);
    let mut wrong = tree.event("wrong");
    wrong.sync_id = SyncId("other".into());
    assert_eq!(
        store.enqueue(&wrong).unwrap_err().kind,
        QueueError::IdentityMismatch
    );
    let connection = rusqlite::Connection::open(tree.db()).unwrap();
    connection.execute_batch("CREATE TRIGGER fail_insert BEFORE INSERT ON events BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert!(store.enqueue(&tree.event("event")).is_err());
    assert_eq!(store.snapshot().pending, 0);
    connection
        .execute_batch("DROP TRIGGER fail_insert;")
        .unwrap();
    store.enqueue(&tree.event("event")).unwrap();
    drop(store);
    connection
        .execute("UPDATE identity SET account='different'", [])
        .unwrap();
    drop(connection);
    assert_eq!(
        Store::open(&tree.0.join("state"), &tree.scope())
            .err()
            .unwrap()
            .kind,
        QueueError::IdentityMismatch
    );
    symlink(tree.0.join("state"), tree.0.join("alias")).unwrap();
    assert!(Store::open(&tree.0.join("alias"), &tree.scope()).is_err());
}

#[test]
fn codec_rejects_unvalidated_paths_unknown_fields_and_future_versions() {
    let tree = Tree::new();
    let file = tree.event("event");
    let bytes = codec::encode(&file).unwrap();
    let mut wire: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    wire["scope"] = serde_json::json!({"Local": [46,46,47,102]});
    assert_eq!(
        codec::decode(&serde_json::to_vec(&wire).unwrap())
            .unwrap_err()
            .kind,
        QueueError::Corrupt
    );
    let mut wire: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    wire["unexpected"] = serde_json::json!(true);
    assert_eq!(
        codec::decode(&serde_json::to_vec(&wire).unwrap())
            .unwrap_err()
            .kind,
        QueueError::Corrupt
    );
    let mut wire: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    wire["version"] = serde_json::json!(900);
    assert_eq!(
        codec::decode(&serde_json::to_vec(&wire).unwrap())
            .unwrap_err()
            .kind,
        QueueError::SchemaUnsupported
    );
}

#[test]
fn corruption_during_a_reservation_blocks_ack_without_confirming_the_batch() {
    let tree = Tree::new();
    let clock = Arc::new(AtomicI64::new(1000));
    let store = tree.store(&clock);
    store.enqueue(&tree.event("event")).unwrap();
    let batch = store.claim(request()).unwrap();
    let connection = rusqlite::Connection::open(tree.db()).unwrap();
    connection
        .execute(
            "UPDATE events SET digest=?1",
            [b"wrong checksum".as_slice()],
        )
        .unwrap();
    assert_eq!(
        store
            .acknowledge(&batch.batch_id.unwrap(), EventDisposition::Processed)
            .unwrap_err()
            .kind,
        QueueError::Corrupt
    );
    assert_eq!(store.snapshot().confirmed, 0);
    assert!(
        store.refresh().is_err(),
        "corruption must remain unavailable, not clear on a status query"
    );
}

#[test]
fn unified_queue_uses_reserved_directory_and_preserves_inventory() {
    use std::os::unix::fs::PermissionsExt;
    let tree = Tree::new();
    let state = tree.scope().local_root.join(".drivesync");
    let store = Store::open_unified(&tree.scope(), "google", "remote-root").unwrap();
    assert!(state.join("detector.sqlite3").is_file());
    assert_eq!(
        fs::metadata(&state).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(state.join("detector.sqlite3"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert!(!tree.0.join("state").exists());
    let inventory = state.join("inventory.sqlite3");
    fs::write(&inventory, b"independent inventory owner").unwrap();
    store
        .enqueue_from(&tree.event("durable"), DetectorOrigin::Local)
        .unwrap();
    drop(store);
    let reopened = Store::open_unified(&tree.scope(), "google", "remote-root").unwrap();
    assert_eq!(reopened.snapshot().pending, 1);
    assert_eq!(fs::read(inventory).unwrap(), b"independent inventory owner");
}

#[test]
fn root_queue_refuses_different_accounts_roots_or_syncs_without_replacement() {
    let tree = Tree::new();
    let store = Store::open_unified(&tree.scope(), "google", "remote-root").unwrap();
    store
        .enqueue_from(&tree.event("owned"), DetectorOrigin::Local)
        .unwrap();
    drop(store);
    let mut scope = tree.scope();
    scope.account_id = AccountId("other".into());
    assert_eq!(
        Store::open_unified(&scope, "google", "remote-root")
            .err()
            .unwrap()
            .kind,
        QueueError::IdentityMismatch
    );
    scope = tree.scope();
    scope.sync_id = SyncId("other-sync".into());
    assert_eq!(
        Store::open_unified(&scope, "google", "remote-root")
            .err()
            .unwrap()
            .kind,
        QueueError::IdentityMismatch
    );
    assert_eq!(
        Store::open_unified(&tree.scope(), "google", "other-root")
            .err()
            .unwrap()
            .kind,
        QueueError::IdentityMismatch
    );
    assert_eq!(
        Store::open_unified(&tree.scope(), "google", "remote-root")
            .unwrap()
            .snapshot()
            .pending,
        1
    );
}

#[test]
fn reserved_state_never_follows_symlinks_or_creates_a_missing_user_root() {
    let tree = Tree::new();
    let mut scope = tree.scope();
    scope.local_root = tree.0.join("missing/root");
    assert!(Store::open_unified(&scope, "google", "root").is_err());
    assert!(!tree.0.join("missing").exists());
    fs::create_dir(tree.0.join("outside")).unwrap();
    symlink(
        tree.0.join("outside"),
        tree.scope().local_root.join(".drivesync"),
    )
    .unwrap();
    assert!(Store::open_unified(&tree.scope(), "google", "root").is_err());
    assert!(fs::read_dir(tree.0.join("outside"))
        .unwrap()
        .next()
        .is_none());
}

#[test]
fn reserved_database_rejects_old_schema_without_migration() {
    let tree = Tree::new();
    drop(Store::open_unified(&tree.scope(), "google", "root").unwrap());
    let path = tree.scope().local_root.join(".drivesync/detector.sqlite3");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.pragma_update(None, "user_version", 1).unwrap();
    db.execute_batch("DROP TABLE observations;").unwrap();
    drop(db);
    let before = fs::read(&path).unwrap();
    assert_eq!(
        Store::open_unified(&tree.scope(), "google", "root")
            .err()
            .unwrap()
            .kind,
        QueueError::SchemaUnsupported
    );
    assert_eq!(fs::read(path).unwrap(), before);
}
