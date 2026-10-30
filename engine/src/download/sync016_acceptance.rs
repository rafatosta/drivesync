//! Rename safety oracles with real files/SQLite and a simulated remote provider.
use super::sync006_acceptance::{execute, prepare_record, remote_bytes, with_folder};
use super::*;
use crate::{
    state::{CheckpointScope, Reconciliation, Reconciliations},
    ChangeKind, DifferenceKind, DifferenceReason,
};

/// Persists and reopens observational evidence without advancing installed B.
fn reopen(f: &Fixture, record: &Reconciliation) -> Reconciliation {
    let scope = CheckpointScope {
        client: "client",
        account: &record.plan.account_id,
        remote: &record.plan.remote_root,
        local: &f.root,
    };
    let path = f.db().with_file_name("reconciliation.sqlite3");
    let mut store = Reconciliations::open(&path).unwrap();
    let previous = store.load(&scope).unwrap();
    store
        .save(
            &scope,
            previous.as_ref().map(|(revision, _)| revision.as_str()),
            record,
            &Cancellation::new(),
        )
        .unwrap();
    drop(store);
    let recovered = Reconciliations::open(&path)
        .unwrap()
        .load(&scope)
        .unwrap()
        .unwrap()
        .1;
    assert_eq!(&recovered, record);
    recovered
}

#[test]
fn remote_only_rename_or_move_never_downloads_a_false_copy() {
    for moved in [false, true] {
        let f = Fixture::new();
        let mut inv = with_folder();
        baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
        let base = baseline_tests::base(&f, &inv).unwrap();
        let destination = if moved {
            inv.entries[0].parent_id = "folder".into();
            "folder/file.txt"
        } else {
            inv.entries[0].name = "renamed.txt".into();
            "renamed.txt"
        };
        let mut remote = remote_bytes(&mut inv, b"abc");
        for _ in 0..3 {
            let record = reopen(&f, &prepare_record(&f, &inv, &mut remote));
            assert!(matches!(record.plan.entries[0].action,
                PlanAction::Classified { decision, .. }
                if decision.kind == DifferenceKind::RemoteChanged));
            let report = execute(&f, &inv, &record.plan, &mut remote);
            assert_eq!(report.downloaded + report.updated, 0);
            assert!(report.conflict_copies.is_empty());
            assert!(report.preserved_copies.is_empty());
            assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"abc");
            assert!(!f.root.join(destination).exists());
            assert_eq!(baseline_tests::base(&f, &inv), Some(base.clone()));
        }
    }
}

#[test]
fn c07_equal_and_divergent_renames_survive_offline_restart_and_repetition() {
    for equal in [false, true] {
        let f = Fixture::new();
        let mut inv = inventory();
        baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
        let base = baseline_tests::base(&f, &inv).unwrap();
        fs::rename(f.root.join("file.txt"), f.root.join("local.txt")).unwrap();
        inv.entries[0].name = if equal { "local.txt" } else { "remote.txt" }.into();
        let mut remote = remote_bytes(&mut inv, b"abc");
        for offline in [true, false, true, false, false] {
            remote.fail_stream = offline;
            let record = reopen(&f, &prepare_record(&f, &inv, &mut remote));
            if offline {
                assert_eq!(record.plan.entries[0].action, PlanAction::DeferredUnknown);
            } else {
                let PlanAction::Classified {
                    decision,
                    local_path,
                } = &record.plan.entries[0].action
                else {
                    panic!("rename classification missing");
                };
                assert_eq!(local_path, &PathBuf::from("local.txt"));
                assert_eq!(
                    decision.kind,
                    if equal {
                        DifferenceKind::Equivalent
                    } else {
                        DifferenceKind::Conflict
                    }
                );
                assert_eq!(
                    decision.reason,
                    if equal {
                        DifferenceReason::EqualContent
                    } else {
                        DifferenceReason::DivergentLocation
                    }
                );
            }
            assert!(!record
                .entries
                .iter()
                .any(|e| e.kind == ChangeKind::LocalNew));
            let report = execute(&f, &inv, &record.plan, &mut remote);
            assert_eq!(report.downloaded + report.updated, 0);
            assert!(report.conflict_copies.is_empty());
            assert_eq!(fs::read(f.root.join("local.txt")).unwrap(), b"abc");
            assert!(!f.root.join("file.txt").exists());
            assert!(!f.root.join("remote.txt").exists());
            assert_eq!(remote.content["file"], b"abc");
            assert_eq!(baseline_tests::base(&f, &inv), Some(base.clone()));
        }
    }
}

