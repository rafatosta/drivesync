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

use drivesync_engine::models as m;
fn remote_file(event: &str, id: &str, name: m::MetadataValue<String>) -> m::File {
    let remote = m::RemoteFile {
        reference: m::RemoteFileRef {
            account_id: AccountId("account".into()),
            file_id: m::RemoteFileId(id.into()),
        },
        provider: m::ProviderId("google".into()),
        name: name.clone(),
        kind: m::MetadataValue::Known(m::RemoteEntryKind::File),
        mime_type: m::MetadataValue::NotCollected,
        parent_ids: m::MetadataValue::Known(vec![m::RemoteFileId("root".into())]),
        trashed: m::MetadataValue::Known(false),
        version: m::MetadataValue::Known("1".into()),
        created_time: m::MetadataValue::NotCollected,
        modified_time: m::MetadataValue::NotCollected,
        size: m::MetadataValue::Known(u64::MAX),
        md5_checksum: m::MetadataValue::NotCollected,
        sha256_checksum: m::MetadataValue::NotCollected,
        provider_data: m::MetadataValue::NotCollected,
    };
    let reference = remote.reference.clone();
    m::File {
        id: m::FileId(event.into()),
        sync_id: SyncId("sync".into()),
        observed_at: m::Timestamp { unix_millis: 10 },
        scope: m::FileScope::Remote(reference.clone()),
        coverage: m::ObservationCoverage::Partial,
        event: m::FileEvent {
            kind: m::FileEventKind::Observed,
            local: None,
            remote: Some(m::RemoteObservation {
                source: m::RemoteObservationSource::Changes,
                removed: Some(false),
                time: None,
                change_type: None,
                drive_id: None,
            }),
            uncertainty: None,
            previous: None,
            current: Some(m::FileLocation::Remote {
                reference,
                parent_id: Some(m::RemoteFileId("root".into())),
                name: if let m::MetadataValue::Known(name) = name {
                    Some(name)
                } else {
                    None
                },
            }),
        },
        data: Some(m::FileData::Remote(remote)),
    }
}
fn remote_request(
    file: m::File,
    target: ObservationTarget,
    revision: Option<u64>,
) -> RecordRemoteObservation {
    RecordRemoteObservation {
        selector: target,
        expected_revision: revision,
        observation: RemoteObservation {
            observed: file,
            presence: PresenceState::Present,
            proof: ObservationProof {
                source: ObservationSource::RemoteChange,
                observed_at: m::Timestamp { unix_millis: 10 },
                scan_or_generation: None,
                complete_scope: false,
            },
        },
    }
}
#[test]
fn fields_keep_known_values_and_latest_read_state_with_idempotence_and_reopen() {
    let tree = Tree::new();
    let inventory = InventoryManager::open(tree.request()).unwrap().inventory;
    let first = inventory
        .record_remote(remote_request(
            remote_file(
                "event-1",
                "remote",
                m::MetadataValue::Known("original".into()),
            ),
            ObservationTarget::NewItem,
            None,
        ))
        .unwrap();
    let id = first.item.id;
    let request = remote_request(
        remote_file("event-2", "remote", m::MetadataValue::PermissionDenied),
        ObservationTarget::Existing(id.clone()),
        Some(1),
    );
    let updated = inventory.record_remote(request.clone()).unwrap();
    assert_eq!(
        updated.item.remote.as_ref().unwrap().name,
        m::MetadataValue::Known("original".into())
    );
    assert_eq!(
        updated.item.remote_latest.as_ref().unwrap().name,
        m::MetadataValue::PermissionDenied
    );
    assert_eq!(
        updated.item.remote.as_ref().unwrap().size,
        m::MetadataValue::Known(u64::MAX)
    );
    assert_eq!(updated.item.revision, 2);
    assert_eq!(inventory.record_remote(request).unwrap().item.revision, 2);
    assert!(updated.item.baseline.is_none());
    assert_eq!(
        inventory
            .record_remote(remote_request(
                remote_file("event-3", "remote", m::MetadataValue::Known("new".into())),
                ObservationTarget::Existing(id.clone()),
                Some(1)
            ))
            .unwrap_err()
            .kind,
        InventoryError::VersionMismatch
    );
    drop(inventory);
    let reopened = InventoryManager::open(tree.request()).unwrap().inventory;
    let item = reopened
        .get_item(GetItemRequest {
            selector: ItemSelector::RemoteRef(m::RemoteFileRef {
                account_id: AccountId("account".into()),
                file_id: m::RemoteFileId("remote".into()),
            }),
        })
        .unwrap()
        .item
        .unwrap();
    assert_eq!(item, updated.item);
    assert_eq!(item.id, id);
}
#[test]
fn item_pagination_is_bound_to_filter_and_insertion_high_water_mark() {
    let tree = Tree::new();
    let inventory = InventoryManager::open(tree.request()).unwrap().inventory;
    for n in 0..2 {
        inventory
            .record_remote(remote_request(
                remote_file(
                    &format!("event-{n}"),
                    &format!("remote-{n}"),
                    m::MetadataValue::NotCollected,
                ),
                ObservationTarget::NewItem,
                None,
            ))
            .unwrap();
    }
    let first = inventory
        .list_items(ListItemsRequest {
            filter: ItemFilter::default(),
            pagination: Pagination {
                limit: 1,
                after: None,
            },
        })
        .unwrap()
        .page;
    inventory
        .record_remote(remote_request(
            remote_file("event-2", "remote-2", m::MetadataValue::NotCollected),
            ObservationTarget::NewItem,
            None,
        ))
        .unwrap();
    let second = inventory
        .list_items(ListItemsRequest {
            filter: ItemFilter::default(),
            pagination: Pagination {
                limit: 1,
                after: first.next_cursor.clone(),
            },
        })
        .unwrap()
        .page;
    assert_eq!(second.entries.len(), 1);
    assert!(second.next_cursor.is_none());
    assert_ne!(first.entries[0].id, second.entries[0].id);
    let filter = ItemFilter {
        statuses: vec![SyncStatus::Conflict],
        ..Default::default()
    };
    assert_eq!(
        inventory
            .list_items(ListItemsRequest {
                filter,
                pagination: Pagination {
                    limit: 1,
                    after: first.next_cursor
                }
            })
            .unwrap_err()
            .kind,
        InventoryError::InvalidCursor
    );
}
#[test]
fn unconfirmed_absence_and_wrong_account_never_destroy_known_metadata() {
    let tree = Tree::new();
    let inventory = InventoryManager::open(tree.request()).unwrap().inventory;
    let first = inventory
        .record_remote(remote_request(
            remote_file(
                "event-1",
                "remote",
                m::MetadataValue::Known("original".into()),
            ),
            ObservationTarget::NewItem,
            None,
        ))
        .unwrap()
        .item;
    let mut request = remote_request(
        remote_file("absence", "remote", m::MetadataValue::NotCollected),
        ObservationTarget::Existing(first.id.clone()),
        Some(1),
    );
    request.observation.observed.data = None;
    request.observation.observed.event.current = None;
    request.observation.presence = PresenceState::AbsentConfirmed;
    assert_eq!(
        inventory.record_remote(request.clone()).unwrap_err().kind,
        InventoryError::UnconfirmedAbsence
    );
    request.observation.proof = ObservationProof {
        source: ObservationSource::RemoteListing,
        observed_at: m::Timestamp { unix_millis: 11 },
        scan_or_generation: Some("verified-scan".into()),
        complete_scope: true,
    };
    let absent = inventory.record_remote(request).unwrap().item;
    assert_eq!(absent.remote_presence, PresenceState::AbsentConfirmed);
    assert_eq!(absent.remote, first.remote);
    assert!(absent.baseline.is_none());
    assert_eq!(
        inventory
            .get_item(GetItemRequest {
                selector: ItemSelector::RemoteRef(m::RemoteFileRef {
                    account_id: AccountId("other".into()),
                    file_id: m::RemoteFileId("remote".into())
                })
            })
            .unwrap_err()
            .kind,
        InventoryError::InvalidIdentity
    );
}

