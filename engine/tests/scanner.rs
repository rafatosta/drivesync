//! Public Scanner API driven without a Tokio runtime, on a real temporary inventory/root.
use drivesync_engine::{inventory::*, models::*, scanner::*};
use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    time::Duration,
};
struct WakeThread(std::thread::Thread);
impl Wake for WakeThread {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}
fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(WakeThread(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::park_timeout(Duration::from_secs(2)),
        }
    }
}
#[test]
fn public_scanner_runs_without_runtime_and_validates_exact_root_bytes() {
    use std::os::unix::{ffi::OsStrExt, ffi::OsStringExt};
    fn send_sync<T: Send + Sync>() {}
    send_sync::<Scanner>();
    let path = std::env::temp_dir().join(std::ffi::OsString::from_vec(
        format!("scanner-public-{}-", std::process::id())
            .into_bytes()
            .into_iter()
            .chain([255])
            .collect(),
    ));
    std::fs::create_dir(&path).unwrap();
    let inventory = Arc::new(
        InventoryManager::open(OpenInventoryRequest {
            local_root: path.clone(),
            expected: SyncIdentity {
                sync_id: SyncId("sync".into()),
                account_id: AccountId("account".into()),
                remote_root_id: "root".into(),
            },
            create_if_missing: true,
        })
        .unwrap()
        .inventory,
    );
    assert_eq!(
        inventory
            .get_sync_info(GetSyncInfoRequest {})
            .unwrap()
            .local_root,
        path.as_os_str().as_bytes()
    );
    std::fs::write(path.join("file"), b"unchanged").unwrap();
    let scanner = Scanner::new(ScannerConfig {
        inventory: inventory.clone(),
        root: LocalRoot {
            sync_id: SyncId("sync".into()),
            path: path.clone(),
        },
    })
    .unwrap();
    let report = block_on(scanner.scan(ScanRequest {
        sync_id: SyncId("sync".into()),
    }))
    .unwrap();
    assert!(report.ready_for_analysis());
    assert_eq!(report.files_observed, 1);
    assert_eq!(report.items_recorded, 1);
    assert_eq!(scanner.status().state, ScannerState::Completed);
    drop(scanner);
    drop(inventory);
    let reopened = InventoryManager::open(OpenInventoryRequest {
        local_root: path.clone(),
        expected: SyncIdentity {
            sync_id: SyncId("sync".into()),
            account_id: AccountId("account".into()),
            remote_root_id: "root".into(),
        },
        create_if_missing: false,
    })
    .unwrap()
    .inventory;
    let pending = reopened
        .list_items(ListItemsRequest {
            filter: ItemFilter {
                statuses: vec![SyncStatus::PendingReconciliation],
                ..Default::default()
            },
            pagination: Pagination::default(),
        })
        .unwrap()
        .page;
    assert_eq!(pending.entries.len(), 1);
    assert_eq!(pending.entries[0].local_presence, PresenceState::Present);
    drop(reopened);
    std::fs::remove_dir_all(path).unwrap();
}
