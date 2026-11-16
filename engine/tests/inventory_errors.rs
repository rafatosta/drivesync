//! Caller identity mistakes must not poison a valid inventory instance.
use drivesync_engine::{inventory::*, models::*};
#[test]
fn rejected_account_reference_keeps_valid_inventory_available() {
    let path = std::env::temp_dir().join(format!("inventory-caller-error-{}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    let manager = InventoryManager::open(OpenInventoryRequest {
        local_root: path.clone(),
        expected: SyncIdentity {
            sync_id: SyncId("sync".into()),
            account_id: AccountId("account".into()),
            remote_root_id: "root".into(),
        },
        create_if_missing: true,
    })
    .unwrap()
    .inventory;
    let error = manager
        .get_item(GetItemRequest {
            selector: ItemSelector::RemoteRef(RemoteFileRef {
                account_id: AccountId("other".into()),
                file_id: RemoteFileId("file".into()),
            }),
        })
        .unwrap_err();
    assert_eq!(error.kind, InventoryError::InvalidIdentity);
    assert_eq!(
        manager.status(InventoryStatusRequest {}).availability,
        InventoryAvailability::Ready
    );
    assert_eq!(
        manager
            .get_sync_info(GetSyncInfoRequest {})
            .unwrap()
            .info
            .account_id,
        AccountId("account".into())
    );
    drop(manager);
    std::fs::remove_dir_all(path).unwrap();
}
