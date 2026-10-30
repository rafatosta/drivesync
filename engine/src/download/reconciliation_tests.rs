//! C06/C21/N10 acceptance with current scans, re-opened journals and fake Drive.
use super::*;
use crate::{ChangeKind, ConflictReason, DownloadPlan};
fn prepare(f: &Fixture, inv: &RemoteInventory, source: &mut Fake) -> DownloadPlan {
    let store = Store::open(&f.db()).unwrap();
    let bases = store
        .confirmed_bases("client", &inv.account_id, &inv.root_id, &f.root)
        .unwrap();
    let root = Root::open(&f.root).unwrap();
    let local = crate::local::scan(&root, &Cancellation::new()).unwrap();
    crate::changes::prepare(inv, &f.root, &bases, &local, source, &Cancellation::new()).unwrap()
}
fn execute_plan(
    f: &Fixture,
    inv: &RemoteInventory,
    plan: &DownloadPlan,
    source: &mut Fake,
) -> DownloadReport {
    let mut store = Store::open(&f.db()).unwrap();
    store
        .save("client", inv, plan, &Cancellation::new())
        .unwrap();
    run(
        Context {
            client: "client",
            scope: plan,
            store: &store,
            cancel: &Cancellation::new(),
        },
        inv,
        source,
        |_| {},
    )
    .unwrap()
}
fn v2(inv: &RemoteInventory, bytes: &[u8]) -> (RemoteInventory, Fake) {
    let mut inv = inv.clone();
    inv.entries[0].version = Some(2);
    inv.entries[0].size = Some(bytes.len() as u64);
    inv.entries[0].md5_checksum = Some(format!("{:x}", md5::Md5::digest(bytes)));
    let mut remote = Fake::new(&inv);
    remote.content.insert("file".into(), bytes.to_vec());
    (inv, remote)
}
#[test]
fn c06_reopened_offline_local_remote_and_divergent_changes_preserve_bases() {
    for scenario in ["local", "remote", "both"] {
        let f = Fixture::new();
        let inv = inventory();
        baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
        let base = baseline_tests::base(&f, &inv).unwrap();
        if scenario != "remote" {
            fs::write(f.root.join("file.txt"), b"local").unwrap();
        }
        let (next, mut remote) = if scenario == "local" {
            (inv.clone(), Fake::new(&inv))
        } else {
            v2(&inv, b"remote")
        };
        let plan = prepare(&f, &next, &mut remote);
        let expected = match scenario {
            "local" => PlanAction::Blocked(ConflictReason::LocalChanged),
            "remote" => PlanAction::Download,
            _ => PlanAction::Blocked(ConflictReason::ConcurrentChanges),
        };
        assert_eq!(plan.entries[0].action, expected);
        let report = execute_plan(&f, &next, &plan, &mut remote);
        if scenario == "remote" {
            assert_eq!(report.updated, 1);
            assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"remote");
            assert_ne!(baseline_tests::base(&f, &next).unwrap(), base);
        } else {
            assert_eq!(report.issues.len(), 1);
            assert_eq!(baseline_tests::base(&f, &next), Some(base));
            assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"local");
            assert_eq!(prepare(&f, &next, &mut remote).entries[0].action, expected);
        }
    }
}
#[test]
fn c21_equivalent_concurrent_bytes_are_read_only_and_idempotent_after_reopen() {
    let f = Fixture::new();
    let inv = inventory();
    baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
    let base = baseline_tests::base(&f, &inv).unwrap();
    fs::write(f.root.join("file.txt"), b"same edit").unwrap();
    let (next, mut remote) = v2(&inv, b"same edit");
    for _ in 0..2 {
        let plan = prepare(&f, &next, &mut remote);
        assert_eq!(plan.entries[0].action, PlanAction::Equivalent);
        let report = execute_plan(&f, &next, &plan, &mut remote);
        assert!(report.issues.is_empty());
        assert_eq!(report.unchanged, 1);
        assert_eq!(baseline_tests::base(&f, &next), Some(base.clone()));
        assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"same edit");
    }
    assert_eq!(fs::read_dir(&f.root).unwrap().count(), 1);
}
#[test]
fn c21_timestamp_only_changes_do_not_create_conflicts_or_adopt_replacements() {
    let f = Fixture::new();
    let inv = inventory();
    baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
    fs::File::open(f.root.join("file.txt"))
        .unwrap()
        .set_times(
            fs::FileTimes::new().set_modified(
                std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(42),
            ),
        )
        .unwrap();
    let mut remote = Fake::new(&inv);
    assert_eq!(
        prepare(&f, &inv, &mut remote).entries[0].action,
        PlanAction::NoOp
    );
    let (next, mut remote) = v2(&inv, b"abc");
    let plan = prepare(&f, &next, &mut remote);
    assert_eq!(plan.entries[0].action, PlanAction::Download);
    assert!(execute_plan(&f, &next, &plan, &mut remote)
        .issues
        .is_empty());
    fs::write(f.root.join("replacement"), b"abc").unwrap();
    fs::rename(f.root.join("replacement"), f.root.join("file.txt")).unwrap();
    let before = remote.media_calls;
    assert_eq!(
        prepare(&f, &next, &mut remote).entries[0].action,
        PlanAction::Blocked(ConflictReason::LocalIdentityChanged)
    );
    assert_eq!(remote.media_calls, before);
    let fresh = Fixture::new();
    fs::write(fresh.root.join("file.txt"), b"abc").unwrap();
    assert_eq!(
        prepare(&fresh, &next, &mut remote).entries[0].action,
        PlanAction::Blocked(ConflictReason::ExistingLocalData)
    );
}
#[test]
fn equivalent_plan_revalidates_remote_and_local_before_reporting_success() {
    for failure in ["remote", "local", "stream", "changed_before_execution"] {
        let f = Fixture::new();
        let inv = inventory();
        baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
        let base = baseline_tests::base(&f, &inv).unwrap();
        fs::write(f.root.join("file.txt"), b"same edit").unwrap();
        let (next, mut remote) = v2(&inv, b"same edit");
        let plan = prepare(&f, &next, &mut remote);
        match failure {
            "remote" => remote.changed = true,
            "local" => {
                let path = f.root.join("file.txt");
                remote.on_media = Some(Box::new(move || fs::write(&path, b"concurrent").unwrap()));
            }
            "stream" => remote.fail_stream = true,
            _ => {
                let path = f.root.join("file.txt");
                fs::write(path, b"cancelled candidate").unwrap();
            }
        }
        let report = execute_plan(&f, &next, &plan, &mut remote);
        assert_eq!(report.unchanged, 0);
        assert_eq!(report.issues.len(), 1);
        assert_eq!(baseline_tests::base(&f, &next), Some(base));
    }
}
#[test]
fn incomplete_remote_equivalence_proof_is_unknown_without_media_installation() {
    let f = Fixture::new();
    let inv = inventory();
    baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
    let base = baseline_tests::base(&f, &inv).unwrap();
    fs::write(f.root.join("file.txt"), b"same edit").unwrap();
    let (next, mut remote) = v2(&inv, b"same edit");
    remote.fail_stream = true;
    let plan = prepare(&f, &next, &mut remote);
    assert_eq!(plan.entries[0].action, PlanAction::DeferredUnknown);
    let root = Root::open(&f.root).unwrap();
    let local = crate::local::scan(&root, &Cancellation::new()).unwrap();
    drop(root);
    assert_eq!(
        crate::changes::classify(&local, &plan, &[])[0].kind,
        ChangeKind::Unknown
    );
    let report = execute_plan(&f, &next, &plan, &mut remote);
    assert_eq!(report.unchanged + report.updated + report.downloaded, 0);
    assert_eq!(baseline_tests::base(&f, &next), Some(base));
}