#[test]
fn c09_both_directions_resume_after_offline_without_selecting_a_version() {
    for local_rename in [false, true] {
        let f = Fixture::new();
        let mut inv = inventory();
        baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
        let base = baseline_tests::base(&f, &inv).unwrap();
        let (local_path, local_bytes, remote_content): (&str, &[u8], &[u8]) = if local_rename {
            fs::rename(f.root.join("file.txt"), f.root.join("renamed.txt")).unwrap();
            ("renamed.txt", b"abc", b"remote edit")
        } else {
            fs::write(f.root.join("file.txt"), b"local edit").unwrap();
            inv.entries[0].name = "renamed.txt".into();
            ("file.txt", b"local edit", b"abc")
        };
        let mut remote = remote_bytes(&mut inv, remote_content);
        for offline in [true, false, false] {
            remote.fail_stream = offline;
            let record = reopen(&f, &prepare_record(&f, &inv, &mut remote));
            if offline {
                assert_eq!(record.plan.entries[0].action, PlanAction::DeferredUnknown);
            } else {
                assert!(matches!(record.plan.entries[0].action,
                    PlanAction::Classified { decision, .. }
                    if decision.kind == DifferenceKind::Conciliable
                    && decision.reason == DifferenceReason::IndependentChanges));
            }
            let report = execute(&f, &inv, &record.plan, &mut remote);
            assert_eq!(report.downloaded + report.updated, 0);
            assert!(report.conflict_copies.is_empty());
            assert_eq!(fs::read(f.root.join(local_path)).unwrap(), local_bytes);
            assert_eq!(remote.content["file"], remote_content);
            assert_eq!(baseline_tests::base(&f, &inv), Some(base.clone()));
            assert_eq!(fs::read_dir(&f.root).unwrap().count(), 1);
        }
    }
}

#[test]
fn rename_proof_rejects_local_edits_replacements_and_remote_revision_races() {
    for race in ["edit", "replace", "remote"] {
        let f = Fixture::new();
        let mut inv = inventory();
        baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
        let base = baseline_tests::base(&f, &inv).unwrap();
        inv.entries[0].name = "renamed.txt".into();
        let mut remote = remote_bytes(&mut inv, b"abc");
        let root = f.root.clone();
        match race {
            "remote" => remote.changed = true,
            "edit" => {
                remote.on_media = Some(Box::new(move || {
                    fs::write(root.join("file.txt"), b"concurrent edit").unwrap();
                }))
            }
            _ => {
                remote.on_media = Some(Box::new(move || {
                    fs::rename(root.join("file.txt"), root.join("held-original")).unwrap();
                    fs::write(root.join("file.txt"), b"abc").unwrap();
                }))
            }
        }
        let record = reopen(&f, &prepare_record(&f, &inv, &mut remote));
        assert_eq!(record.plan.entries[0].action, PlanAction::DeferredUnknown);
        let report = execute(&f, &inv, &record.plan, &mut remote);
        assert_eq!(report.downloaded + report.updated, 0);
        assert!(!f.root.join("renamed.txt").exists());
        assert_eq!(baseline_tests::base(&f, &inv), Some(base));
        assert_eq!(
            fs::read(f.root.join("file.txt")).unwrap(),
            if race == "edit" {
                b"concurrent edit".as_slice()
            } else {
                b"abc".as_slice()
            }
        );
    }
}

