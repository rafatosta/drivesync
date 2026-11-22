use super::*;
use crate::{
    detectors::{
        context::{Environment, File, Observations, PathRoot},
        observation::ObservationCoverage,
    },
    inventory::models::{Baseline, InventoryLookupStatus, LocalState, RemoteState},
    models::{
        AccountId, Checksum, DateTime, Evidence, FileIdentity, FileKind, FileState,
        LocalCapabilities, LocalIdentity, ObservationEvent, ObservationOrigin, RemoteCapabilities,
        RemoteId, SyncRootId,
    },
};
use std::path::PathBuf;

pub(in crate::orchestrator) fn fixture() -> File {
    let local = local_state("same", 10);
    let remote = remote_state("same", 10);
    File {
        registration_id: Some(crate::models::ItemId("proposed".into())),
        association_candidates: Vec::new(),
        identity: FileIdentity {
            account_id: AccountId("account-a".into()),
            sync_root_id: SyncRootId("root-a".into()),
            item_id: Some(crate::models::ItemId("item-a".into())),
            remote_id: Some(RemoteId("remote-a".into())),
            local_identity: Evidence::Known(LocalIdentity {
                device_id: 1,
                inode: 2,
                mount_id: Some(3),
            }),
        },
        local: local.clone(),
        remote: remote.clone(),
        baseline: Baseline {
            state: BaselineState::Synced,
            local: Some(local),
            remote: Some(remote),
            confirmed_at: Some(timestamp(1)),
            projection: None,
        },
        observations: Observations {
            events: Vec::<ObservationEvent>::new(),
            latest_origin: ObservationOrigin::Local,
            local_coverage: ObservationCoverage::Complete,
            remote_coverage: ObservationCoverage::Complete,
            local_generation: Some(2),
            remote_page_token: None,
            observed_at: timestamp(4),
        },
        environment: Environment {
            local_root: PathRoot(PathBuf::from("/sync")),
            remote_root_id: RemoteId("drive-root".into()),
            local_root_available: true,
            remote_available: true,
            destination_local_parent: Evidence::NotCollected,
            destination_remote_parent_id: Evidence::NotCollected,
            destination_remote_matches: Evidence::NotCollected,
            destination_local_occupied: Evidence::NotCollected,
        },
        inventory_lookup: InventoryLookupStatus::Found,
        pending_operations: Vec::new(),
        prior_conflict: None,
        initial_association_authorized: false,
        inventory_revision: Some(4),
    }
}

fn timestamp(unix_millis: i64) -> DateTime {
    DateTime { unix_millis }
}

fn checksum(value: &str) -> Evidence<Checksum> {
    Evidence::Known(Checksum {
        algorithm: "sha256".into(),
        value: value.into(),
    })
}

fn local_state(digest: &str, size: u64) -> LocalState {
    LocalState {
        state: FileState::Present,
        path: Some(PathBuf::from("folder/file.txt")),
        name: Evidence::Known("file.txt".into()),
        parent: Evidence::Known(PathBuf::from("folder")),
        kind: Evidence::Known(FileKind::Regular),
        identity: Evidence::Known(LocalIdentity {
            device_id: 1,
            inode: 2,
            mount_id: Some(3),
        }),
        size: Evidence::Known(size),
        modified_at: Evidence::Known(timestamp(2)),
        changed_at: Evidence::NotCollected,
        checksum: checksum(digest),
        capabilities: Evidence::Known(LocalCapabilities {
            can_read: Evidence::Known(true),
            can_write: Evidence::Known(true),
            can_rename: Evidence::Known(true),
            can_delete: Evidence::Known(true),
        }),
    }
}