#[test]
fn cancellation_during_equivalence_proof_never_publishes_a_plan() {
    let f = Fixture::new();
    let inv = inventory();
    baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
    let base = baseline_tests::base(&f, &inv).unwrap();
    fs::write(f.root.join("file.txt"), b"same edit").unwrap();
    let (next, mut remote) = v2(&inv, b"same edit");
    let root = Root::open(&f.root).unwrap();
    let cancel = Cancellation::new();
    let local = crate::local::scan(&root, &cancel).unwrap();
    let bases = Store::open(&f.db())
        .unwrap()
        .confirmed_bases("client", &inv.account_id, &inv.root_id, &f.root)
        .unwrap();
    let cancellation = cancel.clone();
    remote.on_media = Some(Box::new(move || cancellation.cancel()));
    assert_eq!(
        crate::changes::prepare(&next, &f.root, &bases, &local, &mut remote, &cancel),
        Err(EngineError::Authentication(crate::AuthError::Cancelled))
    );
    assert_eq!(baseline_tests::base(&f, &inv), Some(base));
}

#[test]
fn unknown_remote_proof_never_authorizes_download_after_local_reversion() {
    let f = Fixture::new();
    let inv = inventory();
    baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
    let base = baseline_tests::base(&f, &inv).unwrap();
    fs::write(f.root.join("file.txt"), b"same edit").unwrap();
    let (next, mut remote) = v2(&inv, b"same edit");
    remote.fail_stream = true;
    let plan = prepare(&f, &next, &mut remote);
    assert_eq!(plan.entries[0].action, PlanAction::DeferredUnknown);
    remote.fail_stream = false;
    fs::write(f.root.join("file.txt"), b"abc").unwrap();
    let calls = remote.media_calls;
    let report = execute_plan(&f, &next, &plan, &mut remote);
    assert_eq!(report.updated + report.downloaded + report.unchanged, 0);
    assert_eq!(report.issues.len(), 1);
    assert_eq!(remote.media_calls, calls);
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"abc");
    assert_eq!(baseline_tests::base(&f, &inv), Some(base));
}

