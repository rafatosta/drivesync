//! SYNC-001 acceptance algorithms with real files/SQLite and a fake remote.
use super::*;
use crate::{ConfirmedBase, ConflictReason, DownloadPlan, LocalObservation};

/// Reopens durable state and uses fresh descriptor-based evidence as the facade does.
fn prepare(f: &Fixture, inv: &RemoteInventory) -> Result<DownloadPlan, EngineError> {
    let mut store = Store::open(&f.db())?;
    let bases = store.confirmed_bases("client", &inv.account_id, &inv.root_id, &f.root)?;
    let plan = crate::planner::plan_with_bases(inv, &f.root, &bases, |_, path, base| {
        let observed = crate::planner::observe(&f.root, path);
        if base.is_some() && observed == LocalObservation::UnmanagedFile {
            Ok(
                filesystem::fingerprint(&f.root, path, &Cancellation::new())?
                    .map(LocalObservation::Verified)
                    .unwrap_or(LocalObservation::Unknown),
            )
        } else {
            Ok(observed)
        }
    })?;
    store.save("client", inv, &plan, &Cancellation::new())?;
    Ok(plan)
}
fn base(f: &Fixture, inv: &RemoteInventory) -> Option<ConfirmedBase> {
    Store::open(&f.db())
        .unwrap()
        .confirmed_bases("client", &inv.account_id, &inv.root_id, &f.root)
        .unwrap()
        .remove(&inv.entries[0].id)
}
fn cycle(
    f: &Fixture,
    inv: &RemoteInventory,
    source: &mut Fake,
) -> Result<DownloadReport, EngineError> {
    let plan = prepare(f, inv)?;
    let store = Store::open(&f.db())?;
    run(
        Context {
            client: "client",
            scope: &plan,
            store: &store,
            cancel: &Cancellation::new(),
        },
        inv,
        source,
        |_| {},
    )
}
fn changed(inv: &RemoteInventory) -> (RemoteInventory, Fake) {
    let mut next = inv.clone();
    next.entries[0].version = Some(2);
    next.entries[0].md5_checksum = Some("4ed9407630eb1000c0f6b63842defa7d".into());
    let mut source = Fake::new(&next);
    source.content.insert("file".into(), b"def".to_vec());
    (next, source)
}

#[test]
fn observed_only_and_first_sync_never_adopt_equal_or_different_local_bytes() {
    for bytes in [b"abc", b"xyz"] {
        let f = Fixture::new();
        let inv = inventory();
        assert_eq!(
            prepare(&f, &inv).unwrap().entries[0].action,
            PlanAction::Download
        );
        assert!(base(&f, &inv).is_none());
        fs::write(f.root.join("file.txt"), bytes).unwrap();
        let mut source = Fake::new(&inv);
        for _ in 0..2 {
            assert_eq!(
                prepare(&f, &inv).unwrap().entries[0].action,
                PlanAction::Blocked(ConflictReason::ExistingLocalData)
            );
            assert_eq!(
                cycle(&f, &inv, &mut source).unwrap().issues[0].error,
                EngineError::DownloadConflict
            );
            assert!(base(&f, &inv).is_none());
            assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), bytes);
        }
        assert_eq!(source.media_calls, 0);
    }
}

#[test]
fn reopened_base_classifies_local_and_offline_concurrent_edits_without_advancing() {
    for concurrent in [false, true] {
        let f = Fixture::new();
        let inv = inventory();
        cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
        let confirmed = base(&f, &inv).unwrap();
        fs::write(f.root.join("file.txt"), b"local edit").unwrap();
        let (next, mut source) = if concurrent {
            changed(&inv)
        } else {
            (inv.clone(), Fake::new(&inv))
        };
        for _ in 0..2 {
            let reason = if concurrent {
                ConflictReason::ConcurrentChanges
            } else {
                ConflictReason::LocalChanged
            };
            assert_eq!(
                prepare(&f, &next).unwrap().entries[0].action,
                PlanAction::Blocked(reason)
            );
            assert_eq!(
                cycle(&f, &next, &mut source).unwrap().issues[0].error,
                EngineError::DownloadConflict
            );
            assert_eq!(base(&f, &inv), Some(confirmed.clone()));
            assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"local edit");
        }
        assert_eq!(source.media_calls, 0);
        assert_eq!(source.entries["file"], next.entries[0]);
    }
}

