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
    static NEXT_ORDER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
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
        causal_order: Some(NEXT_ORDER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)),
        sync_id: SyncId("sync".into()),
        observed_at: m::Timestamp { unix_millis: 10 },
        scope: m::FileScope::Remote(reference.clone()),
        coverage: m::ObservationCoverage::Partial,
        event: m::FileEvent {
            action: m::FileEventAction::StateChanged,
            local: None,
            local_presence: None,
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
#[test]
fn incomparable_receipt_replay_reissues_scoped_revalidation_after_interruption() {
    let (_tree, inventory, identity) = setup();
    let mut first = remote_file(
        "ordered",
        "remote",
        m::MetadataValue::Known("current".into()),
    );
    if let Some(m::FileData::Remote(remote)) = &mut first.data {
        remote.version = m::MetadataValue::Unavailable;
    }
    let current = ingest(&inventory, &first, &identity).unwrap();
    let current_id = current.item.unwrap();
    let revision = item(&inventory, current_id.clone()).revision;

    let mut unknown = remote_file(
        "unknown-order",
        "remote",
        m::MetadataValue::Known("uncertain".into()),
    );
    unknown.causal_order = None;
    if let Some(m::FileData::Remote(remote)) = &mut unknown.data {
        remote.version = m::MetadataValue::Unavailable;
    }
    let result = ingest(&inventory, &unknown, &identity).unwrap();
    assert_eq!(result.item, None);
    assert!(!result.analyze_all);
    assert_eq!(
        result.revalidate,
        Some(m::FileScope::Remote(m::RemoteFileRef {
            account_id: identity.account_id.clone(),
            file_id: m::RemoteFileId("remote".into()),
        }))
    );
    assert_eq!(item(&inventory, current_id.clone()).revision, revision);

    let replay = ingest(&inventory, &unknown, &identity).unwrap();
    assert_eq!(replay.revalidate, result.revalidate);
    assert!(!replay.analyze_all);
}

#[test]
fn incomparable_revalidation_outbox_survives_reopen_until_an_observation_applies() {
    let (tree, inventory, identity) = setup();
    let mut first = remote_file(
        "outbox-initial",
        "remote",
        m::MetadataValue::Known("current".into()),
    );
    if let Some(m::FileData::Remote(remote)) = &mut first.data {
        remote.version = m::MetadataValue::Unavailable;
    }
    let item_id = ingest(&inventory, &first, &identity).unwrap().item.unwrap();
    let mut uncertain = remote_file(
        "outbox-uncertain",
        "remote",
        m::MetadataValue::Known("unknown".into()),
    );
    uncertain.causal_order = None;
    if let Some(m::FileData::Remote(remote)) = &mut uncertain.data {
        remote.version = m::MetadataValue::Unavailable;
    }
    assert_eq!(
        ingest(&inventory, &uncertain, &identity)
            .unwrap()
            .revalidate,
        Some(uncertain.scope.clone())
    );
    assert_eq!(inventory.pending_revalidations().unwrap().len(), 1);
    drop(inventory);

    let reopened = InventoryManager::open(tree.request()).unwrap().inventory;
    let pending = reopened.pending_revalidations().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].0, item_id);
    assert!(pending[0].1);
    assert_eq!(pending[0].2, uncertain.scope);
    assert_eq!(pending[0].3, uncertain.id.0);

    let mut verified = remote_file(
        "outbox-revalidated",
        "remote",
        m::MetadataValue::Known("verified".into()),
    );
    verified.causal_order = Some(i64::MAX as u64);
    if let Some(m::FileData::Remote(remote)) = &mut verified.data {
        remote.version = m::MetadataValue::Unavailable;
    }
    let current = item(&reopened, item_id.clone());
    assert_eq!(
        reopened
            .record_remote(RecordRemoteObservation {
                selector: ObservationTarget::Existing(item_id),
                expected_revision: Some(current.revision),
                observation: crate::inventory::RemoteObservation {
                    observed: verified,
                    presence: PresenceState::Present,
                    proof: ObservationProof {
                        source: ObservationSource::RemoteChange,
                        observed_at: Timestamp { unix_millis: 5 },
                        scan_or_generation: None,
                        complete_scope: false,
                    },
                },
            })
            .unwrap()
            .disposition,
        ObservationDisposition::Applied
    );
    assert!(reopened.pending_revalidations().unwrap().is_empty());
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
            observations: Vec::new(),
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
    assert!(replay.analyze_all);
    assert_eq!(
        item(&inventory, id.clone()).revision,
        first.resulting_item_revision
    );
    let second = analyzer
        .analyze(AnalyzeRequest {
            observations: Vec::new(),
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
fn remote_scope_removal_keeps_presence_unknown_without_inventing_items() {
    let (_tree, inventory, identity) = setup();
    let mut file = remote_file("present", "remote", m::MetadataValue::Known("file".into()));
    let id = ingest(&inventory, &file, &identity).unwrap().item.unwrap();
    file.id = m::FileId("removed".into());
    file.causal_order = file.causal_order.map(|order| order + 1);
    file.data = None;
    file.event.current = None;
    file.event.action = m::FileEventAction::RemovedFromScope;
    file.event.remote.as_mut().unwrap().removed = Some(true);
    ingest(&inventory, &file, &identity).unwrap();
    let observed = item(&inventory, id);
    assert_eq!(observed.remote_presence, PresenceState::Unknown);
    assert!(observed.remote.is_some());
    assert!(observed.baseline.is_none());
    let warning = File {
        id: FileId("warning".into()),
        causal_order: None,
        sync_id: identity.sync_id.clone(),
        observed_at: Timestamp { unix_millis: 1 },
        scope: FileScope::Remote(RemoteFileRef {
            account_id: identity.account_id.clone(),
            file_id: RemoteFileId(identity.remote_root_id.clone()),
        }),
        coverage: ObservationCoverage::Unavailable,
        data: None,
        event: FileEvent {
            action: FileEventAction::ScopeUnavailable,
            local: None,
            local_presence: None,
            remote: None,
            uncertainty: None,
            previous: None,
            current: None,
        },
    };
    let result = ingest(&inventory, &warning, &identity).unwrap();
    assert!(result.item.is_none());
    assert!(result.analyze_all);
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
