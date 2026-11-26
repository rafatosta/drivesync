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
        birth_ns: None,
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
        collect_epoch: 0,
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
        recovery: None,
        recovery_diagnostic: None,
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
        trash_propagation_authorized: false,
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

#[test]
fn atomic_slot_replacement_requires_unchanged_remote_and_absent_old_inode() {
    let (context, directory) = context();
    let old = local("file", 1, false);
    let r = remote("r", "file", "drive-root", false);
    seed(&context, old.clone(), r.clone());
    context
        .consolidate(Observation::Capture(Box::new(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Remote(vec![r.clone()]),
        ))))
        .unwrap();
    let replacement = local("file", 2, false);
    let files = context
        .consolidate(Observation::Capture(Box::new(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Local(vec![replacement.clone()]),
        ))))
        .unwrap();
    let item = files.iter().find(|f| f.identity.item_id.is_some()).unwrap();
    assert_eq!(item.environment.local_replacement, Evidence::Known(true));
    assert_eq!(item.identity.local_identity, old.identity);
    assert_eq!(item.local.identity, replacement.identity);
    let moved = local("elsewhere", 1, false);
    let files = context
        .consolidate(Observation::Capture(Box::new(round(
            &context,
            2,
            ObservationCoverage::Complete,
            CaptureEntries::Local(vec![replacement.clone(), moved]),
        ))))
        .unwrap();
    let item = files.iter().find(|f| f.identity.item_id.is_some()).unwrap();
    assert_ne!(item.environment.local_replacement, Evidence::Known(true));
    assert_eq!(item.local.state, FileState::Unknown);
    let mut changed = r;
    changed.version = Evidence::Known(2);
    changed.checksum = Evidence::Known(crate::models::Checksum {
        algorithm: "sha256".into(),
        value: "changed".into(),
    });
    context
        .consolidate(Observation::Capture(Box::new(round(
            &context,
            2,
            ObservationCoverage::Complete,
            CaptureEntries::Remote(vec![changed]),
        ))))
        .unwrap();
    let files = context
        .consolidate(Observation::Capture(Box::new(round(
            &context,
            3,
            ObservationCoverage::Complete,
            CaptureEntries::Local(vec![replacement]),
        ))))
        .unwrap();
    let item = files.iter().find(|f| f.identity.item_id.is_some()).unwrap();
    assert_ne!(item.environment.local_replacement, Evidence::Known(true));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn atomic_divergent_slot_becomes_conflict_evidence_without_automatic_overwrite() {
    let (context, directory) = context();
    let old = local("file", 1, false);
    let original = remote("r", "file", "drive-root", false);
    seed(&context, old.clone(), original.clone());
    let mut changed = original;
    changed.version = Evidence::Known(2);
    changed.checksum = Evidence::Known(crate::models::Checksum {
        algorithm: "sha256".into(),
        value: "remote-edit".into(),
    });
    context
        .build_capture(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Remote(vec![changed]),
        ))
        .unwrap();
    let mut replacement = local("file", 2, false);
    replacement.checksum = Evidence::Known(crate::models::Checksum {
        algorithm: "sha256".into(),
        value: "local-edit".into(),
    });
    let files = context
        .build_capture(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Local(vec![replacement.clone()]),
        ))
        .unwrap();
    let item = files.iter().find(|f| f.identity.item_id.is_some()).unwrap();
    assert_eq!(item.local.identity, replacement.identity);
    assert_eq!(item.identity.local_identity, old.identity);
    assert_eq!(item.environment.local_replacement, Evidence::Known(true));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn moved_incarnations_follow_trees_but_recycled_or_aliased_inodes_do_not() {
    for scenario in ["tree", "recycled", "hardlink", "partial", "outside"] {
        let (context, _directory) = context();
        let mut parent = local("a", 501, true);
        let mut child = local("a/file", 502, false);
        for entry in [&mut parent, &mut child] {
            if let Evidence::Known(i) = &mut entry.identity {
                i.birth_ns = Some(700);
            }
        }
        seed(
            &context,
            parent.clone(),
            remote("folder", "a", "remote-root", true),
        );
        seed(
            &context,
            child.clone(),
            remote("child", "file", "folder", false),
        );
        let mut moved_parent = parent.clone();
        moved_parent.path = Some("b".into());
        moved_parent.name = Evidence::Known("b".into());
        let mut moved_child = child.clone();
        moved_child.path = Some("b/file".into());
        moved_child.parent = Evidence::Known("b".into());
        if scenario == "recycled" {
            if let Evidence::Known(i) = &mut moved_child.identity {
                i.birth_ns = Some(701);
            }
        }
        let mut entries = vec![moved_parent, moved_child.clone()];
        if scenario == "hardlink" {
            let mut alias = moved_child;
            alias.path = Some("alias".into());
            alias.parent = Evidence::Known(Path::new());
            entries.push(alias);
        }
        if scenario == "outside" {
            entries.clear();
        }
        context
            .build_capture(round(
                &context,
                1,
                if scenario == "partial" {
                    ObservationCoverage::Partial
                } else {
                    ObservationCoverage::Complete
                },
                CaptureEntries::Local(entries),
            ))
            .unwrap();
        let files = context
            .build_capture(round(
                &context,
                1,
                ObservationCoverage::Complete,
                CaptureEntries::Remote(vec![
                    remote("folder", "a", "remote-root", true),
                    remote("child", "file", "folder", false),
                ]),
            ))
            .unwrap();
        let file = files
            .iter()
            .find(|f| f.identity.item_id == Some(ItemId("child".into())))
            .unwrap();
        if scenario == "tree" {
            assert_eq!(file.local.path, Some("b/file".into()));
            let parent_index = files
                .iter()
                .position(|f| f.identity.item_id == Some(ItemId("folder".into())))
                .unwrap();
            let child_index = files
                .iter()
                .position(|f| f.identity.item_id == Some(ItemId("child".into())))
                .unwrap();
            assert!(parent_index < child_index);
            assert_eq!(file.identity.remote_id, Some(RemoteId("child".into())));
            assert_eq!(
                file.environment.destination_remote_parent_id,
                Evidence::Known(RemoteId("folder".into()))
            );
        } else {
            assert_ne!(file.local.state, FileState::Present, "{scenario}");
        }
    }
}

