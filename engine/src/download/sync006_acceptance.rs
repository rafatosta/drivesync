//! SYNC-006 location/content oracles with fake Drive and real files/SQLite.
use super::*;
use crate::{
    state::{CheckpointScope, Reconciliation, Reconciliations},
    ChangeKind, DifferenceKind, DifferenceReason, DownloadPlan,
};
fn prepare_record(f: &Fixture, inv: &RemoteInventory, source: &mut Fake) -> Reconciliation {
    let journal = Store::open(&f.db()).unwrap();
    let bases = journal
        .confirmed_bases("client", &inv.account_id, &inv.root_id, &f.root)
        .unwrap();
    let root = Root::open(&f.root).unwrap();
    let local = crate::local::scan(&root, &Cancellation::new()).unwrap();
    let plan = crate::changes::prepare(inv, &f.root, &bases, &local, source, &Cancellation::new())
        .unwrap();
    let history = journal
        .operations("client", &plan)
        .unwrap()
        .into_iter()
        .map(|op| crate::changes::Notice {
            id: op.item.id,
            path: Some(op.relative),
        })
        .collect();
    Reconciliation::new(local, inv.clone(), plan, bases, history, None)
}
fn execute(
    f: &Fixture,
    inv: &RemoteInventory,
    plan: &DownloadPlan,
    source: &mut Fake,
) -> DownloadReport {
    let mut journal = Store::open(&f.db()).unwrap();
    journal
        .save("client", inv, plan, &Cancellation::new())
        .unwrap();
    run(
        Context {
            client: "client",
            scope: plan,
            store: &journal,
            cancel: &Cancellation::new(),
        },
        inv,
        source,
        |_| {},
    )
    .unwrap()
}
fn with_folder() -> RemoteInventory {
    let mut inv = inventory();
    inv.entries.push(InventoryEntry {
        id: "folder".into(),
        parent_id: "root".into(),
        name: "folder".into(),
        mime_type: "application/vnd.google-apps.folder".into(),
        version: Some(1),
        size: None,
        md5_checksum: None,
        web_view_link: None,
        kind: InventoryKind::Folder,
    });
    inv
}
fn remote_bytes(inv: &mut RemoteInventory, bytes: &[u8]) -> Fake {
    inv.entries[0].version = Some(2);
    inv.entries[0].size = Some(bytes.len() as u64);
    inv.entries[0].md5_checksum = Some(format!("{:x}", md5::Md5::digest(bytes)));
    let mut remote = Fake::new(inv);
    remote.content.insert("file".into(), bytes.to_vec());
    remote
}
#[test]
fn c09_c12_classify_both_directions_persist_and_repeat_without_duplicate_copies() {
    for moved in [false, true] {
        for local_location in [false, true] {
            let f = Fixture::new();
            let mut inv = with_folder();
            baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
            let base = baseline_tests::base(&f, &inv).unwrap();
            let destination = if moved {
                "folder/file.txt"
            } else {
                "renamed.txt"
            };
            let local_path = if local_location {
                fs::rename(f.root.join("file.txt"), f.root.join(destination)).unwrap();
                destination
            } else {
                fs::write(f.root.join("file.txt"), b"local edit").unwrap();
                if moved {
                    inv.entries[0].parent_id = "folder".into();
                } else {
                    inv.entries[0].name = "renamed.txt".into();
                }
                "file.txt"
            };
            let bytes: &[u8] = if local_location {
                b"remote edit"
            } else {
                b"abc"
            };
            let mut remote = remote_bytes(&mut inv, bytes);
            let scope = CheckpointScope {
                client: "client",
                account: &inv.account_id,
                remote: "root",
                local: &f.root,
            };
            let observation_path = f.db().with_file_name("reconciliation.sqlite3");
            for _ in 0..2 {
                let record = prepare_record(&f, &inv, &mut remote);
                let action = &record.plan.entries[0].action;
                let PlanAction::Classified {
                    decision,
                    local_path: observed_path,
                } = action
                else {
                    panic!("{action:?}")
                };
                assert_eq!(decision.kind, DifferenceKind::Conciliable);
                assert_eq!(decision.reason, DifferenceReason::IndependentChanges);
                assert_eq!(observed_path, &PathBuf::from(local_path));
                assert!(record
                    .entries
                    .iter()
                    .any(|e| e.remote_id.as_deref() == Some("file")
                        && e.kind == ChangeKind::Conciliable));
                assert!(!record
                    .entries
                    .iter()
                    .any(|e| e.kind == ChangeKind::LocalNew));
                let mut store = Reconciliations::open(&observation_path).unwrap();
                let previous = store.load(&scope).unwrap();
                store
                    .save(
                        &scope,
                        previous.as_ref().map(|(r, _)| r.as_str()),
                        &record,
                        &Cancellation::new(),
                    )
                    .unwrap();
                drop(store);
                assert_eq!(
                    Reconciliations::open(&observation_path)
                        .unwrap()
                        .load(&scope)
                        .unwrap()
                        .unwrap()
                        .1,
                    record
                );
                let report = execute(&f, &inv, &record.plan, &mut remote);
                assert_eq!(report.downloaded + report.updated + report.unchanged, 0);
                assert_eq!(report.issues.len(), 1);
                assert_eq!(baseline_tests::base(&f, &inv), Some(base.clone()));
                let expected: &[u8] = if local_location {
                    b"abc"
                } else {
                    b"local edit"
                };
                assert_eq!(fs::read(f.root.join(local_path)).unwrap(), expected);
                assert_eq!(remote.content["file"], bytes);
                let missing = if local_location {
                    "file.txt"
                } else {
                    destination
                };
                assert!(!f.root.join(missing).exists());
            }
        }
    }
}
#[test]
fn location_content_proof_offline_or_stale_remains_unknown_and_preserves_base() {
    for stale in [false, true] {
        let f = Fixture::new();
        let mut inv = inventory();
        baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
        let base = baseline_tests::base(&f, &inv).unwrap();
        fs::write(f.root.join("file.txt"), b"local edit").unwrap();
        inv.entries[0].name = "renamed.txt".into();
        let mut remote = remote_bytes(&mut inv, b"abc");
        remote.fail_stream = !stale;
        remote.changed = stale;
        let record = prepare_record(&f, &inv, &mut remote);
        assert_eq!(record.plan.entries[0].action, PlanAction::DeferredUnknown);
        let report = execute(&f, &inv, &record.plan, &mut remote);
        assert_eq!(report.downloaded + report.updated, 0);
        assert_eq!(baseline_tests::base(&f, &inv), Some(base));
        assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"local edit");
        assert!(!f.root.join("renamed.txt").exists());
    }
}
#[test]
fn conflicting_names_or_content_remain_real_conflicts_not_conciliable() {
    for names in [false, true] {
        let f = Fixture::new();
        let mut inv = inventory();
        baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
        if names {
            fs::rename(f.root.join("file.txt"), f.root.join("local.txt")).unwrap();
        } else {
            fs::write(f.root.join("file.txt"), b"local edit").unwrap();
        }
        inv.entries[0].name = "remote.txt".into();
        let mut remote = remote_bytes(&mut inv, if names { b"abc" } else { b"remote edit" });
        let record = prepare_record(&f, &inv, &mut remote);
        let PlanAction::Classified { decision, .. } = &record.plan.entries[0].action else {
            panic!("classification missing")
        };
        assert_eq!(decision.kind, DifferenceKind::Conflict);
        assert_eq!(
            decision.reason,
            if names {
                DifferenceReason::DivergentLocation
            } else {
                DifferenceReason::DivergentContent
            }
        );
        let report = execute(&f, &inv, &record.plan, &mut remote);
        assert_eq!(report.downloaded + report.updated, 0);
        assert!(!f.root.join("remote.txt").exists());
    }
}