#[test]
fn updated_base_survives_reopen_and_classifies_later_local_edit_against_v2() {
    let f = Fixture::new();
    let inv = inventory();
    let mut initial = Fake::new(&inv);
    assert_eq!(cycle(&f, &inv, &mut initial).unwrap().downloaded, 1);
    let v1 = base(&f, &inv).unwrap();
    assert_eq!(cycle(&f, &inv, &mut initial).unwrap().unchanged, 1);
    assert_eq!(initial.media_calls, 1);
    let (next, mut source) = changed(&inv);
    assert_eq!(
        prepare(&f, &next).unwrap().entries[0].action,
        PlanAction::Download
    );
    assert_eq!(base(&f, &inv), Some(v1.clone()));
    assert_eq!(cycle(&f, &next, &mut source).unwrap().updated, 1);
    let v2 = base(&f, &next).unwrap();
    assert_eq!(v2.remote_version, 2);
    assert_ne!(v1.local_fingerprint, v2.local_fingerprint);
    assert_eq!(
        prepare(&f, &next).unwrap().entries[0].action,
        PlanAction::NoOp
    );
    assert_eq!(cycle(&f, &next, &mut source).unwrap().unchanged, 1);
    fs::write(f.root.join("file.txt"), b"local after v2").unwrap();
    for _ in 0..2 {
        assert_eq!(
            prepare(&f, &next).unwrap().entries[0].action,
            PlanAction::Blocked(ConflictReason::LocalChanged)
        );
        assert_eq!(
            cycle(&f, &next, &mut source).unwrap().issues[0].error,
            EngineError::DownloadConflict
        );
        assert_eq!(base(&f, &next), Some(v2.clone()));
        assert_eq!(
            fs::read(f.root.join("file.txt")).unwrap(),
            b"local after v2"
        );
    }
    assert_eq!(source.media_calls, 1);
}

#[test]
fn missing_or_observation_only_state_cannot_reconstruct_a_base() {
    for observation_only in [false, true] {
        let f = Fixture::new();
        let inv = inventory();
        cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
        if observation_only {
            rusqlite::Connection::open(f.db())
                .unwrap()
                .execute("DELETE FROM downloads", [])
                .unwrap();
        } else {
            fs::remove_file(f.db()).unwrap();
        }
        let mut source = Fake::new(&inv);
        for _ in 0..2 {
            assert_eq!(
                prepare(&f, &inv).unwrap().entries[0].action,
                PlanAction::Blocked(ConflictReason::ExistingLocalData)
            );
            assert_eq!(
                cycle(&f, &inv, &mut source).unwrap().issues[0].error,
                EngineError::DownloadConflict
            );
            assert!(base(&f, &inv).is_none());
            assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"abc");
        }
        assert_eq!(source.media_calls, 0);
    }
}

#[test]
fn corrupt_database_or_malformed_installation_evidence_stops_before_effects() {
    for malformed_record in [false, true] {
        let f = Fixture::new();
        let inv = inventory();
        cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
        if malformed_record {
            rusqlite::Connection::open(f.db())
                .unwrap()
                .execute("UPDATE downloads SET record='{}'", [])
                .unwrap();
        } else {
            fs::write(f.db(), b"corrupt database").unwrap();
        }
        let before = fs::read(f.db()).unwrap();
        let (next, mut source) = changed(&inv);
        assert_eq!(
            cycle(&f, &next, &mut source).unwrap_err(),
            EngineError::StateStorage
        );
        assert_eq!(source.media_calls, 0);
        assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"abc");
        assert_eq!(fs::read(f.db()).unwrap(), before);
    }
}

#[test]
fn later_incomplete_transfer_masks_old_base_until_verified_recovery() {
    for update in [false, true] {
        let f = Fixture::new();
        let inv = inventory();
        if update {
            cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
        }
        let (next, mut source) = if update {
            changed(&inv)
        } else {
            (inv.clone(), Fake::new(&inv))
        };
        source.fail_stream = true;
        assert_eq!(cycle(&f, &next, &mut source).unwrap().issues.len(), 1);
        assert!(base(&f, &next).is_none());
        if update {
            assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"abc");
        } else {
            assert!(!f.root.join("file.txt").exists());
        }
        source.fail_stream = false;
        let report = cycle(&f, &next, &mut source).unwrap();
        assert!(report.issues.is_empty());
        assert_eq!(
            base(&f, &next).unwrap().remote_version,
            next.entries[0].version.unwrap()
        );
        assert_eq!(cycle(&f, &next, &mut source).unwrap().unchanged, 1);
    }
}