#[test]
fn invalid_names_shortcuts_collisions_and_special_locals_are_per_item_refusals() {
    let (context, _directory) = context();
    let mut invalid = remote("bad", "bad/name", "drive-root", false);
    let mut doc = remote("doc", "Report", "drive-root", false);
    doc.mime_type = Evidence::Known("application/vnd.google-apps.document".into());
    doc.size = Evidence::NotApplicable;
    doc.checksum = Evidence::NotApplicable;
    let mut shortcut = remote("shortcut", "shortcut", "drive-root", false);
    shortcut.mime_type = Evidence::Known("application/vnd.google-apps.shortcut".into());
    invalid.checksum = Evidence::Unavailable;
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
            CaptureEntries::Remote(vec![invalid, doc.clone(), shortcut]),
        ))
        .unwrap();
    assert!(files
        .iter()
        .find(|f| f.remote.id == Some(RemoteId("bad".into())))
        .unwrap()
        .environment
        .projection_issue
        .is_some());
    assert!(files
        .iter()
        .find(|f| f.remote.id == Some(RemoteId("shortcut".into())))
        .unwrap()
        .environment
        .projection_issue
        .is_some());
    let f = files
        .iter()
        .find(|f| f.remote.id == Some(RemoteId("doc".into())))
        .unwrap();
    assert_eq!(f.local.path, Some("Report.html".into()));
    assert_eq!(f.local.state, FileState::Absent);
    assert!(f.environment.projection_issue.is_none());
    assert_eq!(
        f.environment.destination_remote_matches,
        Evidence::Known(vec![RemoteId("doc".into())])
    );
    let mut blob = remote("blob", "Report.html", "drive-root", false);
    blob.checksum = Evidence::Known(crate::models::Checksum {
        algorithm: "sha256".into(),
        value: "a".repeat(64),
    });
    let files = context
        .build_capture(round(
            &context,
            2,
            ObservationCoverage::Complete,
            CaptureEntries::Remote(vec![doc, blob]),
        ))
        .unwrap();
    assert!(files
        .iter()
        .filter(|f| f.remote.id.is_some())
        .all(|f| f.environment.projection_issue.is_some()));
    let duplicate_a = remote("dup-a", "duplicate", "drive-root", false);
    let duplicate_b = remote("dup-b", "duplicate", "drive-root", false);
    let mut no_hash = remote("no-hash", "no-hash", "drive-root", false);
    no_hash.checksum = Evidence::Unavailable;
    let files = context
        .build_capture(round(
            &context,
            3,
            ObservationCoverage::Complete,
            CaptureEntries::Remote(vec![duplicate_a, duplicate_b, no_hash]),
        ))
        .unwrap();
    assert!(files
        .iter()
        .filter(|f| f.remote.id.is_some())
        .all(|f| f.environment.projection_issue.is_some()));
    let mut link = local("link", 88, false);
    link.kind = Evidence::Known(crate::models::FileKind::Symlink);
    let mut fifo = local("fifo", 89, false);
    fifo.kind = Evidence::Known(crate::models::FileKind::Unsupported);
    let mut nonutf = local("bytes", 90, false);
    nonutf.name = Evidence::Unsupported;
    let files = context
        .build_capture(round(
            &context,
            2,
            ObservationCoverage::Complete,
            CaptureEntries::Local(vec![link, fifo, nonutf]),
        ))
        .unwrap();
    for path in ["link", "fifo", "bytes"] {
        assert!(files
            .iter()
            .find(|f| f.local.path == Some(path.into()))
            .unwrap()
            .environment
            .projection_issue
            .is_some());
    }
}

