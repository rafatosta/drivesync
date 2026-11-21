use super::Database;
use crate::{
    inventory::models::{
        AppliedEffect, Baseline, BaselineState, InventoryLookup, InventoryLookupStatus,
        InventoryWrite, LocalState, OperationAction, OperationState, PendingOperation,
        PersistedEffect, ProjectionKind, RemoteState, WriteStatus,
    },
    models::{
        AccountId, DateTime, Evidence, FileIdentity, FileKind, FileState, Id, ItemId,
        LocalCapabilities, LocalIdentity, RemoteCapabilities, RemoteId, SyncRootId,
    },
};
use rusqlite::Connection;

fn database(account: &str) -> Database {
    Database::from_connection(
        Connection::open_in_memory().unwrap(),
        AccountId(account.into()),
    )
    .unwrap()
}

fn timestamp(value: i64) -> DateTime {
    DateTime { unix_millis: value }
}

fn identity(item: &str, remote: &str) -> FileIdentity {
    FileIdentity {
        account_id: AccountId("account-a".into()),
        sync_root_id: SyncRootId("root-a".into()),
        item_id: Some(ItemId(item.into())),
        remote_id: Some(RemoteId(remote.into())),
        local_identity: Evidence::Known(LocalIdentity {
            device_id: 7,
            inode: item.as_bytes()[0] as u64,
            mount_id: Some(2),
        }),
    }
}

fn pending(item: &str, operation: &str, state: OperationState, at: i64) -> PendingOperation {
    PendingOperation {
        id: Id(operation.into()),
        item_id: ItemId(item.into()),
        action: OperationAction::Upload,
        state,
        remote_id: None,
        applied_effect: None,
        idempotency_key: None,
        created_at: timestamp(at),
        updated_at: timestamp(at),
    }
}

fn write(
    item: &str,
    remote: &str,
    operation: &str,
    expected_revision: Option<u64>,
    effect: PersistedEffect,
    at: i64,
) -> InventoryWrite {
    InventoryWrite {
        operation_id: Id(operation.into()),
        item_id: ItemId(item.into()),
        identity: identity(item, remote),
        expected_revision,
        effect,
        recorded_at: timestamp(at),
    }
}

fn lookup(item_id: Option<&str>, remote_id: Option<&str>) -> InventoryLookup {
    InventoryLookup {
        account_id: AccountId("account-a".into()),
        sync_root_id: SyncRootId("root-a".into()),
        item_id: item_id.map(|id| ItemId(id.into())),
        remote_id: remote_id.map(|id| RemoteId(id.into())),
        local_identity: Evidence::NotCollected,
        local_path: None,
    }
}

fn insert_pending(db: &mut Database, item: &str, remote: &str) {
    let result = db.write(write(
        item,
        remote,
        &format!("operation-{item}"),
        Some(0),
        PersistedEffect::Pending(pending(
            item,
            &format!("operation-{item}"),
            OperationState::Pending,
            1,
        )),
        1,
    ));
    assert_eq!(result.status, WriteStatus::Written);
}

#[test]
fn lookup_is_scoped_and_requires_consistent_keys() {
    let mut db = database("account-a");
    insert_pending(&mut db, "item-a", "remote-a");
    insert_pending(&mut db, "item-b", "remote-b");

    let result = db.lookup(lookup(Some("item-a"), Some("remote-b")));
    assert_eq!(result.status, InventoryLookupStatus::Ambiguous);
    assert!(result.snapshot.is_none());

    let result = db.lookup(lookup(None, None));
    assert_eq!(result.status, InventoryLookupStatus::Error);

    let mut wrong_account = lookup(Some("item-a"), None);
    wrong_account.account_id = AccountId("account-b".into());
    assert_eq!(
        db.lookup(wrong_account).status,
        InventoryLookupStatus::Unavailable
    );
}