#[test]
fn root_suspension_during_equivalence_is_fatal_instead_of_an_item_unknown() {
    struct Suspended;
    impl Source for Suspended {
        fn configured(&mut self) -> Result<(), EngineError> {
            Err(EngineError::RootSuspended(
                crate::RootSuspension::RemoteAccessDenied,
            ))
        }
        fn metadata(&mut self, _: &InventoryEntry) -> Result<InventoryEntry, EngineError> {
            panic!("suspended root cannot query item metadata")
        }
        fn media(&mut self, _: &str) -> Result<Box<dyn Read>, EngineError> {
            panic!("suspended root cannot read media")
        }
    }
    let f = Fixture::new();
    let inv = inventory();
    baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
    let confirmed = baseline_tests::base(&f, &inv).unwrap();
    fs::write(f.root.join("file.txt"), b"equivalent").unwrap();
    let (next, _) = v2(&inv, b"equivalent");
    let store = Store::open(&f.db()).unwrap();
    let bases = store
        .confirmed_bases("client", &inv.account_id, &inv.root_id, &f.root)
        .unwrap();
    let root = Root::open(&f.root).unwrap();
    let local = crate::local::scan(&root, &Cancellation::new()).unwrap();
    let result = crate::changes::prepare(
        &next,
        &f.root,
        &bases,
        &local,
        &mut Suspended,
        &Cancellation::new(),
    );
    assert_eq!(
        result,
        Err(EngineError::RootSuspended(
            crate::RootSuspension::RemoteAccessDenied
        ))
    );
    assert_eq!(baseline_tests::base(&f, &inv), Some(confirmed));
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"equivalent");
}

#[test]
fn c04_missing_local_and_new_remote_revision_require_review_after_reopen() {
    let f = Fixture::new();
    let inv = inventory();
    baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
    let base = baseline_tests::base(&f, &inv).unwrap();
    fs::remove_file(f.root.join("file.txt")).unwrap();
    let (next, mut remote) = v2(&inv, b"remote edit");
    for _ in 0..2 {
        let plan = prepare(&f, &next, &mut remote);
        assert_eq!(
            plan.entries[0].action,
            PlanAction::Blocked(ConflictReason::LocalMissingRemoteChanged)
        );
        let report = execute_plan(&f, &next, &plan, &mut remote);
        assert_eq!(report.downloaded + report.updated, 0);
        assert_eq!(report.issues[0].error, EngineError::DownloadConflict);
        assert!(!f.root.join("file.txt").exists());
        assert_eq!(baseline_tests::base(&f, &next), Some(base.clone()));
        assert_eq!(remote.content["file"], b"remote edit");
    }
    assert_eq!(remote.media_calls, 0);
}

#[test]
fn local_removal_after_remote_update_planning_invalidates_the_candidate() {
    let f = Fixture::new();
    let inv = inventory();
    baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
    let base = baseline_tests::base(&f, &inv).unwrap();
    let (next, mut remote) = v2(&inv, b"remote edit");
    let plan = prepare(&f, &next, &mut remote);
    assert_eq!(plan.entries[0].action, PlanAction::Download);
    fs::remove_file(f.root.join("file.txt")).unwrap();
    let report = execute_plan(&f, &next, &plan, &mut remote);
    assert_eq!(report.issues[0].error, EngineError::DownloadConflict);
    assert_eq!(report.downloaded + report.updated, 0);
    assert!(!f.root.join("file.txt").exists());
    assert_eq!(baseline_tests::base(&f, &next), Some(base));
    assert_eq!(remote.media_calls, 0);
}

#[test]
fn metadata_only_remote_revision_does_not_turn_local_edit_into_content_conflict() {
    let f = Fixture::new();
    let inv = inventory();
    baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
    let base = baseline_tests::base(&f, &inv).unwrap();
    fs::write(f.root.join("file.txt"), b"local edit").unwrap();
    let (next, mut remote) = v2(&inv, b"abc");
    for _ in 0..2 {
        let plan = prepare(&f, &next, &mut remote);
        assert_eq!(
            plan.entries[0].action,
            PlanAction::Blocked(ConflictReason::LocalChanged)
        );
        let root = Root::open(&f.root).unwrap();
        let local = crate::local::scan(&root, &Cancellation::new()).unwrap();
        assert_eq!(
            crate::changes::classify(&local, &plan, &[])[0].kind,
            ChangeKind::LocalChanged
        );
        drop(root);
        let report = execute_plan(&f, &next, &plan, &mut remote);
        assert_eq!(report.downloaded + report.updated, 0);
        assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"local edit");
        assert_eq!(baseline_tests::base(&f, &next), Some(base.clone()));
    }
}
