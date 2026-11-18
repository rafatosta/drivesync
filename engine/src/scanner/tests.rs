//! Real filesystem/SQLite scenarios with deterministic races at private observation boundaries.
use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::{atomic::AtomicU64, Condvar},
    time::{Duration, Instant},
};
struct Fixture {
    root: PathBuf,
    inventory: Arc<InventoryManager>,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "scanner-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let inventory = Arc::new(
            InventoryManager::open(OpenInventoryRequest {
                local_root: root.clone(),
                expected: SyncIdentity {
                    sync_id: SyncId("scan-scope".into()),
                    account_id: AccountId("account".into()),
                    remote_root_id: "remote-root".into(),
                },
                create_if_missing: true,
            })
            .unwrap()
            .inventory,
        );
        Self { root, inventory }
    }
    fn scanner(&self) -> Scanner {
        Scanner::new(ScannerConfig {
            inventory: self.inventory.clone(),
            root: LocalRoot {
                sync_id: SyncId("scan-scope".into()),
                path: self.root.clone(),
            },
        })
        .unwrap()
    }
    fn request(&self) -> ScanRequest {
        ScanRequest {
            sync_id: SyncId("scan-scope".into()),
        }
    }
    fn item(&self, path: &[u8]) -> SyncItem {
        self.inventory
            .get_item(GetItemRequest {
                selector: ItemSelector::LocalRelativePath(
                    LocalRelativePath::new(path.to_vec()).unwrap(),
                ),
            })
            .unwrap()
            .item
            .unwrap()
    }
    fn list(&self) -> Vec<SyncItem> {
        self.inventory
            .list_items(ListItemsRequest {
                filter: ItemFilter::default(),
                pagination: Pagination {
                    limit: 1000,
                    after: None,
                },
            })
            .unwrap()
            .page
            .entries
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
#[tokio::test]
async fn scan_records_local_state_without_internal_files_or_physical_changes() {
    let f = Fixture::new();
    fs::create_dir(f.root.join("folder")).unwrap();
    fs::write(f.root.join("folder/a"), b"content").unwrap();
    fs::write(f.root.join("empty"), b"").unwrap();
    fs::write(f.root.join(".drivesync/private"), b"private").unwrap();
    let scanner = f.scanner();
    assert_eq!(scanner.status(), ScannerStatus::default());
    assert!(f.list().is_empty());
    let report = scanner.scan(f.request()).await.unwrap();
    assert!(report.ready_for_analysis());
    assert_eq!(report.files_observed, 2);
    assert_eq!(report.directories_observed, 2);
    assert_eq!(report.items_recorded, 3);
    assert_eq!(f.list().len(), 3);
    assert_eq!(fs::read(f.root.join("folder/a")).unwrap(), b"content");
    assert_eq!(
        fs::read(f.root.join(".drivesync/private")).unwrap(),
        b"private"
    );
    for item in f.list() {
        assert_eq!(item.local_presence, PresenceState::Present);
        assert_eq!(item.status, SyncStatus::PendingReconciliation);
        assert!(item.baseline.is_none());
        assert!(item.remote.is_none());
        assert_eq!(item.remote_presence, PresenceState::Unknown);
    }
    let status = scanner.status();
    assert_eq!(status.state, ScannerState::Completed);
    assert_eq!(status.last_report_status, Some(ScanStatus::Completed));
    assert_eq!(status.files_observed, report.files_observed);
    assert_eq!(status.items_recorded, report.items_recorded);
    assert!(status.finished_at.is_some());
    assert!(status.current_directory.is_none());
}
#[tokio::test]
async fn rescanning_preserves_identity_and_detects_confirmed_absence_including_removed_subtrees() {
    let f = Fixture::new();
    fs::create_dir(f.root.join("folder")).unwrap();
    fs::write(f.root.join("folder/a"), b"data").unwrap();
    fs::write(f.root.join("keep"), b"same").unwrap();
    let scanner = f.scanner();
    scanner.scan(f.request()).await.unwrap();
    let first = f.item(b"keep");
    let removed = f.item(b"folder/a");
    let again = scanner.scan(f.request()).await.unwrap();
    assert!(again.ready_for_analysis());
    assert_eq!(f.item(b"keep").id, first.id);
    assert_eq!(f.list().len(), 3);
    fs::remove_dir_all(f.root.join("folder")).unwrap();
    let report = scanner.scan(f.request()).await.unwrap();
    assert!(report.ready_for_analysis());
    assert_eq!(f.item(b"folder/a").id, removed.id);
    assert_eq!(
        f.item(b"folder/a").local_presence,
        PresenceState::AbsentConfirmed
    );
    assert_eq!(
        f.item(b"folder").local_presence,
        PresenceState::AbsentConfirmed
    );
    assert_eq!(f.item(b"keep").local_presence, PresenceState::Present);
}
#[tokio::test]
async fn unreadable_subtree_keeps_descendants_and_blocks_analysis_while_other_files_progress() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    fs::create_dir(f.root.join("locked")).unwrap();
    fs::write(f.root.join("locked/child"), b"data").unwrap();
    let scanner = f.scanner();
    scanner.scan(f.request()).await.unwrap();
    fs::write(f.root.join("new"), b"ok").unwrap();
    fs::set_permissions(f.root.join("locked"), fs::Permissions::from_mode(0o000)).unwrap();
    let result = scanner.scan(f.request()).await;
    fs::set_permissions(f.root.join("locked"), fs::Permissions::from_mode(0o700)).unwrap();
    let report = result.unwrap();
    assert_eq!(report.status, ScanStatus::Partial);
    assert!(!report.ready_for_analysis());
    assert!(report
        .issues
        .iter()
        .any(|i| i.kind == ScanIssueKind::AccessDenied));
    assert_eq!(
        f.item(b"locked/child").local_presence,
        PresenceState::Present
    );
    assert_eq!(f.item(b"new").local_presence, PresenceState::Present);
    assert_eq!(scanner.status().state, ScannerState::Completed);
    assert_eq!(
        scanner.status().last_report_status,
        Some(ScanStatus::Partial)
    );
    assert!(scanner
        .scan(f.request())
        .await
        .unwrap()
        .ready_for_analysis());
}
#[tokio::test]
async fn non_utf8_symlinks_and_special_entries_never_escape_or_open_file_contents() {
    use std::os::unix::{ffi::OsStringExt, fs::symlink};
    let f = Fixture::new();
    let outside = Fixture::new();
    fs::write(outside.root.join("secret"), b"untouched").unwrap();
    symlink(&outside.root, f.root.join("link")).unwrap();
    symlink("missing", f.root.join("broken")).unwrap();
    fs::write(
        f.root
            .join(std::ffi::OsString::from_vec(b"name-\xff".to_vec())),
        b"bytes",
    )
    .unwrap();
    let fifo = std::ffi::CString::new(f.root.join("pipe").as_os_str().as_bytes()).unwrap();
    // SAFETY: valid temporary path and mode; only creates a fixture FIFO, never opens it.
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    let report = f.scanner().scan(f.request()).await.unwrap();
    assert!(report.ready_for_analysis());
    assert!(report
        .issues
        .iter()
        .any(|i| i.kind == ScanIssueKind::UnsupportedEntryType));
    assert_eq!(f.list().len(), 4);
    assert!(f.item(b"link").local.unwrap().kind == MetadataValue::Known(LocalEntryKind::Symlink));
    assert_eq!(f.item(b"name-\xff").local_presence, PresenceState::Present);
    assert_eq!(fs::read(outside.root.join("secret")).unwrap(), b"untouched");
}
#[tokio::test]
async fn local_scan_preserves_remote_baseline_and_last_sync_but_marks_work_pending() {
    let f = Fixture::new();
    fs::write(f.root.join("file"), b"content").unwrap();
    let scanner = f.scanner();
    scanner.scan(f.request()).await.unwrap();
    let item = f.item(b"file");
    let reference = RemoteFileRef {
        account_id: AccountId("account".into()),
        file_id: RemoteFileId("remote-file".into()),
    };
    let remote = RemoteFile {
        reference: reference.clone(),
        provider: ProviderId("google".into()),
        name: MetadataValue::Known("file".into()),
        kind: MetadataValue::Known(RemoteEntryKind::File),
        mime_type: MetadataValue::NotCollected,
        parent_ids: MetadataValue::Known(vec![RemoteFileId("remote-root".into())]),
        trashed: MetadataValue::Known(false),
        version: MetadataValue::Known("1".into()),
        created_time: MetadataValue::NotCollected,
        modified_time: MetadataValue::NotCollected,
        size: MetadataValue::Known(7),
        md5_checksum: MetadataValue::NotCollected,
        sha256_checksum: MetadataValue::NotCollected,
        provider_data: MetadataValue::NotCollected,
    };
    let at = now();
    let observed = crate::models::File {
        id: FileId("remote-event".into()),
        sync_id: f.request().sync_id.clone(),
        observed_at: at,
        coverage: ObservationCoverage::Partial,
        scope: FileScope::Remote(reference.clone()),
        event: FileEvent {
            action: FileEventAction::StateChanged,
            local: None,
            remote: Some(crate::models::RemoteObservation {
                source: RemoteObservationSource::Changes,
                removed: Some(false),
                time: None,
                change_type: None,
                drive_id: None,
            }),
            uncertainty: None,
            previous: None,
            current: Some(FileLocation::Remote {
                reference,
                parent_id: None,
                name: Some("file".into()),
            }),
        },
        data: Some(FileData::Remote(remote)),
    };
    let current = f
        .inventory
        .record_remote(RecordRemoteObservation {
            selector: ObservationTarget::Existing(item.id.clone()),
            expected_revision: Some(item.revision),
            observation: crate::inventory::RemoteObservation {
                observed,
                presence: PresenceState::Present,
                proof: ObservationProof {
                    source: ObservationSource::RemoteChange,
                    observed_at: at,
                    scan_or_generation: None,
                    complete_scope: false,
                },
            },
        })
        .unwrap()
        .item;
    let side = BaselineSide {
        presence: PresenceState::Present,
        name: None,
        local_name_bytes: None,
        parent_identity: None,
        version: None,
        checksum: None,
        size: Some(7),
        modified_at: None,
    };
    let confirmed = f
        .inventory
        .confirm_sync(ConfirmSyncRequest {
            item_id: current.id,
            expected_item_revision: current.revision,
            confirmed: ConfirmedState {
                baseline: SyncBaseline {
                    local: side.clone(),
                    remote: side,
                    confirmed_at: at,
                },
                status: SyncStatus::Synced,
                verified_at: at,
            },
            completed_operation: None,
            expected_operation_attempt: None,
            activity: None,
        })
        .unwrap()
        .item;
    fs::write(f.root.join("file"), b"changed content").unwrap();
    scanner.scan(f.request()).await.unwrap();
    let after = f.item(b"file");
    assert_eq!(after.id, confirmed.id);
    assert_eq!(after.baseline, confirmed.baseline);
    assert_eq!(after.remote, confirmed.remote);
    assert_eq!(after.remote_presence, confirmed.remote_presence);
    assert_eq!(after.last_synced_at, confirmed.last_synced_at);
    assert_eq!(after.status, SyncStatus::PendingReconciliation);
}
#[tokio::test]
async fn wrong_scope_and_replaced_root_never_produce_absence() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let scanner = f.scanner();
    assert_eq!(
        scanner
            .scan(ScanRequest {
                sync_id: SyncId("wrong".into())
            })
            .await
            .unwrap_err()
            .kind,
        ScannerErrorKind::WrongSyncScope
    );
    assert_eq!(scanner.status().state, ScannerState::Idle);
    let other = Fixture::new();
    assert!(
        matches!(Scanner::new(ScannerConfig{inventory:f.inventory.clone(),root:LocalRoot{sync_id:f.request().sync_id,path:other.root.clone()}}),Err(e)if e.kind==ScannerErrorKind::WrongSyncScope)
    );
    let moved = f.root.with_extension("moved");
    fs::rename(&f.root, &moved).unwrap();
    symlink(&other.root, &f.root).unwrap();
    let result = scanner.scan(f.request()).await;
    fs::remove_file(&f.root).unwrap();
    fs::rename(moved, &f.root).unwrap();
    assert_eq!(result.unwrap_err().kind, ScannerErrorKind::RootInaccessible);
    assert_eq!(scanner.status().state, ScannerState::Failed);
    assert!(f.list().is_empty());
}
#[tokio::test]
async fn concurrent_scan_status_and_future_cancellation_are_coherent() {
    let f = Fixture::new();
    fs::write(f.root.join("file"), b"data").unwrap();
    let mut scanner = f.scanner();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let worker_gate = gate.clone();
    let (started, mut entered) = tokio::sync::mpsc::unbounded_channel();
    Arc::get_mut(&mut scanner.inner).unwrap().hook = Some(Arc::new(move |stage, _| {
        if stage == Stage::Observed {
            started.send(()).unwrap();
            let mut flag = worker_gate.0.lock().unwrap();
            while !*flag {
                flag = worker_gate.1.wait(flag).unwrap();
            }
        }
    }));
    let scanner = Arc::new(scanner);
    let first = scanner.clone();
    let request = f.request();
    let task = tokio::spawn(async move { first.scan(request).await });
    entered.recv().await.unwrap();
    let start = Instant::now();
    let status = scanner.status();
    assert!(start.elapsed() < Duration::from_millis(100));
    assert_eq!(status.state, ScannerState::Running);
    assert_eq!(
        scanner.scan(f.request()).await.unwrap_err().kind,
        ScannerErrorKind::AlreadyRunning
    );
    task.abort();
    let _ = task.await;
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
    let start = Instant::now();
    while scanner.status().state == ScannerState::Running
        && start.elapsed() < Duration::from_secs(3)
    {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(scanner.status().state, ScannerState::Cancelled);
    assert_eq!(scanner.status().last_report_status, None);
    assert_eq!(scanner.status().items_recorded, 0);
    assert!(f.list().is_empty());
    assert!(scanner
        .scan(f.request())
        .await
        .unwrap()
        .ready_for_analysis());
    assert_eq!(scanner.status().last_error, None);
    assert_eq!(scanner.status().items_recorded, 1);
}
#[tokio::test]
async fn revision_race_does_not_overwrite_a_newer_inventory_observation() {
    let f = Fixture::new();
    fs::write(f.root.join("file"), b"data").unwrap();
    f.scanner().scan(f.request()).await.unwrap();
    let mut scanner = f.scanner();
    let inventory = f.inventory.clone();
    let once = AtomicBool::new(false);
    Arc::get_mut(&mut scanner.inner).unwrap().hook = Some(Arc::new(move |stage, path| {
        if stage == Stage::BeforeRecord && path == b"file" && !once.swap(true, Ordering::AcqRel) {
            let item = inventory
                .get_item(GetItemRequest {
                    selector: ItemSelector::LocalRelativePath(
                        LocalRelativePath::new(b"file".to_vec()).unwrap(),
                    ),
                })
                .unwrap()
                .item
                .unwrap();
            inventory
                .update_item_status(UpdateItemStatusRequest {
                    item_id: item.id,
                    expected_revision: item.revision,
                    change: ItemStatusChange {
                        status: SyncStatus::Conflict,
                        conflict_reason: FieldUpdate::Set("concurrent-analysis".into()),
                    },
                })
                .unwrap();
        }
    }));
    let report = scanner.scan(f.request()).await.unwrap();
    assert_eq!(report.status, ScanStatus::Partial);
    assert!(report
        .issues
        .iter()
        .any(|i| i.kind == ScanIssueKind::InventoryRecordFailed));
    assert_eq!(f.item(b"file").status, SyncStatus::Conflict);
    assert_eq!(report.items_recorded, 0);
}
#[tokio::test]
async fn disappeared_entry_and_mutated_directory_keep_scan_conservative() {
    let f = Fixture::new();
    fs::write(f.root.join("file"), b"data").unwrap();
    let mut scanner = f.scanner();
    let path = f.root.join("file");
    Arc::get_mut(&mut scanner.inner).unwrap().hook = Some(Arc::new(move |stage, relative| {
        if stage == Stage::Observed && relative == b"file" {
            fs::remove_file(&path).unwrap();
        }
    }));
    let report = scanner.scan(f.request()).await.unwrap();
    assert_eq!(report.status, ScanStatus::Partial);
    assert!(!report.ready_for_analysis());
    assert!(f.list().is_empty());
    assert!(report
        .issues
        .iter()
        .any(|i| i.kind == ScanIssueKind::EntryChanged));
}
#[tokio::test]
async fn localized_sql_failure_continues_and_general_storage_failure_is_structural() {
    let f = Fixture::new();
    fs::write(f.root.join("one"), b"1").unwrap();
    fs::write(f.root.join("two"), b"2").unwrap();
    let db = rusqlite::Connection::open(f.root.join(".drivesync/inventory.sqlite3")).unwrap();
    db.execute_batch("CREATE TRIGGER fail_one BEFORE INSERT ON local_files WHEN NEW.path=x'6f6e65' BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    let report = f.scanner().scan(f.request()).await.unwrap();
    assert_eq!(report.status, ScanStatus::Partial);
    assert_eq!(f.list().len(), 1);
    assert_eq!(f.item(b"two").local_presence, PresenceState::Present);
    db.execute_batch("DROP TRIGGER fail_one").unwrap();
    let scanner = f.scanner();
    let tx = db.unchecked_transaction().unwrap();
    tx.execute("UPDATE sync_info SET generation=generation", [])
        .unwrap();
    assert_eq!(
        scanner.scan(f.request()).await.unwrap_err().kind,
        ScannerErrorKind::InventoryUnavailable
    );
    drop(tx);
    assert!(scanner
        .scan(f.request())
        .await
        .unwrap()
        .ready_for_analysis());
}

#[tokio::test]
async fn unavailable_optional_metadata_keeps_known_values_and_allows_completed_coverage() {
    let f = Fixture::new();
    fs::write(f.root.join("file"), b"data").unwrap();
    f.scanner().scan(f.request()).await.unwrap();
    let original = f.item(b"file");
    let mut scanner = f.scanner();
    Arc::get_mut(&mut scanner.inner).unwrap().native =
        |_| Err(std::io::Error::from_raw_os_error(libc::ENOSYS));
    let report = scanner.scan(f.request()).await.unwrap();
    assert!(report.ready_for_analysis());
    assert!(report
        .issues
        .iter()
        .any(|i| i.kind == ScanIssueKind::MetadataUnavailable));
    let after = f.item(b"file");
    assert_eq!(
        after.local_latest.as_ref().unwrap().flags.statx_attributes,
        MetadataValue::Unsupported
    );
    assert_eq!(
        after.local.as_ref().unwrap().flags,
        original.local.as_ref().unwrap().flags
    );
    assert_eq!(scanner.status().issues_recorded, report.issues.len() as u64);
}
#[tokio::test]
async fn absence_reconciliation_visits_more_than_one_inventory_page() {
    let f = Fixture::new();
    for n in 0..105 {
        fs::write(f.root.join(format!("file-{n}")), b"data").unwrap();
    }
    let scanner = f.scanner();
    assert!(scanner
        .scan(f.request())
        .await
        .unwrap()
        .ready_for_analysis());
    for n in 0..105 {
        fs::remove_file(f.root.join(format!("file-{n}"))).unwrap();
    }
    let report = scanner.scan(f.request()).await.unwrap();
    assert!(report.ready_for_analysis());
    assert_eq!(report.items_recorded, 105);
    assert_eq!(f.list().len(), 105);
    assert!(f
        .list()
        .iter()
        .all(|i| i.local_presence == PresenceState::AbsentConfirmed));
}

#[tokio::test]
async fn queued_directory_replaced_with_symlink_never_enumerates_the_external_target() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let outside = Fixture::new();
    fs::create_dir(f.root.join("folder")).unwrap();
    fs::write(outside.root.join("secret"), b"outside").unwrap();
    let mut scanner = f.scanner();
    let folder = f.root.join("folder");
    let moved = outside.root.join("moved");
    let target = outside.root.clone();
    let once = AtomicBool::new(false);
    Arc::get_mut(&mut scanner.inner).unwrap().hook = Some(Arc::new(move |stage, path| {
        if stage == Stage::DirectoryRead && path.is_empty() && !once.swap(true, Ordering::AcqRel) {
            fs::rename(&folder, &moved).unwrap();
            symlink(&target, &folder).unwrap();
        }
    }));
    let report = scanner.scan(f.request()).await.unwrap();
    assert_eq!(report.status, ScanStatus::Partial);
    assert!(report
        .issues
        .iter()
        .any(|i| i.kind == ScanIssueKind::EntryChanged));
    assert_eq!(f.list().len(), 1);
    assert_eq!(
        f.list()[0].local.as_ref().unwrap().relative_path.as_bytes(),
        b"folder"
    );
    assert_eq!(fs::read(outside.root.join("secret")).unwrap(), b"outside");
}

#[tokio::test]
async fn former_directory_replaced_by_a_leaf_confirms_old_locations_without_following_it() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let outside = Fixture::new();
    fs::create_dir(f.root.join("folder")).unwrap();
    fs::write(f.root.join("folder/child"), b"old").unwrap();
    fs::write(outside.root.join("child"), b"outside").unwrap();
    let scanner = f.scanner();
    scanner.scan(f.request()).await.unwrap();
    fs::remove_dir_all(f.root.join("folder")).unwrap();
    symlink(&outside.root, f.root.join("folder")).unwrap();
    let report = scanner.scan(f.request()).await.unwrap();
    assert!(report.ready_for_analysis());
    assert_eq!(
        f.item(b"folder").local.as_ref().unwrap().kind,
        MetadataValue::Known(LocalEntryKind::Symlink)
    );
    assert_eq!(
        f.item(b"folder/child").local_presence,
        PresenceState::AbsentConfirmed
    );
    assert_eq!(fs::read(outside.root.join("child")).unwrap(), b"outside");
}
