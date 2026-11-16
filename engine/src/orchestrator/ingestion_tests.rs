//! Remote/local ingestion recovery with real Inventory and durable Analyzer decisions.
use super::*;
use crate::models as m;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "orchestrator-ingestion-{}-{}",
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
fn setup() -> (Tree, std::sync::Arc<InventoryManager>, SyncIdentity) {
    let tree = Tree::new();
    let identity = tree.request().expected;
    let inventory = std::sync::Arc::new(InventoryManager::open(tree.request()).unwrap().inventory);
    (tree, inventory, identity)
}
fn item(inventory: &InventoryManager, id: SyncItemId) -> SyncItem {
    inventory
        .get_item(GetItemRequest {
            selector: ItemSelector::Id(id),
        })
        .unwrap()
        .item
        .unwrap()
}
#[tokio::test]
async fn remote_observation_reaches_analyzer_and_redelivery_reuses_inventory_receipts() {
    use crate::analyzer::*;
    let (_tree, inventory, identity) = setup();
    let file = remote_file(
        "remote-event",
        "remote",
        m::MetadataValue::Known("remote document".into()),
    );
    let first = ingest(&inventory, &file, &identity).unwrap();
    assert!(!first.scan_local);
    let id = first.item.unwrap();
    let analyzer = Analyzer::new(AnalyzerConfig {
        provider: crate::models::ProviderId("google".into()),
        identity: identity.clone(),
        dependencies: AnalyzerDependencies {
            inventory: inventory.clone(),
            execution: None,
        },
    })
    .unwrap();
    let evidence = AnalysisEvidence {
        baseline_confirmed: false,
        local_scope_complete: false,
        remote_scope_complete: true,
        local_content: EvidenceRelation::Unknown,
        remote_content: EvidenceRelation::Unknown,
        current_content: EvidenceRelation::Unknown,
        local_parent: None,
        current_parents: EvidenceRelation::Unknown,
    };
    let first = analyzer
        .analyze(AnalyzeRequest {
            directive: None,
            sync_id: identity.sync_id.clone(),
            item: item(&inventory, id.clone()),
            evidence: evidence.clone(),
            recorded_at: Timestamp { unix_millis: 20 },
        })
        .await
        .unwrap()
        .analysis;
    let replay = ingest(&inventory, &file, &identity).unwrap();
    assert_eq!(replay.item, Some(id.clone()));
    assert_eq!(
        item(&inventory, id.clone()).revision,
        first.resulting_item_revision
    );
    let second = analyzer
        .analyze(AnalyzeRequest {
            directive: None,
            sync_id: identity.sync_id.clone(),
            item: item(&inventory, id),
            evidence,
            recorded_at: Timestamp { unix_millis: 21 },
        })
        .await
        .unwrap()
        .analysis;
    assert_eq!(first, second);
    assert_eq!(first.disposition, AnalysisDisposition::PendingEvidence);
    assert_eq!(first.decisions.len(), 1);
    assert_eq!(
        first.decisions[0].intent,
        AnalysisIntent {
            direction: OperationDirection::Download,
            kind: OperationKind::Create,
        }
    );
    let operations = inventory
        .list_operations(ListOperationsRequest {
            filter: Default::default(),
            pagination: Default::default(),
        })
        .unwrap()
        .page
        .entries;
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].state, OperationState::Blocked);
    assert_eq!(operations[0].attempt_count, 0);
    assert_eq!(
        inventory
            .list_items(ListItemsRequest {
                filter: Default::default(),
                pagination: Default::default()
            })
            .unwrap()
            .page
            .entries
            .len(),
        1
    );
}
#[test]
fn remote_removal_keeps_presence_unknown_and_scope_warnings_do_not_invent_items() {
    let (_tree, inventory, identity) = setup();
    let mut file = remote_file("present", "remote", m::MetadataValue::Known("file".into()));
    let id = ingest(&inventory, &file, &identity).unwrap().item.unwrap();
    file.id = m::FileId("removed".into());
    file.data = None;
    file.event.current = None;
    file.event.remote.as_mut().unwrap().removed = Some(true);
    ingest(&inventory, &file, &identity).unwrap();
    let observed = item(&inventory, id);
    assert_eq!(observed.remote_presence, PresenceState::Unknown);
    assert!(observed.remote.is_some());
    assert!(observed.baseline.is_none());
    let warning = File {
        id: FileId("warning".into()),
        sync_id: identity.sync_id.clone(),
        observed_at: Timestamp { unix_millis: 1 },
        scope: FileScope::Remote(RemoteFileRef {
            account_id: identity.account_id.clone(),
            file_id: RemoteFileId(identity.remote_root_id.clone()),
        }),
        coverage: ObservationCoverage::Unavailable,
        data: None,
        event: FileEvent {
            kind: FileEventKind::ScopeUnavailable,
            local: None,
            remote: None,
            uncertainty: None,
            previous: None,
            current: None,
        },
    };
    let result = ingest(&inventory, &warning, &identity).unwrap();
    assert!(result.item.is_none());
    assert!(result.analyze_all);
    assert!(!result.scan_local);
    assert_eq!(
        inventory
            .list_items(ListItemsRequest {
                filter: Default::default(),
                pagination: Default::default()
            })
            .unwrap()
            .page
            .entries
            .len(),
        1
    );
}
#[test]
fn wrong_scope_and_unknown_remote_resource_are_retained_as_errors_not_fake_observations() {
    let (_tree, inventory, identity) = setup();
    let mut file = remote_file(
        "remote-event",
        "remote",
        m::MetadataValue::Known("file".into()),
    );
    file.sync_id = SyncId("another-sync".into());
    assert!(matches!(
        ingest(&inventory, &file, &identity),
        Err(PreparationFailure::InvalidObservation(_))
    ));
    file.sync_id = identity.sync_id.clone();
    file.data = None;
    file.event.current = None;
    assert!(matches!(
        ingest(&inventory, &file, &identity),
        Err(PreparationFailure::InvalidObservation(_))
    ));
    assert!(inventory
        .list_items(ListItemsRequest {
            filter: Default::default(),
            pagination: Default::default()
        })
        .unwrap()
        .page
        .entries
        .is_empty());
}