#[test]
fn folder_trash_evidence_requires_complete_empty_captures_and_no_registered_descendant() {
    let (context, _directory) = context();
    let folder = local("folder", 41, true);
    seed(
        &context,
        folder.clone(),
        remote("folder", "folder", "drive-root", true),
    );
    context
        .build_capture(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Local(vec![folder.clone()]),
        ))
        .unwrap();
    let files = context
        .build_capture(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Remote(vec![remote("folder", "folder", "drive-root", true)]),
        ))
        .unwrap();
    let parent = files
        .iter()
        .find(|f| f.identity.remote_id == Some(RemoteId("folder".into())))
        .unwrap();
    assert_eq!(parent.environment.trash_tree_safe, Evidence::Known(true));
    seed(
        &context,
        local("folder/child", 42, false),
        remote("child", "child", "folder", false),
    );
    let files = context
        .build_capture(round(
            &context,
            2,
            ObservationCoverage::Complete,
            CaptureEntries::Local(vec![folder.clone()]),
        ))
        .unwrap();
    let parent = files
        .iter()
        .find(|f| f.identity.remote_id == Some(RemoteId("folder".into())))
        .unwrap();
    assert_eq!(parent.environment.trash_tree_safe, Evidence::NotCollected);
    let mut removed_child = remote("child", "child", "folder", false);
    removed_child.state = FileState::Absent;
    removed_child.trashed = Evidence::Known(true);
    let files = context
        .build_capture(round(
            &context,
            2,
            ObservationCoverage::Complete,
            CaptureEntries::Remote(vec![
                remote("folder", "folder", "drive-root", true),
                removed_child,
            ]),
        ))
        .unwrap();
    let parent = files
        .iter()
        .find(|f| f.identity.remote_id == Some(RemoteId("folder".into())))
        .unwrap();
    assert_eq!(parent.environment.trash_tree_safe, Evidence::Known(false));
    let files = context
        .build_capture(round(
            &context,
            3,
            ObservationCoverage::Partial,
            CaptureEntries::Local(vec![folder]),
        ))
        .unwrap();
    let parent = files
        .iter()
        .find(|f| f.identity.remote_id == Some(RemoteId("folder".into())))
        .unwrap();
    assert_eq!(parent.environment.trash_tree_safe, Evidence::NotCollected);
}

#[test]
fn trashed_remote_retains_parent_correspondence_without_claiming_a_free_destination() {
    let (mut context, _directory) = context();
    context.config.trash_propagation_authorized = true;
    let entry = local("file", 43, false);
    let mut source = remote("file", "file", "drive-root", false);
    seed(&context, entry.clone(), source.clone());
    context
        .build_capture(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Local(vec![entry]),
        ))
        .unwrap();
    source.state = FileState::Absent;
    source.trashed = Evidence::Known(true);
    source.version = Evidence::Known(2);
    let mut inaccessible = remote("inaccessible", "inaccessible", "drive-root", false);
    inaccessible.state = FileState::Unknown;
    let files = context
        .build_capture(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Remote(vec![source, inaccessible]),
        ))
        .unwrap();
    let file = files
        .iter()
        .find(|f| f.identity.item_id == Some(ItemId("file".into())))
        .unwrap();
    assert_eq!(
        file.environment.destination_local_parent,
        Evidence::Known(Path::new())
    );
    assert_eq!(
        file.environment.destination_remote_parent_id,
        Evidence::Known(RemoteId("drive-root".into()))
    );
    assert_ne!(
        file.environment.destination_local_occupied,
        Evidence::Known(false)
    );
}

