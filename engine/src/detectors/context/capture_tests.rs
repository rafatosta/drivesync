use super::tests::{context, local_observation};
use super::*;
use crate::{
    detectors::observation::models::{CaptureEntries, ScopeCapture},
    inventory::models::*,
    models::{Id, ItemId, LocalIdentity, RemoteId},
};

fn local(path: &str, inode: u64, directory: bool) -> LocalObservation {
    let mut entry = local_observation();
    let path = Path::from(path);
    entry.path = Some(path.clone());
    entry.parent = Evidence::Known(path.parent().unwrap().to_path_buf());
    entry.name = Evidence::Known(path.file_name().unwrap().to_str().unwrap().into());
    entry.identity = Evidence::Known(LocalIdentity {
        device_id: 1,
        inode,
        mount_id: None,
    });
    entry.kind = Evidence::Known(if directory {
        crate::models::FileKind::Directory
    } else {
        crate::models::FileKind::Regular
    });
    entry
}
fn remote(id: &str, name: &str, parent: &str, directory: bool) -> RemoteObservation {
    RemoteObservation {
        account_id: crate::models::AccountId("account-a".into()),
        provider_id: crate::models::ProviderId("google-drive".into()),
        sync_root_id: SyncRootId("root-a".into()),
        state: FileState::Present,
        remote_id: Some(RemoteId(id.into())),
        name: Evidence::Known(name.into()),
        parents: Evidence::Known(vec![RemoteId(parent.into())]),
        mime_type: Evidence::Known(
            if directory {
                "application/vnd.google-apps.folder"
            } else {
                "text/plain"
            }
            .into(),
        ),
        size: Evidence::Known(7),
        checksum: local_observation().checksum,
        version: Evidence::Known(1),
        modified_time: Evidence::NotCollected,
        trashed: Evidence::Known(false),
        capabilities: Evidence::NotCollected,
        observed_at: crate::models::DateTime { unix_millis: 10 },
        coverage: ObservationCoverage::Complete,
        page_token: None,
        event: None,
    }
}
fn round(
    context: &Context,
    generation: u64,
    coverage: ObservationCoverage,
    entries: CaptureEntries,
) -> ScopeCapture {
    ScopeCapture {
        account_id: context.config.account_id.clone(),
        provider_id: context.config.provider_id.clone(),
        sync_root_id: context.config.sync_root_id.clone(),
        local_root: context.config.local_root.0.clone(),
        remote_root_id: context.config.remote_root_id.clone(),
        generation,
        coverage,
        observed_at: crate::models::DateTime { unix_millis: 10 },
        entries,
    }
}
fn seed(context: &Context, local: LocalObservation, remote: RemoteObservation) {
    let item = ItemId(remote.remote_id.as_ref().unwrap().0.clone());
    let identity = FileIdentity {
        account_id: context.config.account_id.clone(),
        sync_root_id: context.config.sync_root_id.clone(),
        item_id: Some(item.clone()),
        remote_id: remote.remote_id.clone(),
        local_identity: local.identity.clone(),
    };
    let baseline = Baseline {
        state: BaselineState::Synced,
        local: Some(local_state(local)),
        remote: Some(remote_state(remote)),
        confirmed_at: Some(crate::models::DateTime { unix_millis: 1 }),
        projection: Some(ProjectionKind::Native),
    };
    let pending = PendingOperation {
        id: Id(item.0.clone()),
        item_id: item.clone(),
        action: OperationAction::Upload,
        state: OperationState::Pending,
        remote_id: identity.remote_id.clone(),
        applied_effect: None,
        idempotency_key: None,
        created_at: crate::models::DateTime { unix_millis: 1 },
        updated_at: crate::models::DateTime { unix_millis: 1 },
    };
    let mut running = pending.clone();
    running.state = OperationState::Running;
    for (index, effect) in [
        PersistedEffect::Pending(pending),
        PersistedEffect::Pending(running),
        PersistedEffect::OperationApplied(AppliedEffect {
            local: baseline.local.clone(),
            remote: baseline.remote.clone(),
            remote_id: identity.remote_id.clone(),
            verified: true,
        }),
        PersistedEffect::BaselineConfirmed {
            baseline,
            parent: None,
        },
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            context
                .inventory
                .write(InventoryWrite {
                    operation_id: Id(item.0.clone()),
                    item_id: item.clone(),
                    identity: identity.clone(),
                    expected_revision: Some(index as u64),
                    effect,
                    recorded_at: crate::models::DateTime { unix_millis: 1 }
                })
                .status,
            WriteStatus::Written
        );
    }
}
#[test]
fn empty_captures_resolve_unilateral_destinations_without_assigning_identity() {
    let (context, directory) = context();
    assert!(context
        .build_capture(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Local(vec![])
        ))
        .unwrap()
        .is_empty());
    let files = context
        .build_capture(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Remote(vec![remote("r", "file", "drive-root", false)]),
        ))
        .unwrap();
    assert_eq!(files[0].local.state, FileState::Absent);
    assert_eq!(files[0].local.path, Some(Path::from("file")));
    assert_eq!(
        files[0].environment.destination_local_occupied,
        Evidence::Known(false)
    );
    assert!(files[0].identity.item_id.is_none());
    assert_eq!(files[0].baseline.state, BaselineState::NeverSynced);
    let files = context
        .build_capture(round(
            &context,
            2,
            ObservationCoverage::Complete,
            CaptureEntries::Local(vec![local("other", 1, false)]),
        ))
        .unwrap();
    let file = files
        .iter()
        .find(|f| f.local.path == Some(Path::from("other")))
        .unwrap();
    assert_eq!(file.remote.state, FileState::Absent);
    assert_eq!(
        file.environment.destination_remote_matches,
        Evidence::Known(vec![])
    );
    std::fs::remove_dir_all(directory).unwrap();
}
#[test]
fn duplicate_names_are_candidates_and_collisions_never_a_persisted_join() {
    let (context, directory) = context();
    context
        .build_capture(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Local(vec![local("file", 1, false)]),
        ))
        .unwrap();
    let files = context
        .build_capture(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Remote(vec![
                remote("r1", "file", "drive-root", false),
                remote("r2", "file", "drive-root", false),
            ]),
        ))
        .unwrap();
    let file = files.iter().find(|f| f.remote.id.is_none()).unwrap();
    assert_eq!(file.association_candidates.len(), 2);
    assert_eq!(
        file.environment.destination_remote_matches,
        Evidence::Known(vec![RemoteId("r1".into()), RemoteId("r2".into())])
    );
    assert_eq!(file.remote.state, FileState::Unknown);
    assert!(files
        .iter()
        .all(|f| f.identity.item_id.is_none() && !f.initial_association_authorized));
    assert!(context.snapshots().unwrap().is_empty());
    std::fs::remove_dir_all(directory).unwrap();
}
#[test]
fn nested_associated_parents_resolve_both_destinations_at_three_levels() {
    let (context, directory) = context();
    let locals = vec![
        local("a", 1, true),
        local("a/b", 2, true),
        local("a/b/c", 3, true),
        local("a/b/c/file", 4, false),
    ];
    let remotes = vec![
        remote("a", "a", "drive-root", true),
        remote("b", "b", "a", true),
        remote("c", "c", "b", true),
        remote("file", "file", "c", false),
    ];
    for (l, r) in locals.iter().zip(&remotes) {
        seed(&context, l.clone(), r.clone());
    }
    let before = context.snapshots().unwrap();
    context
        .build_capture(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Local(locals),
        ))
        .unwrap();
    let files = context
        .build_capture(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Remote(remotes),
        ))
        .unwrap();
    let file = files
        .iter()
        .find(|f| f.identity.remote_id == Some(RemoteId("file".into())))
        .unwrap();
    assert_eq!(
        file.environment.destination_local_parent,
        Evidence::Known(Path::from("a/b/c"))
    );
    assert_eq!(
        file.environment.destination_remote_parent_id,
        Evidence::Known(RemoteId("c".into()))
    );
    assert_eq!(
        file.environment.destination_local_occupied,
        Evidence::Known(true)
    );
    assert_eq!(context.snapshots().unwrap(), before);
    std::fs::remove_dir_all(directory).unwrap();
}
#[test]
fn reopened_records_are_revisited_and_remote_omission_is_unknown() {
    let (context, directory) = context();
    seed(
        &context,
        local("file", 1, false),
        remote("file", "file", "drive-root", false),
    );
    let before = context.snapshots().unwrap();
    let config = ContextConfig {
        inventory_directory: directory.clone(),
        account_id: context.config.account_id.clone(),
        provider_id: context.config.provider_id.clone(),
        sync_root_id: context.config.sync_root_id.clone(),
        local_root: context.config.local_root.clone(),
        remote_root_id: context.config.remote_root_id.clone(),
        local_root_available: true,
        remote_available: true,
        initial_association_authorized: false,
    };
    drop(context);
    let context = Context::open(config).unwrap();
    context
        .build_capture(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Local(vec![]),
        ))
        .unwrap();
    let files = context
        .build_capture(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Remote(vec![]),
        ))
        .unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].local.state, FileState::Absent);
    assert_eq!(files[0].remote.state, FileState::Unknown);
    assert_eq!(files[0].inventory_revision, Some(4));
    let files = context
        .build_capture(round(
            &context,
            2,
            ObservationCoverage::Complete,
            CaptureEntries::Local(vec![local("other", 2, false)]),
        ))
        .unwrap();
    let other = files
        .iter()
        .find(|file| file.local.path == Some(Path::from("other")))
        .unwrap();
    assert_eq!(other.remote.state, FileState::Unknown);
    assert_eq!(
        other.environment.destination_remote_matches,
        Evidence::NotCollected
    );
    assert_eq!(context.snapshots().unwrap(), before);
    std::fs::remove_dir_all(directory).unwrap();
}
#[test]
fn partial_and_failed_rounds_invalidate_free_destinations_and_absence() {
    let (context, directory) = context();
    seed(
        &context,
        local("known", 1, false),
        remote("known", "known", "drive-root", false),
    );
    context
        .build_capture(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Local(vec![]),
        ))
        .unwrap();
    for (generation, coverage) in [
        (2, ObservationCoverage::Partial),
        (3, ObservationCoverage::Unavailable),
    ] {
        let files = context
            .build_capture(round(
                &context,
                generation,
                coverage,
                CaptureEntries::Local(vec![]),
            ))
            .unwrap();
        assert_ne!(files[0].local.state, FileState::Absent);
        assert_eq!(
            files[0].environment.destination_local_occupied,
            Evidence::NotCollected
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}
#[test]
fn captures_reject_crossed_scopes_duplicates_and_stale_generations() {
    let (context, directory) = context();
    let valid = round(
        &context,
        1,
        ObservationCoverage::Complete,
        CaptureEntries::Local(vec![]),
    );
    for field in 0..5 {
        let mut invalid = valid.clone();
        match field {
            0 => invalid.account_id.0.push('x'),
            1 => invalid.provider_id.0.push('x'),
            2 => invalid.sync_root_id.0.push('x'),
            3 => invalid.local_root.push("other"),
            _ => invalid.remote_root_id.0.push('x'),
        }
        assert_eq!(
            context.build_capture(invalid),
            Err(ContextError::ScopeMismatch)
        );
    }
    context.build_capture(valid.clone()).unwrap();
    assert_eq!(
        context.build_capture(valid),
        Err(ContextError::InvalidCapture)
    );
    let entry = local("file", 1, false);
    assert_eq!(
        context.build_capture(round(
            &context,
            2,
            ObservationCoverage::Complete,
            CaptureEntries::Local(vec![entry.clone(), entry])
        )),
        Err(ContextError::InvalidCapture)
    );
    std::fs::remove_dir_all(directory).unwrap();
}
#[test]
fn moved_inode_or_replaced_path_never_proves_a_logical_items_absence_or_identity() {
    let (context, directory) = context();
    seed(
        &context,
        local("old", 1, false),
        remote("file", "old", "drive-root", false),
    );
    let files = context
        .build_capture(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Local(vec![local("new", 1, false)]),
        ))
        .unwrap();
    assert!(files
        .iter()
        .filter(|f| f.identity.item_id.is_some())
        .all(|f| f.local.state == FileState::Unknown));
    let files = context
        .build_capture(round(
            &context,
            2,
            ObservationCoverage::Complete,
            CaptureEntries::Local(vec![local("old", 99, false)]),
        ))
        .unwrap();
    assert!(files
        .iter()
        .filter(|f| f.identity.item_id.is_some())
        .all(|f| f.local.state == FileState::Unknown));
    assert!(files
        .iter()
        .any(|f| f.inventory_lookup == InventoryLookupStatus::Ambiguous));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn unresolved_remote_identity_blocks_absence_even_when_the_listing_is_complete() {
    let (context, directory) = context();
    context
        .build_capture(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Local(vec![local("file", 1, false)]),
        ))
        .unwrap();
    let mut unknown = remote("inaccessible", "old-name", "drive-root", false);
    unknown.state = FileState::Unknown;
    unknown.name = Evidence::NotCollected;
    unknown.parents = Evidence::NotCollected;
    let files = context
        .build_capture(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Remote(vec![unknown]),
        ))
        .unwrap();
    let file = files
        .iter()
        .find(|f| f.local.path == Some(Path::from("file")))
        .unwrap();
    assert_eq!(file.remote.state, FileState::Unknown);
    assert_eq!(
        file.environment.destination_remote_matches,
        Evidence::NotCollected
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn three_level_unassociated_tree_gathers_candidates_but_keeps_nested_destinations_unresolved() {
    let (context, directory) = context();
    let locals = vec![
        local("a", 1, true),
        local("a/b", 2, true),
        local("a/b/c", 3, true),
        local("a/b/c/file", 4, false),
    ];
    context
        .build_capture(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Local(locals),
        ))
        .unwrap();
    let remotes = vec![
        remote("a", "a", "drive-root", true),
        remote("b", "b", "a", true),
        remote("c", "c", "b", true),
        remote("file", "file", "c", false),
    ];
    let files = context
        .build_capture(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Remote(remotes),
        ))
        .unwrap();
    let file = files
        .iter()
        .find(|f| f.remote.id == Some(RemoteId("file".into())))
        .unwrap();
    assert_eq!(file.association_candidates.len(), 1);
    assert_eq!(
        file.environment.destination_local_parent,
        Evidence::NotCollected
    );
    assert_eq!(file.local.state, FileState::Unknown);
    assert!(file.identity.item_id.is_none());
    assert!(context.snapshots().unwrap().is_empty());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn duplicate_remote_targets_never_resolve_a_free_local_destination() {
    let (context, directory) = context();
    context
        .build_capture(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Local(vec![]),
        ))
        .unwrap();
    let files = context
        .build_capture(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Remote(vec![
                remote("one", "same", "drive-root", false),
                remote("two", "same", "drive-root", false),
            ]),
        ))
        .unwrap();
    assert_eq!(files.len(), 2);
    for file in files {
        assert_eq!(file.local.state, FileState::Unknown);
        assert_eq!(
            file.environment.destination_local_occupied,
            Evidence::NotCollected
        );
        assert_eq!(
            file.environment.destination_local_parent,
            Evidence::NotCollected
        );
        assert_eq!(
            file.environment.destination_remote_matches,
            Evidence::Known(vec![RemoteId("one".into()), RemoteId("two".into())])
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}