#[test]
fn revisions_prevent_lost_updates_and_baselines_require_verified_effects() {
    let mut db = database("account-a");
    let operation_id = "operation-item-a";
    let item_id = "item-a";

    let unscoped_create = db.write(write(
        item_id,
        "remote-a",
        operation_id,
        None,
        PersistedEffect::Pending(pending(item_id, operation_id, OperationState::Pending, 1)),
        1,
    ));
    assert_eq!(unscoped_create.status, WriteStatus::Rejected);

    let created = db.write(write(
        item_id,
        "remote-a",
        operation_id,
        Some(0),
        PersistedEffect::Pending(pending(item_id, operation_id, OperationState::Pending, 1)),
        1,
    ));
    assert_eq!(created.status, WriteStatus::Written);
    assert_eq!(created.revision, Some(1));

    let stale = db.write(write(
        item_id,
        "remote-a",
        operation_id,
        Some(0),
        PersistedEffect::Pending(pending(item_id, operation_id, OperationState::Running, 2)),
        2,
    ));
    assert_eq!(stale.status, WriteStatus::RevisionConflict);
    assert_eq!(stale.revision, Some(1));

    let running = db.write(write(
        item_id,
        "remote-a",
        operation_id,
        Some(1),
        PersistedEffect::Pending(pending(item_id, operation_id, OperationState::Running, 2)),
        2,
    ));
    assert_eq!(running.status, WriteStatus::Written);
    assert_eq!(running.revision, Some(2));

    let unverified = db.write(write(
        item_id,
        "remote-a",
        operation_id,
        Some(2),
        PersistedEffect::OperationApplied(AppliedEffect {
            local: None,
            remote: None,
            remote_id: Some(RemoteId("remote-a".into())),
            verified: false,
        }),
        3,
    ));
    assert_eq!(unverified.status, WriteStatus::Written);

    let baseline = Baseline {
        state: BaselineState::Synced,
        local: Some(local_state()),
        remote: Some(remote_state()),
        confirmed_at: Some(timestamp(4)),
        projection: Some(ProjectionKind::Native),
    };
    let rejected_confirmation = db.write(write(
        item_id,
        "remote-a",
        operation_id,
        unverified.revision,
        PersistedEffect::BaselineConfirmed(baseline.clone()),
        4,
    ));
    assert_eq!(rejected_confirmation.status, WriteStatus::Rejected);

    let verified = db.write(write(
        item_id,
        "remote-a",
        operation_id,
        rejected_confirmation.revision,
        PersistedEffect::OperationApplied(AppliedEffect {
            local: Some(local_state()),
            remote: Some(remote_state()),
            remote_id: Some(RemoteId("remote-a".into())),
            verified: true,
        }),
        5,
    ));
    assert_eq!(verified.status, WriteStatus::Rejected);
}

#[test]
fn verified_operation_can_confirm_a_baseline() {
    let mut db = database("account-a");
    let operation_id = "operation-item-a";
    let item_id = "item-a";
    insert_pending(&mut db, item_id, "remote-a");

    let running = db.write(write(
        item_id,
        "remote-a",
        operation_id,
        Some(1),
        PersistedEffect::Pending(pending(item_id, operation_id, OperationState::Running, 2)),
        2,
    ));
    let applied = db.write(write(
        item_id,
        "remote-a",
        operation_id,
        running.revision,
        PersistedEffect::OperationApplied(AppliedEffect {
            local: Some(local_state()),
            remote: Some(remote_state()),
            remote_id: Some(RemoteId("remote-a".into())),
            verified: true,
        }),
        3,
    ));
    assert_eq!(applied.status, WriteStatus::Written);

    let confirmed = db.write(write(
        item_id,
        "remote-a",
        operation_id,
        applied.revision,
        PersistedEffect::BaselineConfirmed(Baseline {
            state: BaselineState::Synced,
            local: Some(local_state()),
            remote: Some(remote_state()),
            confirmed_at: Some(timestamp(4)),
            projection: Some(ProjectionKind::Native),
        }),
        4,
    ));
    assert_eq!(confirmed.status, WriteStatus::Written);

    let found = db.lookup(lookup(Some(item_id), Some("remote-a")));
    assert_eq!(found.status, InventoryLookupStatus::Found);
    let snapshot = found.snapshot.unwrap();
    assert_eq!(snapshot.revision, confirmed.revision.unwrap());
    assert_eq!(snapshot.baseline.state, BaselineState::Synced);
    assert_eq!(
        snapshot.pending_operations[0].state,
        OperationState::Confirmed
    );
}

fn local_state() -> LocalState {
    LocalState {
        state: FileState::Present,
        path: Some("folder/file.txt".into()),
        name: Evidence::Known("file.txt".into()),
        parent: Evidence::Known("folder".into()),
        kind: Evidence::Known(FileKind::Regular),
        identity: Evidence::Known(LocalIdentity {
            device_id: 7,
            inode: 97,
            mount_id: Some(2),
        }),
        size: Evidence::Known(12),
        modified_at: Evidence::Known(timestamp(3)),
        changed_at: Evidence::NotCollected,
        checksum: Evidence::NotCollected,
        capabilities: Evidence::Known(LocalCapabilities {
            can_read: Evidence::Known(true),
            can_write: Evidence::Known(true),
            can_rename: Evidence::Known(true),
            can_delete: Evidence::Known(true),
        }),
    }
}

fn remote_state() -> RemoteState {
    RemoteState {
        state: FileState::Present,
        id: Some(RemoteId("remote-a".into())),
        name: Evidence::Known("file.txt".into()),
        parents: Evidence::Known(vec![RemoteId("parent-a".into())]),
        mime_type: Evidence::Known("text/plain".into()),
        size: Evidence::Known(12),
        checksum: Evidence::NotCollected,
        version: Evidence::Known(1),
        modified_time: Evidence::Known(timestamp(3)),
        trashed: Evidence::Known(false),
        capabilities: Evidence::Known(RemoteCapabilities {
            can_download: Evidence::Known(true),
            can_modify_content: Evidence::Known(true),
            can_rename: Evidence::Known(true),
            can_move_item_within_drive: Evidence::Known(true),
            can_trash: Evidence::Known(true),
        }),
    }
}