fn remote_state(digest: &str, size: u64) -> RemoteState {
    RemoteState {
        state: FileState::Present,
        id: Some(RemoteId("remote-a".into())),
        name: Evidence::Known("file.txt".into()),
        parents: Evidence::Known(vec![RemoteId("parent-a".into())]),
        mime_type: Evidence::Known("text/plain".into()),
        size: Evidence::Known(size),
        checksum: checksum(digest),
        version: Evidence::Known(8),
        modified_time: Evidence::Known(timestamp(2)),
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

#[test]
fn unchanged_content_and_timestamp_only_differences_do_not_create_work() {
    let mut file = fixture();
    file.local.modified_at = Evidence::Known(timestamp(99));
    file.remote.modified_time = Evidence::Known(timestamp(99));

    let decision = analyze(&file);

    assert_eq!(decision.status, DecisionStatus::NoOp);
    assert_eq!(decision.changeset.status, ComparisonStatus::Comparable);
    assert!(decision.operations.is_empty());
}

#[test]
fn unilateral_local_content_change_plans_a_guarded_remote_update() {
    let mut file = fixture();
    file.local.checksum = checksum("local-change");

    let decision = analyze(&file);

    assert_eq!(decision.status, DecisionStatus::Ready);
    assert_eq!(decision.operations.len(), 1);
    assert_eq!(
        decision.operations[0].action,
        OperationAction::UpdateRemoteContent
    );
    assert_eq!(decision.operations[0].step, 1);
    assert_eq!(
        decision.operations[0].remote_id,
        Some(RemoteId("remote-a".into()))
    );
    assert_eq!(decision.execution.status, ExecutionStatus::Executable);
    assert_eq!(
        decision.operations[0]
            .preconditions
            .as_ref()
            .unwrap()
            .observed_remote_version,
        Some(8)
    );
}

#[test]
fn remote_content_update_is_deferred_without_a_conditional_version() {
    let mut file = fixture();
    file.local.checksum = checksum("local-change");
    file.remote.version = Evidence::NotCollected;

    let decision = analyze(&file);

    assert_eq!(decision.status, DecisionStatus::Defer);
    assert_eq!(decision.execution.status, ExecutionStatus::Blocked);
    assert!(decision.operations.is_empty());
}

#[test]
fn incompatible_concurrent_content_changes_are_conflicts_without_operations() {
    let mut file = fixture();
    file.local.checksum = checksum("local-change");
    file.remote.checksum = checksum("remote-change");

    let decision = analyze(&file);

    assert_eq!(decision.status, DecisionStatus::Conflict);
    assert!(decision.operations.is_empty());
}

#[test]
fn initial_association_requires_matching_metadata_stable_identity_and_authorization() {
    let mut file = fixture();
    file.inventory_lookup = InventoryLookupStatus::NotFound;
    file.inventory_revision = None;
    file.identity.item_id = None;
    file.baseline = Baseline::never_synced();
    file.local.path = Some(PathBuf::from("file.txt"));
    file.local.parent = Evidence::Known(PathBuf::new());
    file.remote.parents = Evidence::Known(vec![RemoteId("drive-root".into())]);
    file.environment.destination_local_parent = Evidence::Known(PathBuf::new());
    file.environment.destination_remote_parent_id = Evidence::Known(RemoteId("drive-root".into()));
    file.initial_association_authorized = true;

    assert_eq!(analyze(&file).status, DecisionStatus::Adopt);

    file.initial_association_authorized = false;
    let decision = analyze(&file);
    assert_eq!(decision.status, DecisionStatus::Adopt);
    assert!(decision.operations.is_empty());
}

#[test]
fn inconclusive_inventory_or_incomplete_absence_never_produces_operations() {
    let mut file = fixture();
    file.inventory_lookup = InventoryLookupStatus::Unavailable;
    file.baseline.state = BaselineState::Unavailable;
    let unavailable = analyze(&file);
    assert_eq!(unavailable.status, DecisionStatus::Defer);
    assert!(unavailable.operations.is_empty());

    let mut file = fixture();
    file.local.state = FileState::Absent;
    file.observations.local_coverage = ObservationCoverage::Partial;
    let partial_absence = analyze(&file);
    assert_eq!(partial_absence.status, DecisionStatus::Defer);
    assert!(partial_absence.operations.is_empty());
}

fn initial_pair(directory: bool) -> File {
    let mut file = fixture();
    file.identity.item_id = None;
    file.inventory_lookup = InventoryLookupStatus::NotFound;
    file.inventory_revision = None;
    file.baseline = Baseline::never_synced();
    file.local.path = Some(PathBuf::from("file.txt"));
    file.local.parent = Evidence::Known(PathBuf::new());
    file.remote.parents = Evidence::Known(vec![RemoteId("drive-root".into())]);
    file.environment.destination_local_parent = Evidence::Known(PathBuf::new());
    file.environment.destination_remote_parent_id = Evidence::Known(RemoteId("drive-root".into()));
    file.initial_association_authorized = true;
    if directory {
        file.local.kind = Evidence::Known(FileKind::Directory);
        file.remote.mime_type = Evidence::Known("application/vnd.google-apps.folder".into());
        file.local.size = Evidence::NotApplicable;
        file.remote.size = Evidence::NotApplicable;
        file.local.checksum = Evidence::NotApplicable;
        file.remote.checksum = Evidence::NotApplicable;
    }
    file
}
#[test]
fn directories_adopt_without_binary_metadata_but_not_without_authorization_or_parent_evidence() {
    let mut file = initial_pair(true);
    let result = analyze(&file);
    assert_eq!(result.status, DecisionStatus::Adopt);
    assert_eq!(result.operations[0].action, OperationAction::Adopt);
    assert_eq!(
        result.operations[0].item_ref,
        file.registration_id.clone().unwrap()
    );
    assert!(file.identity.item_id.is_none());
    file.initial_association_authorized = false;
    assert!(analyze(&file).operations.is_empty());
    file.initial_association_authorized = true;
    file.environment.destination_local_parent = Evidence::NotCollected;
    assert!(analyze(&file).operations.is_empty());
}
#[test]
fn differing_bytes_or_ambiguous_candidates_do_not_adopt_even_with_authorization() {
    let mut file = initial_pair(false);
    file.remote.checksum = checksum("different");
    assert!(analyze(&file).operations.is_empty());
    file.remote.checksum = file.local.checksum.clone();
    let candidate = crate::detectors::context::AssociationCandidate {
        local: file.local.clone(),
        remote: file.remote.clone(),
        parents_correspond: Evidence::Known(true),
    };
    file.association_candidates = vec![candidate.clone(), candidate];
    assert!(analyze(&file).operations.is_empty());
}
#[test]
fn proven_concurrent_equivalence_confirms_baseline_while_noop_has_no_write_plan() {
    let mut file = initial_pair(false);
    file.identity.item_id = Some(crate::models::ItemId("known".into()));
    file.inventory_lookup = InventoryLookupStatus::Found;
    file.inventory_revision = Some(5);
    file.baseline = fixture().baseline;
    file.local.checksum = checksum("changed-together");
    file.remote.checksum = file.local.checksum.clone();
    let result = analyze(&file);
    assert_eq!(result.status, DecisionStatus::Ready);
    assert_eq!(
        result.operations[0].action,
        OperationAction::ConfirmBaseline
    );
    let mut unchanged = fixture();
    assert_eq!(analyze(&unchanged).status, DecisionStatus::NoOp);
    assert!(analyze(&unchanged).operations.is_empty());
    unchanged.remote.checksum = Evidence::Unavailable;
    assert!(analyze(&unchanged).operations.is_empty());
}