#[test]
fn filesystem_effect_without_confirmation_never_exposes_old_or_new_base() {
    for update in [false, true] {
        let f = Fixture::new();
        let inv = inventory();
        if update {
            cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
        }
        let (next, mut source) = if update {
            changed(&inv)
        } else {
            (inv.clone(), Fake::new(&inv))
        };
        prepare(&f, &next).unwrap();
        let conn = rusqlite::Connection::open(f.db()).unwrap();
        conn.execute_batch("CREATE TRIGGER fail_confirm BEFORE UPDATE ON downloads WHEN json_extract(NEW.record,'$.phase.Installed') IS NOT NULL OR json_extract(NEW.record,'$.phase.Updated') IS NOT NULL BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
        assert_eq!(
            cycle(&f, &next, &mut source).unwrap_err(),
            EngineError::StateStorage
        );
        assert_eq!(
            fs::read(f.root.join("file.txt")).unwrap(),
            source.content["file"]
        );
        assert!(base(&f, &next).is_none());
        conn.execute_batch("DROP TRIGGER fail_confirm").unwrap();
        drop(conn);
        assert_eq!(cycle(&f, &next, &mut source).unwrap().recovered, 1);
        assert_eq!(
            base(&f, &next).unwrap().remote_version,
            next.entries[0].version.unwrap()
        );
        assert_eq!(cycle(&f, &next, &mut source).unwrap().unchanged, 1);
        assert_eq!(source.media_calls, 1);
    }
}

#[test]
fn inode_replacement_and_foreign_scopes_never_inherit_write_authority() {
    for scope in 0..5 {
        let mut f = Fixture::new();
        let mut inv = inventory();
        cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
        let confirmed = base(&f, &inv).unwrap();
        match scope {
            0 => {
                let old = fs::metadata(f.root.join("file.txt")).unwrap().ino();
                fs::rename(f.root.join("file.txt"), f.root.join("original")).unwrap();
                fs::write(f.root.join("file.txt"), b"abc").unwrap();
                assert_ne!(fs::metadata(f.root.join("file.txt")).unwrap().ino(), old);
            }
            1 => inv.account_id = AccountId("other-account".into()),
            2 => {
                inv.root_id = "other-root".into();
                inv.entries[0].parent_id = inv.root_id.clone();
            }
            3 => {
                f.root = f.base.join("other-local");
                fs::create_dir(&f.root).unwrap();
                fs::write(f.root.join("file.txt"), b"abc").unwrap();
            }
            _ => inv.entries[0].id = "other-id".into(),
        }
        let mut source = Fake::new(&inv);
        for _ in 0..2 {
            let expected = if scope == 0 {
                ConflictReason::LocalChanged
            } else {
                ConflictReason::ExistingLocalData
            };
            assert_eq!(
                prepare(&f, &inv).unwrap().entries[0].action,
                PlanAction::Blocked(expected)
            );
            assert_eq!(
                cycle(&f, &inv, &mut source).unwrap().issues[0].error,
                EngineError::DownloadConflict
            );
            assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"abc");
        }
        assert_eq!(source.media_calls, 0);
        if scope == 0 {
            assert_eq!(base(&f, &inv), Some(confirmed));
        } else {
            assert!(base(&f, &inv).is_none());
        }
    }
}

#[test]
fn unreadable_database_is_an_error_and_is_not_reinitialized() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let inv = inventory();
    cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
    let before = fs::read(f.db()).unwrap();
    fs::set_permissions(f.db(), fs::Permissions::from_mode(0o000)).unwrap();
    let mut source = Fake::new(&inv);
    let result = cycle(&f, &inv, &mut source);
    fs::set_permissions(f.db(), fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(result.unwrap_err(), EngineError::StateStorage);
    assert_eq!(source.media_calls, 0);
    assert_eq!(fs::read(f.db()).unwrap(), before);
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"abc");
}

#[test]
fn installed_phase_without_remote_revision_is_not_a_confirmed_base() {
    let f = Fixture::new();
    let inv = inventory();
    cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
    let conn = rusqlite::Connection::open(f.db()).unwrap();
    conn.execute(
        "UPDATE downloads SET record=json_set(record,'$.item.version',NULL)",
        [],
    )
    .unwrap();
    drop(conn);
    assert!(base(&f, &inv).is_none());
    assert_eq!(
        prepare(&f, &inv).unwrap().entries[0].action,
        PlanAction::Blocked(ConflictReason::ExistingLocalData)
    );
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"abc");
}

#[test]
fn cancellation_during_initial_or_update_transfer_does_not_confirm_partial_bytes() {
    for update in [false, true] {
        let f = Fixture::new();
        let inv = inventory();
        if update {
            cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
        }
        let (next, mut source) = if update {
            changed(&inv)
        } else {
            (inv.clone(), Fake::new(&inv))
        };
        let plan = prepare(&f, &next).unwrap();
        let store = Store::open(&f.db()).unwrap();
        let cancel = Cancellation::new();
        let result = run(
            Context {
                client: "client",
                scope: &plan,
                store: &store,
                cancel: &cancel,
            },
            &next,
            &mut source,
            |p| {
                if p.current.is_some_and(|item| {
                    item.stage == DownloadStage::Downloading && item.received > 0
                }) {
                    cancel.cancel();
                }
            },
        );
        assert_eq!(
            result.unwrap_err(),
            EngineError::Authentication(crate::AuthError::Cancelled)
        );
        drop(store);
        assert!(base(&f, &next).is_none());
        if update {
            assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"abc");
        } else {
            assert!(!f.root.join("file.txt").exists());
        }
        assert!(cycle(&f, &next, &mut source).unwrap().issues.is_empty());
        assert_eq!(
            base(&f, &next).unwrap().remote_version,
            next.entries[0].version.unwrap()
        );
    }
}