fn local_file(tree: &Tree, event: &str) -> m::File {
    use drivesync_engine::detectors::*;
    let collector = LocalMetadataCollector::new(LocalDetectorScope {
        account_id: AccountId("account".into()),
        sync_id: SyncId("sync".into()),
        local_root: tree.0.clone(),
    })
    .unwrap();
    let path = m::LocalRelativePath::new(b"local-file".to_vec()).unwrap();
    let local = collector
        .collect(LocalMetadataRequest {
            relative_path: path.clone(),
            cancellation: LocalCancellation::new(),
        })
        .unwrap()
        .file
        .unwrap();
    m::File {
        id: m::FileId(event.into()),
        sync_id: SyncId("sync".into()),
        observed_at: m::Timestamp { unix_millis: 10 },
        scope: m::FileScope::Local(path.clone()),
        coverage: m::ObservationCoverage::Partial,
        event: m::FileEvent {
            kind: m::FileEventKind::Observed,
            local: Some(m::LocalObservation {
                capture_id: "test".into(),
                mask: libc::IN_MODIFY,
                cookie: 0,
            }),
            remote: None,
            uncertainty: None,
            previous: None,
            current: Some(m::FileLocation::Local(path)),
        },
        data: Some(m::FileData::Local(local)),
    }
}
#[test]
fn local_metadata_is_retained_and_link_conflicts_roll_back_all_patches() {
    let tree = Tree::new();
    let inventory = InventoryManager::open(tree.request()).unwrap().inventory;
    std::fs::write(tree.0.join("local-file"), b"content").unwrap();
    let observed = local_file(&tree, "local-event");
    let request = RecordLocalObservation {
        selector: ObservationTarget::NewItem,
        expected_revision: None,
        observation: LocalObservation {
            observed: observed.clone(),
            presence: PresenceState::Present,
            proof: ObservationProof {
                source: ObservationSource::LocalEvent,
                observed_at: m::Timestamp { unix_millis: 10 },
                scan_or_generation: None,
                complete_scope: false,
            },
        },
    };
    let local = inventory.record_local(request).unwrap().item;
    let mut recent = observed;
    recent.id = m::FileId("local-denied".into());
    if let Some(m::FileData::Local(data)) = &mut recent.data {
        data.mode = m::MetadataValue::PermissionDenied;
    }
    let updated = inventory
        .record_local(RecordLocalObservation {
            selector: ObservationTarget::Existing(local.id.clone()),
            expected_revision: Some(1),
            observation: LocalObservation {
                observed: recent,
                presence: PresenceState::Present,
                proof: ObservationProof {
                    source: ObservationSource::LocalEvent,
                    observed_at: m::Timestamp { unix_millis: 11 },
                    scan_or_generation: None,
                    complete_scope: false,
                },
            },
        })
        .unwrap()
        .item;
    assert_eq!(
        updated.local.as_ref().unwrap().mode,
        local.local.as_ref().unwrap().mode
    );
    assert_eq!(
        updated.local_latest.as_ref().unwrap().mode,
        m::MetadataValue::PermissionDenied
    );
    inventory
        .record_remote(remote_request(
            remote_file("remote-event", "occupied", m::MetadataValue::NotCollected),
            ObservationTarget::NewItem,
            None,
        ))
        .unwrap();
    let reference = m::RemoteFileRef {
        account_id: AccountId("account".into()),
        file_id: m::RemoteFileId("occupied".into()),
    };
    assert_eq!(
        inventory
            .link_item(LinkItemRequest {
                item_id: local.id.clone(),
                expected_revision: 2,
                changes: ItemLinksChange {
                    local_file_id: FieldUpdate::Set(m::LocalFileId("new-local-id".into())),
                    remote_ref: FieldUpdate::Set(reference)
                }
            })
            .unwrap_err()
            .kind,
        InventoryError::DuplicateAssociation
    );
    assert!(inventory
        .get_item(GetItemRequest {
            selector: ItemSelector::LocalId(m::LocalFileId("new-local-id".into()))
        })
        .unwrap()
        .item
        .is_none());
    let linked = inventory
        .link_item(LinkItemRequest {
            item_id: local.id.clone(),
            expected_revision: 2,
            changes: ItemLinksChange {
                local_file_id: FieldUpdate::Set(m::LocalFileId("new-local-id".into())),
                remote_ref: FieldUpdate::Set(m::RemoteFileRef {
                    account_id: AccountId("account".into()),
                    file_id: m::RemoteFileId("reference-only".into()),
                }),
            },
        })
        .unwrap()
        .item;
    assert!(linked.remote.is_none());
    assert!(linked.remote_ref.is_some());
    assert_eq!(linked.revision, 3);
    assert_eq!(
        inventory
            .get_item(GetItemRequest {
                selector: ItemSelector::LocalRelativePath(
                    m::LocalRelativePath::new(b"local-file".to_vec()).unwrap()
                )
            })
            .unwrap()
            .item
            .unwrap()
            .id,
        local.id
    );
}