#[test]
fn ancestor_rename_defers_children_without_downloading_duplicate_content() {
    let f = Fixture::new();
    let mut inv = with_folder();
    inv.entries[0].parent_id = "folder".into();
    baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
    let base = baseline_tests::base(&f, &inv).unwrap();
    inv.entries[1].name = "renamed-folder".into();
    inv.entries[1].version = Some(2);
    let mut remote = Fake::new(&inv);
    for _ in 0..2 {
        let record = reopen(&f, &prepare_record(&f, &inv, &mut remote));
        assert_eq!(record.plan.entries[0].action, PlanAction::DeferredUnknown);
        let report = execute(&f, &inv, &record.plan, &mut remote);
        assert_eq!(report.downloaded + report.updated, 0);
        assert!(!f.root.join("renamed-folder/file.txt").exists());
        assert_eq!(fs::read(f.root.join("folder/file.txt")).unwrap(), b"abc");
        assert_eq!(baseline_tests::base(&f, &inv), Some(base.clone()));
    }
    assert_eq!(remote.media_calls, 0);
}

#[test]
fn rename_with_local_edit_or_ambiguous_aliases_is_unknown_instead_of_new() {
    for alias in [false, true] {
        let f = Fixture::new();
        let mut inv = inventory();
        baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
        let base = baseline_tests::base(&f, &inv).unwrap();
        fs::rename(f.root.join("file.txt"), f.root.join("local.txt")).unwrap();
        if alias {
            fs::hard_link(f.root.join("local.txt"), f.root.join("alias.txt")).unwrap();
        } else {
            fs::write(f.root.join("local.txt"), b"local edit").unwrap();
        }
        inv.entries[0].name = "remote.txt".into();
        let mut remote = remote_bytes(&mut inv, b"abc");
        let record = reopen(&f, &prepare_record(&f, &inv, &mut remote));
        assert!(!record
            .entries
            .iter()
            .any(|e| e.kind == ChangeKind::LocalNew));
        assert!(record
            .entries
            .iter()
            .any(|e| e.relative_path == Some("local.txt".into()) && e.kind == ChangeKind::Unknown));
        let report = execute(&f, &inv, &record.plan, &mut remote);
        assert_eq!(report.downloaded + report.updated, 0);
        assert!(!f.root.join("remote.txt").exists());
        assert_eq!(baseline_tests::base(&f, &inv), Some(base));
    }
}

#[test]
fn c07_failed_snapshot_commit_and_cancellation_preserve_the_last_observation() {
    let f = Fixture::new();
    let mut inv = inventory();
    baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
    let base = baseline_tests::base(&f, &inv).unwrap();
    fs::rename(f.root.join("file.txt"), f.root.join("local.txt")).unwrap();
    inv.entries[0].name = "remote.txt".into();
    let mut remote = remote_bytes(&mut inv, b"abc");
    let record = reopen(&f, &prepare_record(&f, &inv, &mut remote));
    let scope = CheckpointScope {
        client: "client",
        account: &inv.account_id,
        remote: "root",
        local: &f.root,
    };
    let path = f.db().with_file_name("reconciliation.sqlite3");
    let mut store = Reconciliations::open(&path).unwrap();
    let previous = store.load(&scope).unwrap().unwrap();
    remote.fail_stream = true;
    let unknown = prepare_record(&f, &inv, &mut remote);
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_snapshot BEFORE UPDATE ON snapshots BEGIN SELECT RAISE(ABORT, 'injected failure'); END;").unwrap();
    assert_eq!(
        store.save(&scope, Some(&previous.0), &unknown, &Cancellation::new()),
        Err(EngineError::StateStorage)
    );
    conn.execute_batch("DROP TRIGGER fail_snapshot;").unwrap();
    let cancel = Cancellation::new();
    cancel.cancel();
    assert_eq!(
        store.save(&scope, Some(&previous.0), &unknown, &cancel),
        Err(EngineError::Authentication(crate::AuthError::Cancelled))
    );
    drop(store);
    assert_eq!(
        Reconciliations::open(&path)
            .unwrap()
            .load(&scope)
            .unwrap()
            .unwrap(),
        previous
    );
    assert_eq!(record, previous.1);
    assert_eq!(fs::read(f.root.join("local.txt")).unwrap(), b"abc");
    assert_eq!(baseline_tests::base(&f, &inv), Some(base));
}
