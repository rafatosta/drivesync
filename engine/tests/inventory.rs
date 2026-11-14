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
