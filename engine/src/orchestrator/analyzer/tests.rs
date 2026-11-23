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
                birth_ns: None,
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
            local_epoch: None,
            remote_epoch: None,
            remote_generation: None,
            events: Vec::<ObservationEvent>::new(),
            latest_origin: ObservationOrigin::Local,
            local_coverage: ObservationCoverage::Complete,
            remote_coverage: ObservationCoverage::Complete,
            local_generation: Some(2),
            remote_page_token: None,
            observed_at: timestamp(4),
        },
        environment: Environment {
            local_replacement: Evidence::NotCollected,
            projection_issue: None,
            trash_propagation_authorized: false,
            trash_tree_safe: Evidence::NotCollected,
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
            birth_ns: None,
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

fn initial_remote(directory: bool, registered: bool) -> File {
    let mut f = fixture();
    f.baseline = Baseline {
        state: BaselineState::NeverSynced,
        local: None,
        remote: None,
        confirmed_at: None,
        projection: None,
    };
    f.local.state = FileState::Absent;
    f.local.identity = Evidence::NotCollected;
    f.identity.local_identity = Evidence::NotCollected;
    f.local.checksum = Evidence::NotCollected;
    f.remote.checksum = checksum(&"a".repeat(64));
    f.environment.destination_local_parent = f.local.parent.clone();
    f.environment.destination_remote_parent_id = Evidence::Known(RemoteId("parent-a".into()));
    f.remote.parents = Evidence::Known(vec![RemoteId("parent-a".into())]);
    f.environment.destination_local_occupied = Evidence::Known(false);
    f.environment.destination_remote_matches = Evidence::Known(vec![RemoteId("remote-a".into())]);
    if directory {
        f.remote.mime_type = Evidence::Known("application/vnd.google-apps.folder".into());
    }
    if !registered {
        f.identity.item_id = None;
        f.inventory_lookup = InventoryLookupStatus::NotFound;
        f.inventory_revision = None;
    }
    f
}

#[test]
fn initial_remote_plans_register_before_materializing_and_resume_registered_absence() {
    for directory in [false, true] {
        for registered in [false, true] {
            let f = initial_remote(directory, registered);
            let d = analyze(&f);
            assert_eq!(d.status, DecisionStatus::Ready, "{}", d.reason);
            assert_eq!(d.operations.len(), if registered { 1 } else { 2 });
            if !registered {
                assert_eq!(d.operations[0].action, OperationAction::RegisterIdentity);
            }
            let op = d.operations.last().unwrap();
            assert_eq!(
                op.action,
                if directory {
                    OperationAction::CreateLocalDirectory
                } else {
                    OperationAction::Download
                }
            );
            assert_eq!(op.depends_on, if registered { vec![] } else { vec![1] });
            assert_eq!(
                op.preconditions.as_ref().unwrap().destination_free,
                Evidence::Known(true)
            );
        }
    }
}

#[test]
fn initial_remote_planning_rejects_incomplete_parent_collision_and_binary_evidence() {
    for case in 0..9 {
        let mut f = initial_remote(false, false);
        match case {
            0 => f.environment.destination_local_parent = Evidence::NotCollected,
            1 => f.environment.destination_local_occupied = Evidence::Known(true),
            2 => f.observations.local_coverage = ObservationCoverage::Partial,
            3 => f.remote.checksum = Evidence::NotCollected,
            4 => f.remote.checksum = checksum("malformed"),
            5 => f.remote.size = Evidence::Unavailable,
            6 => f.remote.capabilities = Evidence::NotCollected,
            7 => {
                f.remote.mime_type = Evidence::Known("application/vnd.google-apps.document".into())
            }
            _ => {
                f.environment.destination_remote_matches = Evidence::Known(vec![
                    RemoteId("remote-a".into()),
                    RemoteId("duplicate".into()),
                ])
            }
        }
        assert!(
            analyze(&f).operations.iter().all(|op| !matches!(
                op.action,
                OperationAction::Download | OperationAction::CreateLocalDirectory
            )),
            "case {case}"
        );
    }
}

fn initial_local(directory: bool, registered: bool) -> File {
    let mut file = fixture();
    file.baseline = Baseline {
        state: BaselineState::NeverSynced,
        local: None,
        remote: None,
        confirmed_at: None,
        projection: None,
    };
    file.identity.remote_id = None;
    file.remote.id = None;
    file.remote.state = FileState::Absent;
    file.remote.version = Evidence::NotCollected;
    file.local.checksum = checksum(&"b".repeat(64));
    file.environment.destination_remote_parent_id = Evidence::Known(RemoteId("parent-a".into()));
    file.remote.parents = Evidence::Known(vec![RemoteId("parent-a".into())]);
    file.environment.destination_local_parent = file.local.parent.clone();
    file.environment.destination_remote_matches = Evidence::Known(Vec::new());
    if directory {
        file.local.kind = Evidence::Known(FileKind::Directory);
    }
    if !registered {
        file.identity.item_id = None;
        file.inventory_revision = None;
        file.inventory_lookup = InventoryLookupStatus::NotFound;
    }
    file
}
#[test]
fn initial_upload_plans_register_before_create_and_resume_registered_items() {
    for directory in [false, true] {
        for registered in [false, true] {
            let d = analyze(&initial_local(directory, registered));
            assert_eq!(d.status, DecisionStatus::Ready, "{}", d.reason);
            assert_eq!(d.operations.len(), if registered { 1 } else { 2 });
            let last = d.operations.last().unwrap();
            assert_eq!(
                last.action,
                if directory {
                    OperationAction::CreateRemoteDirectory
                } else {
                    OperationAction::Upload
                }
            );
            assert_eq!(last.depends_on, if registered { vec![] } else { vec![1] });
            assert_eq!(
                last.preconditions.as_ref().unwrap().destination_free,
                Evidence::Known(true)
            );
        }
    }
}
#[test]
fn initial_upload_requires_complete_absence_and_stable_readable_binary_evidence() {
    for case in 0..7 {
        let mut f = initial_local(false, false);
        match case {
            0 => {
                f.environment.destination_remote_matches =
                    Evidence::Known(vec![RemoteId("collision".into())])
            }
            1 => f.observations.remote_coverage = ObservationCoverage::Partial,
            2 => f.environment.destination_remote_parent_id = Evidence::NotCollected,
            3 => f.local.checksum = checksum("bad"),
            4 => f.local.size = Evidence::Unavailable,
            5 => f.local.capabilities = Evidence::NotCollected,
            _ => f.remote.state = FileState::Unknown,
        }
        assert!(
            analyze(&f).operations.iter().all(|op| !matches!(
                op.action,
                OperationAction::Upload | OperationAction::CreateRemoteDirectory
            )),
            "case {case}"
        );
    }
}

#[test]
fn replacement_requires_bound_slot_evidence_and_confirms_identical_bytes_without_transfer() {
    let mut f = fixture();
    f.local.identity = Evidence::Known(LocalIdentity {
        device_id: 1,
        inode: 99,
        mount_id: Some(3),
        birth_ns: None,
    });
    assert_eq!(analyze(&f).status, DecisionStatus::Conflict);
    f.environment.local_replacement = Evidence::Known(true);
    let unchanged = analyze(&f);
    assert_eq!(
        unchanged.operations[0].action,
        OperationAction::ConfirmBaseline
    );
    f.local.checksum = checksum("replacement-bytes");
    let edited = analyze(&f);
    assert_eq!(
        edited.operations[0].action,
        OperationAction::UpdateRemoteContent
    );
}

#[test]
fn old_remote_captures_cannot_reverse_a_confirmed_local_edit() {
    let mut f = fixture();
    f.baseline.remote.as_mut().unwrap().version = Evidence::Known(10);
    f.remote.version = Evidence::Known(9);
    f.remote.checksum = checksum("previous-content");
    let d = analyze(&f);
    assert_eq!(d.status, DecisionStatus::Defer);
    assert!(d.operations.is_empty());
    assert!(d
        .validation
        .checks
        .iter()
        .any(|c| c.code == "observation.remote_regression"));
}

#[test]
fn partial_location_receipts_resume_remaining_edits_and_refresh_descendant_paths() {
    let mut f = fixture();
    f.local.name = Evidence::Known("renamed.txt".into());
    f.local.path = Some("folder/renamed.txt".into());
    f.remote.name = f.local.name.clone();
    f.remote.version = Evidence::Known(11);
    f.local.checksum = checksum("edited");
    let decision = analyze(&f);
    assert_eq!(decision.status, DecisionStatus::Ready);
    assert_eq!(decision.operations.len(), 1);
    assert_eq!(
        decision.operations[0].action,
        OperationAction::UpdateRemoteContent
    );
    f.local.checksum = f.remote.checksum.clone();
    f.local.parent = Evidence::Known("moved-parent".into());
    f.local.path = Some("moved-parent/renamed.txt".into());
    f.environment.destination_local_parent = f.local.parent.clone();
    f.environment.destination_remote_parent_id = Evidence::Known(RemoteId("parent-a".into()));
    f.remote.parents = Evidence::Known(vec![RemoteId("parent-a".into())]);
    let decision = analyze(&f);
    assert_eq!(decision.status, DecisionStatus::Ready);
    assert_eq!(
        decision.operations[0].action,
        OperationAction::ConfirmBaseline
    );
}

#[test]
fn combined_tree_location_and_content_plans_resume_each_verified_step_in_both_directions() {
    for from_local in [true, false] {
        let mut f = fixture();
        f.environment.destination_remote_parent_id = Evidence::Known(RemoteId("new-parent".into()));
        f.environment.destination_local_parent = Evidence::Known("new-parent".into());
        f.environment.destination_remote_matches = Evidence::Known(vec![]);
        f.environment.destination_local_occupied = Evidence::Known(false);
        if from_local {
            f.local.name = Evidence::Known("new.txt".into());
            f.local.path = Some("new-parent/new.txt".into());
            f.local.parent = Evidence::Known("new-parent".into());
            f.local.checksum = checksum("edited");
        } else {
            f.remote.name = Evidence::Known("new.txt".into());
            f.remote.parents = Evidence::Known(vec![RemoteId("new-parent".into())]);
            f.remote.checksum = checksum("edited");
        }
        let expected = if from_local {
            vec![
                OperationAction::RenameRemote,
                OperationAction::MoveRemote,
                OperationAction::UpdateRemoteContent,
            ]
        } else {
            vec![
                OperationAction::RenameLocal,
                OperationAction::MoveLocal,
                OperationAction::UpdateLocalContent,
            ]
        };
        let decision = analyze(&f);
        assert_eq!(decision.status, DecisionStatus::Ready);
        assert_eq!(
            decision
                .operations
                .iter()
                .map(|op| op.action)
                .collect::<Vec<_>>(),
            expected
        );
        if from_local {
            f.remote.name = f.local.name.clone();
        } else {
            f.local.name = f.remote.name.clone();
            f.local.path = Some("folder/new.txt".into());
        }
        let decision = analyze(&f);
        assert_eq!(decision.status, DecisionStatus::Ready);
        assert_eq!(
            decision
                .operations
                .iter()
                .map(|op| op.action)
                .collect::<Vec<_>>(),
            expected[1..]
        );
        if from_local {
            f.remote.parents = Evidence::Known(vec![RemoteId("new-parent".into())]);
        } else {
            f.local.parent = Evidence::Known("new-parent".into());
            f.local.path = Some("new-parent/new.txt".into());
        }
        let decision = analyze(&f);
        assert_eq!(decision.status, DecisionStatus::Ready);
        assert_eq!(
            decision
                .operations
                .iter()
                .map(|op| op.action)
                .collect::<Vec<_>>(),
            expected[2..]
        );
    }
}

fn content_conflict() -> File {
    let mut f = fixture();
    f.local.checksum = checksum("local");
    f.remote.checksum = checksum("remote");
    f.remote.version = Evidence::Known(11);
    f.environment.destination_local_parent = f.local.parent.clone();
    if let Evidence::Known(ids) = &f.remote.parents {
        f.environment.destination_remote_parent_id = Evidence::Known(ids[0].clone());
    }
    f
}
fn stored_conflict(
    f: &File,
    choice: Option<crate::conflicts::ConflictChoice>,
) -> crate::inventory::models::ConflictState {
    crate::inventory::models::ConflictState {
        id: crate::models::Id("conflict-test".into()),
        item_id: f.identity.item_id.clone().unwrap(),
        reason: "Content".into(),
        resolved: false,
        local: f.local.clone(),
        remote: f.remote.clone(),
        observed_revision: f.inventory_revision.unwrap(),
        updated_at: timestamp(9),
        resolution: choice.map(|choice| {
            Box::new(crate::inventory::models::ConflictResolution {
                choice,
                expected_local: f.local.clone(),
                expected_remote: f.remote.clone(),
            })
        }),
    }
}
#[test]
fn conflicts_require_proven_incompatible_attributes_and_not_just_missing_evidence() {
    let f = content_conflict();
    assert_eq!(analyze(&f).status, DecisionStatus::Conflict);
    let mut unknown = f.clone();
    unknown.remote.checksum = Evidence::Unavailable;
    assert_eq!(analyze(&unknown).status, DecisionStatus::Defer);
    let mut independent = fixture();
    independent.local.name = Evidence::Known("new.txt".into());
    independent.remote.checksum = checksum("remote");
    assert_eq!(analyze(&independent).status, DecisionStatus::Ready);
    let mut mime = fixture();
    mime.remote.mime_type = Evidence::Known("application/pdf".into());
    assert_eq!(analyze(&mime).status, DecisionStatus::NoOp);
    let mut native = fixture();
    native.remote.mime_type = Evidence::Known("application/vnd.google-apps.document".into());
    native.remote.checksum = Evidence::Unsupported;
    assert_eq!(analyze(&native).status, DecisionStatus::Conflict);
    let mut trash = f.clone();
    trash.remote.trashed = Evidence::Known(true);
    assert_eq!(analyze(&trash).status, DecisionStatus::Conflict);
    let mut kind = fixture();
    kind.remote.mime_type = Evidence::Known("application/vnd.google-apps.folder".into());
    kind.remote.checksum = Evidence::NotApplicable;
    assert_eq!(analyze(&kind).status, DecisionStatus::Conflict);
    let mut moved = fixture();
    moved.local.parent = Evidence::Known("local-destination".into());
    moved.remote.parents = Evidence::Known(vec![RemoteId("remote-destination".into())]);
    assert_eq!(analyze(&moved).status, DecisionStatus::Conflict);
}
#[test]
fn explicit_choices_pin_evidence_and_resolved_boolean_never_authorizes_overwrite() {
    use crate::conflicts::ConflictChoice;
    for (choice, action) in [
        (
            ConflictChoice::KeepLocal,
            OperationAction::UpdateRemoteContent,
        ),
        (
            ConflictChoice::KeepRemote,
            OperationAction::UpdateLocalContent,
        ),
    ] {
        let mut f = content_conflict();
        f.prior_conflict = Some(stored_conflict(&f, Some(choice)));
        let d = analyze(&f);
        assert_eq!(d.status, DecisionStatus::Ready);
        assert_eq!(d.operations[0].action, action);
        assert_eq!(
            d.operations[0].conflict_id,
            f.prior_conflict.as_ref().map(|c| c.id.clone())
        );
        f.remote.version = Evidence::Known(12);
        assert_eq!(analyze(&f).status, DecisionStatus::Conflict);
        f.prior_conflict.as_mut().unwrap().resolved = true;
        assert_eq!(analyze(&f).status, DecisionStatus::Conflict);
    }
    let mut f = content_conflict();
    f.prior_conflict = Some(stored_conflict(&f, None));
    f.remote.checksum = f.local.checksum.clone();
    let d = analyze(&f);
    assert_eq!(d.status, DecisionStatus::Ready);
    assert_eq!(d.operations[0].action, OperationAction::ConfirmBaseline);
}

#[test]
fn initial_collision_is_conflict_and_requires_explicit_choice() {
    let mut f = content_conflict();
    f.baseline = Baseline::never_synced();
    f.identity.item_id = None;
    f.inventory_revision = None;
    f.inventory_lookup = InventoryLookupStatus::NotFound;
    assert_eq!(analyze(&f).status, DecisionStatus::Conflict);
    f.remote.checksum = Evidence::Unavailable;
    assert_eq!(analyze(&f).status, DecisionStatus::Defer);
}

#[test]
fn pending_conflict_effect_is_recovered_before_any_new_conflict_decision() {
    use crate::inventory::models::{OperationState, PendingOperation};
    let mut f = content_conflict();
    f.prior_conflict = Some(stored_conflict(&f, None));
    f.pending_operations.push(PendingOperation {
        id: crate::models::Id("lost".into()),
        item_id: f.identity.item_id.clone().unwrap(),
        action: OperationAction::UpdateLocalContent,
        state: OperationState::AppliedUnconfirmed,
        remote_id: f.remote.id.clone(),
        applied_effect: None,
        idempotency_key: None,
        recovery: None,
        recovery_diagnostic: None,
        created_at: timestamp(9),
        updated_at: timestamp(9),
    });
    let d = analyze(&f);
    assert_eq!(d.status, DecisionStatus::Defer);
    assert!(d
        .validation
        .checks
        .iter()
        .any(|c| c.code == "operation.pending"));
}

#[test]
fn folder_move_conflicts_choose_location_without_planning_binary_transfers() {
    for choice in [
        crate::conflicts::ConflictChoice::KeepLocal,
        crate::conflicts::ConflictChoice::KeepRemote,
    ] {
        let mut f = fixture();
        f.local.kind = Evidence::Known(crate::models::FileKind::Directory);
        f.local.size = Evidence::NotApplicable;
        f.local.checksum = Evidence::NotApplicable;
        f.remote.mime_type = Evidence::Known("application/vnd.google-apps.folder".into());
        f.remote.size = Evidence::NotCollected;
        f.remote.checksum = Evidence::NotCollected;
        f.baseline.local = Some(f.local.clone());
        f.baseline.remote = Some(f.remote.clone());
        f.local.parent = Evidence::Known("local-new".into());
        f.remote.parents = Evidence::Known(vec![RemoteId("remote-new".into())]);
        f.environment.destination_remote_parent_id =
            Evidence::Known(RemoteId("local-new-id".into()));
        f.environment.destination_local_parent = Evidence::Known("remote-new-path".into());
        f.environment.destination_local_occupied = Evidence::Known(false);
        f.environment.destination_remote_matches = Evidence::Known(vec![]);
        assert_eq!(analyze(&f).status, DecisionStatus::Conflict);
        f.prior_conflict = Some(stored_conflict(&f, Some(choice)));
        let d = analyze(&f);
        assert_eq!(d.status, DecisionStatus::Ready);
        assert_eq!(d.operations.len(), 1);
        assert_eq!(
            d.operations[0].action,
            if choice == crate::conflicts::ConflictChoice::KeepLocal {
                OperationAction::MoveRemote
            } else {
                OperationAction::MoveLocal
            }
        );
    }
}

fn workspace_fixture() -> File {
    let mut f = fixture();
    f.remote.mime_type = Evidence::Known("application/vnd.google-apps.document".into());
    f.remote.name = Evidence::Known("Document".into());
    f.remote.parents = Evidence::Known(vec![RemoteId("drive-root".into())]);
    f.remote.size = Evidence::NotApplicable;
    f.remote.checksum = Evidence::NotApplicable;
    f.environment.destination_remote_parent_id = Evidence::Known(RemoteId("drive-root".into()));
    f.environment.destination_remote_matches =
        Evidence::Known(f.remote.id.iter().cloned().collect());
    f.environment.destination_local_parent = Evidence::Known(std::path::PathBuf::new());
    f.environment.destination_local_occupied = Evidence::Known(false);
    f.local.state = FileState::Absent;
    f.local.path = Some("Document.desktop".into());
    f.local.name = Evidence::Known("Document.desktop".into());
    f.local.parent = Evidence::Known(std::path::PathBuf::new());
    f.local.identity = Evidence::NotCollected;
    f.identity.local_identity = Evidence::NotCollected;
    f.identity.item_id = None;
    f.inventory_lookup = InventoryLookupStatus::NotFound;
    f.inventory_revision = None;
    f.baseline = Baseline::never_synced();
    f
}
#[test]
fn workspace_creation_baseline_refresh_and_remote_rename_never_plan_uploads() {
    use sha2::{Digest, Sha256};
    let mut f = workspace_fixture();
    let plan = analyze(&f);
    assert_eq!(plan.status, DecisionStatus::Ready);
    assert_eq!(
        plan.operations.iter().map(|o| o.action).collect::<Vec<_>>(),
        vec![
            OperationAction::RegisterIdentity,
            OperationAction::CreateLocalShortcut
        ]
    );
    let bytes = crate::projections::link_bytes(f.remote.id.as_ref().unwrap());
    f.local.state = FileState::Present;
    f.local.kind = Evidence::Known(crate::models::FileKind::Regular);
    f.local.identity = fixture().local.identity;
    f.identity.local_identity = f.local.identity.clone();
    f.local.size = Evidence::Known(bytes.len() as u64);
    f.local.checksum = checksum(&format!("{:x}", Sha256::digest(&bytes)));
    f.identity.item_id = fixture().identity.item_id;
    f.inventory_lookup = InventoryLookupStatus::Found;
    f.inventory_revision = Some(4);
    f.baseline = Baseline {
        state: BaselineState::Synced,
        local: Some(f.local.clone()),
        remote: Some(f.remote.clone()),
        confirmed_at: Some(timestamp(9)),
        projection: Some(crate::inventory::models::ProjectionKind::GoogleWorkspaceShortcut),
    };
    assert_eq!(analyze(&f).status, DecisionStatus::NoOp);
    f.remote.version = Evidence::Known(11);
    let p = analyze(&f);
    assert_eq!(p.operations[0].action, OperationAction::ConfirmBaseline);
    f.remote.name = Evidence::Known("Renamed".into());
    let p = analyze(&f);
    assert_eq!(p.operations[0].action, OperationAction::RenameLocal);
    assert_eq!(p.operations[0].name, Some("Renamed.desktop".into()));
    f.local.checksum = checksum("edited");
    let p = analyze(&f);
    assert_eq!(p.status, DecisionStatus::Unsupported);
    assert!(p.operations.is_empty());
}

#[test]
fn trash_requires_explicit_policy_complete_scope_unchanged_peer_and_empty_folder() {
    let mut f = fixture();
    f.baseline.projection = Some(crate::inventory::models::ProjectionKind::Native);
    f.environment.destination_local_parent = f.local.parent.clone();
    f.environment.destination_remote_parent_id = Evidence::Known(RemoteId("parent-a".into()));
    f.local.state = FileState::Absent;
    assert_eq!(analyze(&f).status, DecisionStatus::Defer);
    f.environment.trash_propagation_authorized = true;
    assert_eq!(
        analyze(&f).operations[0].action,
        OperationAction::TrashRemote
    );
    f.remote.version = Evidence::Known(9);
    assert_eq!(
        analyze(&f).operations[0].action,
        OperationAction::TrashRemote
    );
    for state in [
        FileState::OutsideScope,
        FileState::Unknown,
        FileState::Unavailable,
    ] {
        let mut denied = f.clone();
        denied.remote.state = state;
        assert!(analyze(&denied).operations.is_empty());
    }
    let mut changed = f.clone();
    changed.remote.checksum = checksum("changed");
    assert!(analyze(&changed).operations.is_empty());
    let mut partial = f.clone();
    partial.observations.local_coverage = ObservationCoverage::Partial;
    assert!(analyze(&partial).operations.is_empty());
    f.local.kind = Evidence::Known(FileKind::Directory);
    f.baseline.local.as_mut().unwrap().kind = f.local.kind.clone();
    assert!(analyze(&f).operations.is_empty());
    f.environment.trash_tree_safe = Evidence::Known(false);
    assert!(analyze(&f).operations.is_empty());
    f.environment.trash_tree_safe = Evidence::Known(true);
    assert_eq!(
        analyze(&f).operations[0].action,
        OperationAction::TrashRemote
    );
    let mut f = fixture();
    f.baseline.projection = Some(crate::inventory::models::ProjectionKind::Native);
    f.environment.trash_propagation_authorized = true;
    f.environment.destination_local_parent = f.local.parent.clone();
    f.environment.destination_remote_parent_id = Evidence::Known(RemoteId("parent-a".into()));
    f.remote.state = FileState::Absent;
    assert!(analyze(&f).operations.is_empty());
    f.remote.trashed = Evidence::Known(true);
    f.remote.version = Evidence::Known(9);
    assert_eq!(
        analyze(&f).operations[0].action,
        OperationAction::RemoveLocal
    );
    f.local.checksum = checksum("edited child");
    assert!(analyze(&f).operations.is_empty());
}