#[test]
fn failed_observation_commit_and_cancellation_keep_the_previous_classification() {
    let f = Fixture::new();
    let mut inv = inventory();
    baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
    let base = baseline_tests::base(&f, &inv).unwrap();
    fs::write(f.root.join("file.txt"), b"local edit").unwrap();
    inv.entries[0].name = "renamed.txt".into();
    let mut remote = remote_bytes(&mut inv, b"abc");
    let record = prepare_record(&f, &inv, &mut remote);
    let path = f.db().with_file_name("reconciliation.sqlite3");
    let scope = CheckpointScope {
        client: "client",
        account: &inv.account_id,
        remote: "root",
        local: &f.root,
    };
    let mut observations = Reconciliations::open(&path).unwrap();
    observations
        .save(&scope, None, &record, &Cancellation::new())
        .unwrap();
    let (revision, _) = observations.load(&scope).unwrap().unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_snapshot BEFORE UPDATE ON snapshots BEGIN SELECT RAISE(ABORT, 'injected failure'); END;").unwrap();
    assert_eq!(
        observations.save(&scope, Some(&revision), &record, &Cancellation::new()),
        Err(EngineError::StateStorage)
    );
    conn.execute_batch("DROP TRIGGER fail_snapshot;").unwrap();
    let cancel = Cancellation::new();
    cancel.cancel();
    assert_eq!(
        observations.save(&scope, Some(&revision), &record, &cancel),
        Err(EngineError::Authentication(crate::AuthError::Cancelled))
    );
    drop(observations);
    assert_eq!(
        Reconciliations::open(&path)
            .unwrap()
            .load(&scope)
            .unwrap()
            .unwrap(),
        (revision, record)
    );
    assert_eq!(baseline_tests::base(&f, &inv), Some(base));
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"local edit");
    assert!(!f.root.join("renamed.txt").exists());
}