fn timestamp(unix_millis: i64) -> m::Timestamp {
    m::Timestamp { unix_millis }
}
fn item(inventory: &InventoryManager, id: &SyncItemId) -> SyncItem {
    inventory
        .get_item(GetItemRequest {
            selector: ItemSelector::Id(id.clone()),
        })
        .unwrap()
        .item
        .unwrap()
}
fn create_remote(inventory: &InventoryManager, suffix: &str) -> SyncItem {
    inventory
        .record_remote(remote_request(
            remote_file(
                &format!("event-{suffix}"),
                suffix,
                m::MetadataValue::Known(suffix.into()),
            ),
            ObservationTarget::NewItem,
            None,
        ))
        .unwrap()
        .item
}
fn enqueue_request(item: &SyncItem, key: &str) -> EnqueueOperationRequest {
    EnqueueOperationRequest {
        item_id: item.id.clone(),
        expected_item_revision: item.revision,
        spec: NewSyncOperation {
            direction: OperationDirection::Download,
            kind: OperationKind::Create,
            expected_remote_version: Some("1".into()),
            created_at: timestamp(10),
            deduplication_key: Some(key.into()),
        },
    }
}
fn claim(
    inventory: &InventoryManager,
    op: &SyncOperation,
    at: i64,
) -> InventoryResult<ClaimOperationResult> {
    inventory.claim_operation(ClaimOperationRequest {
        operation_id: op.id.clone(),
        expected_state: op.state.clone(),
        claimed_at: timestamp(at),
    })
}
#[test]
fn operation_intent_deduplicates_and_rechecks_revision_and_remote_version() {
    let tree = Tree::new();
    let inventory = InventoryManager::open(tree.request()).unwrap().inventory;
    let first = create_remote(&inventory, "remote");
    let request = enqueue_request(&first, "download");
    let op = inventory
        .enqueue_operation(request.clone())
        .unwrap()
        .operation;
    assert_eq!(
        inventory
            .enqueue_operation(request.clone())
            .unwrap()
            .operation,
        op
    );
    let mut different = request.clone();
    different.spec.kind = OperationKind::Delete;
    assert_eq!(
        inventory.enqueue_operation(different).unwrap_err().kind,
        InventoryError::VersionMismatch
    );
    let updated = inventory
        .record_remote(remote_request(
            remote_file("changed", "remote", m::MetadataValue::NotCollected),
            ObservationTarget::Existing(first.id.clone()),
            Some(1),
        ))
        .unwrap()
        .item;
    assert_eq!(
        claim(&inventory, &op, 11).unwrap_err().kind,
        InventoryError::VersionMismatch
    );
    assert_eq!(inventory.enqueue_operation(request).unwrap().operation, op);
    let mut request = enqueue_request(&updated, "version");
    request.spec.expected_remote_version = Some("2".into());
    let wrong = inventory.enqueue_operation(request).unwrap().operation;
    assert_eq!(
        claim(&inventory, &wrong, 11).unwrap_err().kind,
        InventoryError::VersionMismatch
    );
    let page = inventory
        .list_operations(ListOperationsRequest {
            filter: OperationFilter {
                item_id: Some(first.id),
                states: vec![OperationState::Pending],
                direction: Some(OperationDirection::Download),
                eligible_before: Some(timestamp(11)),
            },
            pagination: Pagination::default(),
        })
        .unwrap()
        .page;
    assert_eq!(page.entries.len(), 2);
}
#[test]
fn independent_connections_cannot_claim_the_same_work_and_restart_preserves_running() {
    use std::sync::{Arc, Barrier};
    let tree = Tree::new();
    let inventory = InventoryManager::open(tree.request()).unwrap().inventory;
    let file = create_remote(&inventory, "remote");
    let op = inventory
        .enqueue_operation(enqueue_request(&file, "download"))
        .unwrap()
        .operation;
    let another = InventoryManager::open(tree.request()).unwrap().inventory;
    let barrier = Arc::new(Barrier::new(2));
    let mut workers = Vec::new();
    for manager in [inventory, another] {
        let barrier = barrier.clone();
        let operation = op.clone();
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            claim(&manager, &operation, 11)
        }));
    }
    let results = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results.into_iter().find_map(Result::err).unwrap().kind,
        InventoryError::VersionMismatch
    );
    let reopened = InventoryManager::open(tree.request()).unwrap().inventory;
    let operation = &reopened
        .list_operations(ListOperationsRequest {
            filter: OperationFilter::default(),
            pagination: Pagination::default(),
        })
        .unwrap()
        .page
        .entries[0];
    assert_eq!(operation.state, OperationState::Running);
    assert_eq!(operation.attempt_count, 1);
    assert_eq!(
        claim(&reopened, operation, 12).unwrap_err().kind,
        InventoryError::InvalidTransition
    );
    let second = reopened
        .enqueue_operation(enqueue_request(&file, "second"))
        .unwrap()
        .operation;
    assert_eq!(
        claim(&reopened, &second, 12).unwrap_err().kind,
        InventoryError::InvalidTransition
    );
}
#[test]
fn retry_is_due_only_and_old_attempts_cannot_update_a_new_claim() {
    let tree = Tree::new();
    let inventory = InventoryManager::open(tree.request()).unwrap().inventory;
    let file = create_remote(&inventory, "remote");
    let op = inventory
        .enqueue_operation(enqueue_request(&file, "download"))
        .unwrap()
        .operation;
    let running = claim(&inventory, &op, 11).unwrap().operation;
    let retry = inventory
        .update_operation(UpdateOperationRequest {
            operation_id: op.id.clone(),
            expected_state: OperationState::Running,
            expected_attempt_count: 1,
            transition: OperationTransition::Retry {
                next_attempt_at: timestamp(50),
                error_kind: "network.offline".into(),
            },
        })
        .unwrap()
        .operation;
    assert_eq!(
        claim(&inventory, &retry, 49).unwrap_err().kind,
        InventoryError::InvalidTransition
    );
    let newer = claim(&inventory, &retry, 50).unwrap().operation;
    assert_eq!(newer.attempt_count, 2);
    assert_eq!(
        inventory
            .update_operation(UpdateOperationRequest {
                operation_id: op.id.clone(),
                expected_state: running.state,
                expected_attempt_count: 1,
                transition: OperationTransition::Fail {
                    failed_at: timestamp(51),
                    error_kind: "network.timeout".into()
                }
            })
            .unwrap_err()
            .kind,
        InventoryError::VersionMismatch
    );
    let failed = inventory
        .update_operation(UpdateOperationRequest {
            operation_id: op.id.clone(),
            expected_state: OperationState::Running,
            expected_attempt_count: 2,
            transition: OperationTransition::Fail {
                failed_at: timestamp(51),
                error_kind: "network.timeout".into(),
            },
        })
        .unwrap()
        .operation;
    let requeued = inventory
        .update_operation(UpdateOperationRequest {
            operation_id: op.id,
            expected_state: failed.state,
            expected_attempt_count: 2,
            transition: OperationTransition::Requeue {
                reason: "effect.verified_absent".into(),
            },
        })
        .unwrap()
        .operation;
    assert_eq!(
        claim(&inventory, &requeued, 52)
            .unwrap()
            .operation
            .attempt_count,
        3
    );
}
fn attach_local(tree: &Tree, inventory: &InventoryManager, remote: &SyncItem) -> SyncItem {
    std::fs::write(tree.0.join("local-file"), b"content").unwrap();
    inventory
        .record_local(RecordLocalObservation {
            selector: ObservationTarget::Existing(remote.id.clone()),
            expected_revision: Some(remote.revision),
            observation: LocalObservation {
                observed: local_file(tree, "local-attached"),
                presence: PresenceState::Present,
                proof: ObservationProof {
                    source: ObservationSource::LocalEvent,
                    observed_at: timestamp(11),
                    scan_or_generation: None,
                    complete_scope: false,
                },
            },
        })
        .unwrap()
        .item
}
fn baseline_side() -> BaselineSide {
    BaselineSide {
        presence: PresenceState::Present,
        name: None,
        local_name_bytes: None,
        parent_identity: None,
        version: None,
        checksum: None,
        size: None,
        modified_at: None,
    }
}
fn new_activity(id: &SyncItemId, at: i64) -> NewActivity {
    NewActivity {
        item_id: id.clone(),
        operation_id: None,
        event_type: ActivityType::ConflictDetected,
        origin: ActivityOrigin::System,
        outcome: ActivityOutcome::Informational,
        occurred_at: timestamp(at),
        description: Some("verified fact".into()),
        error_kind: None,
        item_name: Some("historical name".into()),
        item_remote_id: None,
        item_local_path: Some(vec![b'x', 0xff]),
    }
}
fn confirmation(file: &SyncItem, operation: &SyncOperation) -> ConfirmSyncRequest {
    let mut activity = new_activity(&file.id, 20);
    activity.operation_id = Some(operation.id.clone());
    activity.event_type = ActivityType::Download;
    activity.outcome = ActivityOutcome::Completed;
    let mut local = baseline_side();
    local.local_name_bytes = Some(vec![b'x', 0xff]);
    local.size = Some(u64::MAX);
    let mut remote = baseline_side();
    remote.size = Some(u64::MAX);
    remote.version = Some("1".into());
    ConfirmSyncRequest {
        item_id: file.id.clone(),
        expected_item_revision: file.revision,
        confirmed: ConfirmedState {
            baseline: SyncBaseline {
                local,
                remote,
                confirmed_at: timestamp(20),
            },
            status: SyncStatus::Synced,
            verified_at: timestamp(20),
        },
        completed_operation: Some(operation.id.clone()),
        expected_operation_attempt: Some(operation.attempt_count),
        activity: Some(activity),
    }
}
#[test]
fn confirmation_is_atomic_replayable_and_does_not_advance_on_new_observations() {
    let tree = Tree::new();
    let inventory = InventoryManager::open(tree.request()).unwrap().inventory;
    let first = create_remote(&inventory, "remote");
    let file = attach_local(&tree, &inventory, &first);
    let op = inventory
        .enqueue_operation(enqueue_request(&file, "download"))
        .unwrap()
        .operation;
    let running = claim(&inventory, &op, 12).unwrap().operation;
    let request = confirmation(&file, &running);
    let mut invalid = request.clone();
    invalid.activity.as_mut().unwrap().error_kind = Some("contains a secret/token".into());
    assert_eq!(
        inventory.confirm_sync(invalid).unwrap_err().kind,
        InventoryError::InvalidArgument
    );
    assert_eq!(item(&inventory, &file.id), file);
    assert_eq!(
        inventory
            .list_operations(ListOperationsRequest {
                filter: OperationFilter::default(),
                pagination: Pagination::default()
            })
            .unwrap()
            .page
            .entries[0]
            .state,
        OperationState::Running
    );
    assert!(inventory
        .list_activities(ListActivitiesRequest {
            filter: ActivityFilter::default(),
            pagination: Pagination::default()
        })
        .unwrap()
        .page
        .entries
        .is_empty());
    // Force a real SQL failure after the operation and baseline have already been updated.
    let db = rusqlite::Connection::open(tree.0.join(".drivesync/inventory.sqlite3")).unwrap();
    db.execute_batch("CREATE TRIGGER reject_history BEFORE INSERT ON activities BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert!(inventory.confirm_sync(request.clone()).is_err());
    assert_eq!(item(&inventory, &file.id), file);
    assert_eq!(
        inventory
            .list_operations(ListOperationsRequest {
                filter: OperationFilter::default(),
                pagination: Pagination::default()
            })
            .unwrap()
            .page
            .entries[0]
            .state,
        OperationState::Running
    );
    db.execute_batch("DROP TRIGGER reject_history").unwrap();
    drop(db);
    let result = inventory.confirm_sync(request.clone()).unwrap();
    assert_eq!(result.item.revision, file.revision + 1);
    assert_eq!(result.item.status, SyncStatus::Synced);
    assert_eq!(
        result.item.baseline.as_ref().unwrap().local.size,
        Some(u64::MAX)
    );
    assert_eq!(
        result.completed_operation.as_ref().unwrap().state,
        OperationState::Completed
    );
    assert_eq!(inventory.confirm_sync(request.clone()).unwrap(), result);
    let mut different = request.clone();
    different.confirmed.verified_at = timestamp(21);
    assert_eq!(
        inventory.confirm_sync(different).unwrap_err().kind,
        InventoryError::VersionMismatch
    );
    let next = inventory
        .record_remote(remote_request(
            remote_file("later", "remote", m::MetadataValue::Known("renamed".into())),
            ObservationTarget::Existing(file.id.clone()),
            Some(result.item.revision),
        ))
        .unwrap()
        .item;
    assert_eq!(next.baseline, result.item.baseline);
    assert_eq!(
        inventory
            .confirm_sync(request.clone())
            .unwrap()
            .item
            .revision,
        next.revision
    );
    drop(inventory);
    let reopened = InventoryManager::open(tree.request()).unwrap().inventory;
    assert_eq!(item(&reopened, &file.id), next);
    assert_eq!(
        reopened.confirm_sync(request).unwrap().activity,
        result.activity
    );
    assert_eq!(
        reopened
            .list_activities(ListActivitiesRequest {
                filter: ActivityFilter::default(),
                pagination: Pagination::default()
            })
            .unwrap()
            .page
            .entries
            .len(),
        1
    );
}
#[test]
fn activity_pagination_is_chronological_with_ties_filters_and_late_insertions() {
    let tree = Tree::new();
    let inventory = InventoryManager::open(tree.request()).unwrap().inventory;
    let file = create_remote(&inventory, "remote");
    for at in [30, 10, 20, 20] {
        inventory
            .record_activity(RecordActivityRequest {
                activity: new_activity(&file.id, at),
            })
            .unwrap();
    }
    let filter = ActivityFilter {
        item_id: Some(file.id.clone()),
        event_types: vec![ActivityType::ConflictDetected],
        outcomes: vec![ActivityOutcome::Informational],
        from: Some(timestamp(10)),
        until: Some(timestamp(30)),
        ..Default::default()
    };
    let first = inventory
        .list_activities(ListActivitiesRequest {
            filter: filter.clone(),
            pagination: Pagination {
                limit: 2,
                after: None,
            },
        })
        .unwrap()
        .page;
    assert_eq!(
        first
            .entries
            .iter()
            .map(|a| a.occurred_at.unix_millis)
            .collect::<Vec<_>>(),
        vec![10, 20]
    );
    inventory
        .record_activity(RecordActivityRequest {
            activity: new_activity(&file.id, 15),
        })
        .unwrap();
    let second = inventory
        .list_activities(ListActivitiesRequest {
            filter: filter.clone(),
            pagination: Pagination {
                limit: 2,
                after: first.next_cursor.clone(),
            },
        })
        .unwrap()
        .page;
    assert_eq!(
        second
            .entries
            .iter()
            .map(|a| a.occurred_at.unix_millis)
            .collect::<Vec<_>>(),
        vec![20, 30]
    );
    assert!(second.next_cursor.is_none());
    assert_ne!(first.entries[1].id, second.entries[0].id);
    assert_eq!(
        inventory
            .list_activities(ListActivitiesRequest {
                filter: ActivityFilter::default(),
                pagination: Pagination {
                    limit: 2,
                    after: first.next_cursor
                }
            })
            .unwrap_err()
            .kind,
        InventoryError::InvalidCursor
    );
    assert_eq!(second.entries[0].item_local_path, Some(vec![b'x', 0xff]));
}
#[test]
fn activities_cannot_reference_another_items_operation_or_complete_running_work() {
    let tree = Tree::new();
    let inventory = InventoryManager::open(tree.request()).unwrap().inventory;
    let file = create_remote(&inventory, "remote");
    let other = create_remote(&inventory, "other");
    let op = inventory
        .enqueue_operation(enqueue_request(&file, "download"))
        .unwrap()
        .operation;
    let mut a = new_activity(&other.id, 20);
    a.operation_id = Some(op.id.clone());
    assert_eq!(
        inventory
            .record_activity(RecordActivityRequest { activity: a })
            .unwrap_err()
            .kind,
        InventoryError::InvalidIdentity
    );
    let mut a = new_activity(&file.id, 20);
    a.operation_id = Some(op.id);
    a.outcome = ActivityOutcome::Completed;
    assert_eq!(
        inventory
            .record_activity(RecordActivityRequest { activity: a })
            .unwrap_err()
            .kind,
        InventoryError::InvalidTransition
    );
}

#[test]
fn confirmation_requires_known_equal_presence_and_preserves_tombstone_history() {
    let tree = Tree::new();
    let inventory = InventoryManager::open(tree.request()).unwrap().inventory;
    let first = create_remote(&inventory, "remote");
    let file = attach_local(&tree, &inventory, &first);
    let mut request = ConfirmSyncRequest {
        item_id: file.id.clone(),
        expected_item_revision: file.revision,
        confirmed: ConfirmedState {
            baseline: SyncBaseline {
                local: baseline_side(),
                remote: baseline_side(),
                confirmed_at: timestamp(20),
            },
            status: SyncStatus::Synced,
            verified_at: timestamp(20),
        },
        completed_operation: None,
        expected_operation_attempt: None,
        activity: None,
    };
    request.confirmed.baseline.local.presence = PresenceState::Unknown;
    assert_eq!(
        inventory.confirm_sync(request.clone()).unwrap_err().kind,
        InventoryError::InvalidArgument
    );
    request.confirmed.baseline.local.presence = PresenceState::AbsentConfirmed;
    request.confirmed.baseline.remote.presence = PresenceState::AbsentConfirmed;
    assert_eq!(
        inventory.confirm_sync(request.clone()).unwrap_err().kind,
        InventoryError::MissingObservation
    );
    let mut local = local_file(&tree, "local-absence");
    local.data = None;
    local.event.current = None;
    let proof = ObservationProof {
        source: ObservationSource::LocalScan,
        observed_at: timestamp(19),
        scan_or_generation: Some("complete-local-scan".into()),
        complete_scope: true,
    };
    let local_absent = inventory
        .record_local(RecordLocalObservation {
            selector: ObservationTarget::Existing(file.id.clone()),
            expected_revision: Some(file.revision),
            observation: LocalObservation {
                observed: local,
                presence: PresenceState::AbsentConfirmed,
                proof: proof.clone(),
            },
        })
        .unwrap()
        .item;
    let mut remote = remote_request(
        remote_file("remote-absence", "remote", m::MetadataValue::NotCollected),
        ObservationTarget::Existing(file.id.clone()),
        Some(local_absent.revision),
    );
    remote.observation.observed.data = None;
    remote.observation.observed.event.current = None;
    remote.observation.presence = PresenceState::AbsentConfirmed;
    remote.observation.proof = ObservationProof {
        source: ObservationSource::RemoteListing,
        ..proof
    };
    let absent = inventory.record_remote(remote).unwrap().item;
    request.expected_item_revision = absent.revision;
    request.activity = Some(new_activity(&file.id, 20));
    let confirmed = inventory.confirm_sync(request).unwrap().item;
    assert_eq!(
        confirmed.baseline.as_ref().unwrap().local.presence,
        PresenceState::AbsentConfirmed
    );
    assert!(confirmed.local.is_some());
    assert!(confirmed.remote.is_some());
    drop(inventory);
    let reopened = InventoryManager::open(tree.request()).unwrap().inventory;
    assert_eq!(item(&reopened, &file.id), confirmed);
    assert_eq!(
        reopened
            .list_activities(ListActivitiesRequest {
                filter: ActivityFilter {
                    item_id: Some(file.id),
                    ..Default::default()
                },
                pagination: Pagination::default()
            })
            .unwrap()
            .page
            .entries
            .len(),
        1
    );
}
#[test]
fn materialization_cannot_finish_with_outstanding_downloads() {
    let tree = Tree::new();
    let inventory = InventoryManager::open(tree.request()).unwrap().inventory;
    let file = create_remote(&inventory, "remote");
    inventory
        .enqueue_operation(enqueue_request(&file, "download"))
        .unwrap();
    assert_eq!(
        inventory
            .mark_initial_download_complete(MarkInitialDownloadCompleteRequest {
                expected_generation: 1,
                expected_complete: false
            })
            .unwrap_err()
            .kind,
        InventoryError::InvalidTransition
    );
    assert!(
        !inventory
            .get_sync_info(GetSyncInfoRequest {})
            .unwrap()
            .info
            .initial_download_complete
    );
}