#[test]
fn completion_requires_registered_convergence_current_epochs_and_no_unresolved_items() {
    use std::os::unix::fs::MetadataExt;
    let (mut context, directory) = context();
    let root = directory.join("audit-root");
    std::fs::create_dir(&root).unwrap();
    context.config.local_root = PathRoot(root.clone());
    let m = std::fs::metadata(root).unwrap();
    let expected = LocalIdentity {
        device_id: m.dev(),
        inode: m.ino(),
        mount_id: None,
        birth_ns: crate::models::birth_ns(&m),
    };
    context
        .build_capture(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Local(vec![]),
        ))
        .unwrap();
    context
        .build_capture(round(
            &context,
            1,
            ObservationCoverage::Complete,
            CaptureEntries::Remote(vec![]),
        ))
        .unwrap();
    assert!(context.audit(0, &expected).unwrap().initial_complete);
    assert!(!context.audit(1, &expected).unwrap().initial_complete);
    let mut item = local("file", 4, false);
    item.checksum = Evidence::Known(crate::models::Checksum {
        algorithm: "sha256".into(),
        value: "a".repeat(64),
    });
    let mut source = remote("file", "file", "drive-root", false);
    source.checksum = item.checksum.clone();
    context
        .build_capture(round(
            &context,
            2,
            ObservationCoverage::Complete,
            CaptureEntries::Local(vec![item.clone()]),
        ))
        .unwrap();
    context
        .build_capture(round(
            &context,
            2,
            ObservationCoverage::Complete,
            CaptureEntries::Remote(vec![source.clone()]),
        ))
        .unwrap();
    assert!(!context.audit(0, &expected).unwrap().initial_complete);
    seed(&context, item, source);
    assert!(context.audit(0, &expected).unwrap().initial_complete);
    let mut edited = local("file", 4, false);
    edited.size = Evidence::Known(8);
    context
        .build_capture(round(
            &context,
            3,
            ObservationCoverage::Complete,
            CaptureEntries::Local(vec![edited]),
        ))
        .unwrap();
    assert!(!context.audit(0, &expected).unwrap().initial_complete);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn equivalent_complete_capture_refreshes_provenance_without_rebuilding_items() {
    let (context, directory) = context();
    let file = local("file", 8, false);
    let submit = |capture| {
        context
            .consolidate(Observation::Capture(Box::new(capture)))
            .unwrap()
    };
    submit(round(
        &context,
        1,
        ObservationCoverage::Complete,
        CaptureEntries::Remote(vec![]),
    ));
    let first = submit(round(
        &context,
        1,
        ObservationCoverage::Complete,
        CaptureEntries::Local(vec![file.clone()]),
    ));
    assert!(first[0].registration_id.is_some());

    let mut updated = file.clone();
    updated.observed_at.unix_millis += 100;
    let mut capture = round(
        &context,
        2,
        ObservationCoverage::Complete,
        CaptureEntries::Local(vec![updated]),
    );
    capture.collect_epoch = 3;
    let second = submit(capture);
    assert_eq!(second[0].registration_id, first[0].registration_id);
    assert_eq!(second[0].observations.local_epoch, Some(3));
    assert_eq!(second[0].observations.local_generation, Some(2));

    let mut changed = file;
    changed.size = Evidence::Known(123);
    let third = submit(round(
        &context,
        3,
        ObservationCoverage::Complete,
        CaptureEntries::Local(vec![changed]),
    ));
    assert_eq!(third[0].local.size, Evidence::Known(123));
    assert_ne!(third[0].registration_id, second[0].registration_id);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn large_context_batch_reports_real_progress_and_keeps_file_evidence() {
    use crate::detectors::activity::{ActivityChannel, Details, Origin, Phase, Scope, State};
    let (context, directory) = context();
    let total = 1024;
    let entries = (0..total)
        .map(|index| local(&format!("private-entry-{index:04}"), index + 2000, true))
        .collect();
    let capture = round(
        &context,
        1,
        ObservationCoverage::Complete,
        CaptureEntries::Local(entries),
    );
    let channel = ActivityChannel::with_diagnostics(
        Scope {
            account: context.config.account_id.clone(),
            root: context.config.sync_root_id.clone(),
        },
        true,
    );
    let scope = channel.shared().begin(
        Origin::Shared,
        None,
        Phase::Contextualize,
        State::Running,
        Details::default(),
    );
    let files = context
        .consolidate_observed(Observation::Capture(Box::new(capture)), &scope)
        .unwrap();
    assert_eq!(files.len(), total as usize);
    assert!(files.iter().all(|file| {
        file.observations.local_coverage == ObservationCoverage::Complete
            && file.observations.remote_coverage == ObservationCoverage::Unavailable
            && matches!(file.local.identity, Evidence::Known(_))
    }));
    let log = channel.inbox().test_diagnostics();
    assert!(log.lines().any(|line| line
        .contains("phase=Contextualize event=Finished state=Complete")
        && line.contains("Observations=1024/1024 Queries=1024")));
    assert!(!log.contains("private-entry-"));
    scope.finish(State::Complete);
    assert!(channel.inbox().current().is_empty());
    std::fs::remove_dir_all(directory).unwrap();
}