#[test]
fn c03_unlisted_remote_with_local_edit_is_unavailable_not_confirmed_deletion() {
    let f = Fixture::new();
    let inv = inventory();
    baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
    let base = baseline_tests::base(&f, &inv).unwrap();
    fs::write(f.root.join("file.txt"), b"offline local edit").unwrap();
    let mut unavailable = inv.clone();
    unavailable.entries.clear();
    let mut remote = Fake::new(&unavailable);
    for _ in 0..2 {
        let record = prepare_record(&f, &unavailable, &mut remote);
        assert!(record
            .entries
            .iter()
            .any(|entry| entry.remote_id.as_deref() == Some("file")
                && entry.kind == ChangeKind::RemoteUnavailable));
        let report = execute(&f, &unavailable, &record.plan, &mut remote);
        assert_eq!(report.downloaded + report.updated, 0);
        assert_eq!(
            fs::read(f.root.join("file.txt")).unwrap(),
            b"offline local edit"
        );
        assert_eq!(baseline_tests::base(&f, &inv), Some(base.clone()));
    }
}

#[test]
fn equal_bytes_in_a_different_identity_never_prove_a_local_relocation() {
    let f = Fixture::new();
    let mut inv = inventory();
    baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
    let base = baseline_tests::base(&f, &inv).unwrap();
    // Keep the original inode allocated so reuse cannot make the fixture ambiguous.
    fs::rename(f.root.join("file.txt"), f.base.join("original-held")).unwrap();
    fs::write(f.root.join("renamed.txt"), b"abc").unwrap();
    let mut remote = remote_bytes(&mut inv, b"remote edit");
    let record = prepare_record(&f, &inv, &mut remote);
    assert_eq!(
        record.plan.entries[0].action,
        PlanAction::Blocked(crate::ConflictReason::LocalMissingRemoteChanged)
    );
    assert!(!record
        .entries
        .iter()
        .any(|e| e.kind == ChangeKind::Conciliable));
    assert_eq!(baseline_tests::base(&f, &inv), Some(base));
}

#[test]
fn conciliable_item_does_not_block_independent_downloads() {
    let f = Fixture::new();
    let mut inv = inventory();
    baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
    fs::write(f.root.join("file.txt"), b"local edit").unwrap();
    inv.entries[0].name = "renamed.txt".into();
    let mut remote = remote_bytes(&mut inv, b"abc");
    let mut independent = inv.entries[0].clone();
    independent.id = "independent".into();
    independent.name = "independent.txt".into();
    inv.entries.push(independent.clone());
    remote.entries.insert(independent.id.clone(), independent);
    remote.content.insert("independent".into(), b"abc".to_vec());
    let record = prepare_record(&f, &inv, &mut remote);
    let report = execute(&f, &inv, &record.plan, &mut remote);
    assert_eq!(report.downloaded, 1);
    assert_eq!(report.issues.len(), 1);
    assert_eq!(fs::read(f.root.join("independent.txt")).unwrap(), b"abc");
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"local edit");
    assert!(!f.root.join("renamed.txt").exists());
}
