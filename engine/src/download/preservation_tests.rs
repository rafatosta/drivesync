//! P02/P08 use actual temporary files and SQLite, with a deterministic remote.
use super::*;
use crate::download::Preservation;
use crate::{ConflictReason, DownloadPlan};
use std::path::Path;

fn setup() -> (Fixture, RemoteInventory, Fake, DownloadPlan) {
    let f = Fixture::new();
    let mut inv = inventory();
    baseline_tests::cycle(&f, &inv, &mut Fake::new(&inv)).unwrap();
    fs::write(f.root.join("file.txt"), b"local edit").unwrap();
    inv.entries[0].version = Some(2);
    inv.entries[0].md5_checksum = Some(format!("{:x}", md5::Md5::digest(b"def")));
    let mut source = Fake::new(&inv);
    source.content.insert("file".into(), b"def".to_vec());
    let plan = baseline_tests::prepare(&f, &inv).unwrap();
    assert_eq!(
        plan.entries[0].action,
        PlanAction::Blocked(ConflictReason::ConcurrentChanges)
    );
    (f, inv, source, plan)
}
fn execute(
    f: &Fixture,
    inv: &RemoteInventory,
    source: &mut Fake,
    plan: &DownloadPlan,
) -> Result<DownloadReport, EngineError> {
    let store = Store::open(&f.db()).unwrap();
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
}
fn record(f: &Fixture, inv: &RemoteInventory, plan: &DownloadPlan) -> Preservation {
    let key = crate::state::preservation_key(&inv.entries[0], Path::new("file.txt")).unwrap();
    Store::open(&f.db())
        .unwrap()
        .preservation("client", plan, &key)
        .unwrap()
        .unwrap()
}
fn rewrite(f: &Fixture, record: &Preservation) {
    rusqlite::Connection::open(f.db())
        .unwrap()
        .execute(
            "UPDATE conflict_copies SET record=?1",
            [serde_json::to_string(record).unwrap()],
        )
        .unwrap();
}
fn copy_path(inv: &RemoteInventory) -> PathBuf {
    let key = crate::state::preservation_key(&inv.entries[0], Path::new("file.txt")).unwrap();
    format!("DriveSync backup conflict-{key}.bak").into()
}
fn originals(f: &Fixture, source: &Fake) {
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"local edit");
    assert_eq!(source.content["file"], b"def");
}
#[test]
fn p02_p08_preserve_bind_reopen_repeat_and_retain_each_remote_version() {
    let (f, mut inv, mut source, plan) = setup();
    let base = baseline_tests::base(&f, &inv);
    let first = execute(&f, &inv, &mut source, &plan).unwrap();
    assert_eq!(first.conflict_copies.len(), 1);
    assert_eq!(first.issues.len(), 1);
    assert_eq!(
        first.downloaded + first.updated + first.recovered + first.unchanged,
        0
    );
    let copy = &first.conflict_copies[0];
    assert_eq!(copy.original_path, Path::new("file.txt"));
    assert_eq!(copy.remote_id, "file");
    assert_eq!(copy.remote_version, 2);
    assert_eq!(fs::read(f.root.join(&copy.copy_path)).unwrap(), b"def");
    assert!(first.attention()[0]
        .message
        .contains(copy.copy_path.to_str().unwrap()));
    let calls = source.media_calls;
    source.fail_stream = true;
    let second = execute(&f, &inv, &mut source, &plan).unwrap();
    assert_eq!(second.conflict_copies, first.conflict_copies);
    assert_eq!(source.media_calls, calls);
    originals(&f, &source);
    assert_eq!(baseline_tests::base(&f, &inv), base);
    assert!(crate::local::ignored(&copy.copy_path));
    inv.entries[0].version = Some(3);
    source.entries.insert("file".into(), inv.entries[0].clone());
    source.fail_stream = false;
    let next_plan = baseline_tests::prepare(&f, &inv).unwrap();
    let third = execute(&f, &inv, &mut source, &next_plan).unwrap();
    assert_eq!(third.conflict_copies.len(), 1);
    assert_ne!(third.conflict_copies[0].copy_path, copy.copy_path);
    assert_eq!(fs::read(f.root.join(&copy.copy_path)).unwrap(), b"def");
    assert_eq!(baseline_tests::base(&f, &inv), base);
}
#[test]
fn p02_failure_before_ready_retries_without_exposing_partial_or_losing_origins() {
    for failure in ["offline", "checksum", "revision", "local"] {
        let (f, inv, mut source, plan) = setup();
        let base = baseline_tests::base(&f, &inv);
        match failure {
            "offline" => source.fail_stream = true,
            "checksum" => {
                source.content.insert("file".into(), b"bad".to_vec());
            }
            "revision" => source.changed = true,
            "local" => {
                let path = f.root.join("file.txt");
                source.on_media = Some(Box::new(move || fs::write(&path, b"new edit").unwrap()));
            }
            _ => unreachable!(),
        }
        let failed = execute(&f, &inv, &mut source, &plan).unwrap();
        assert!(failed.conflict_copies.is_empty(), "{failure}");
        assert_eq!(failed.issues.len(), 1);
        assert!(!f.root.join(copy_path(&inv)).exists());
        assert_eq!(baseline_tests::base(&f, &inv), base);
        source.fail_stream = false;
        source.changed = false;
        source.on_media = None;
        source.content.insert("file".into(), b"def".to_vec());
        fs::write(f.root.join("file.txt"), b"local edit").unwrap();
        let retry = execute(&f, &inv, &mut source, &plan).unwrap();
        assert_eq!(retry.conflict_copies.len(), 1, "{failure}");
        originals(&f, &source);
    }
}
#[test]
fn p08_occupied_names_and_concurrent_creations_are_never_replaced() {
    for kind in ["file", "symlink", "directory", "race"] {
        let (f, inv, mut source, plan) = setup();
        let destination = f.root.join(copy_path(&inv));
        match kind {
            "file" => fs::write(&destination, b"other").unwrap(),
            "symlink" => std::os::unix::fs::symlink("file.txt", &destination).unwrap(),
            "directory" => fs::create_dir(&destination).unwrap(),
            "race" => {
                let path = destination.clone();
                source.on_media = Some(Box::new(move || fs::write(&path, b"other").unwrap()));
            }
            _ => unreachable!(),
        }
        for _ in 0..2 {
            let result = execute(&f, &inv, &mut source, &plan).unwrap();
            assert!(result.conflict_copies.is_empty(), "{kind}");
            assert_eq!(result.issues.len(), 1);
            match kind {
                "symlink" => assert!(destination.is_symlink()),
                "directory" => assert!(destination.is_dir()),
                _ => assert_eq!(fs::read(&destination).unwrap(), b"other"),
            }
            originals(&f, &source);
        }
    }
}
#[test]
fn p02_replay_before_and_after_rename_is_idempotent_and_checks_identity() {
    for installed in [false, true] {
        let (f, inv, mut source, plan) = setup();
        execute(&f, &inv, &mut source, &plan).unwrap();
        let mut saved = record(&f, &inv, &plan);
        let Phase::Installed { identity, sha256 } = saved.operation.phase.clone() else {
            panic!()
        };
        saved.operation.phase = Phase::Ready { identity, sha256 };
        if !installed {
            fs::rename(
                f.root.join(&saved.operation.relative),
                f.root.join(&saved.operation.temporary),
            )
            .unwrap();
        }
        rewrite(&f, &saved);
        source.fail_stream = true;
        let calls = source.media_calls;
        for _ in 0..2 {
            assert_eq!(
                execute(&f, &inv, &mut source, &plan)
                    .unwrap()
                    .conflict_copies
                    .len(),
                1
            );
            assert_eq!(source.media_calls, calls);
            originals(&f, &source);
        }
        let path = f.root.join(&saved.operation.relative);
        fs::rename(&path, f.root.join("saved-copy")).unwrap();
        fs::write(&path, b"def").unwrap();
        assert!(execute(&f, &inv, &mut source, &plan)
            .unwrap()
            .conflict_copies
            .is_empty());
        assert_eq!(fs::read(&path).unwrap(), b"def");
        assert_eq!(fs::read(f.root.join("saved-copy")).unwrap(), b"def");
    }
}
#[test]
fn p02_sqlite_failure_after_publication_recovers_without_redownload() {
    let (f, inv, mut source, plan) = setup();
    let conn = rusqlite::Connection::open(f.db()).unwrap();
    conn.execute_batch("CREATE TRIGGER stop_confirmation BEFORE UPDATE ON conflict_copies WHEN json_type(NEW.record,'$.operation.phase.Installed') IS NOT NULL BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert!(matches!(
        execute(&f, &inv, &mut source, &plan),
        Err(EngineError::StateStorage)
    ));
    assert_eq!(fs::read(f.root.join(copy_path(&inv))).unwrap(), b"def");
    assert!(matches!(
        record(&f, &inv, &plan).operation.phase,
        Phase::Ready { .. }
    ));
    conn.execute_batch("DROP TRIGGER stop_confirmation")
        .unwrap();
    source.fail_stream = true;
    let calls = source.media_calls;
    assert_eq!(
        execute(&f, &inv, &mut source, &plan)
            .unwrap()
            .conflict_copies
            .len(),
        1
    );
    assert_eq!(source.media_calls, calls);
    originals(&f, &source);
}
#[test]
fn p08_scopes_do_not_adopt_each_others_copy_or_advance_baselines() {
    let (f, inv, mut source, plan) = setup();
    execute(&f, &inv, &mut source, &plan).unwrap();
    let key = crate::state::preservation_key(&inv.entries[0], Path::new("file.txt")).unwrap();
    let store = Store::open(&f.db()).unwrap();
    assert!(store
        .preservation("other-client", &plan, &key)
        .unwrap()
        .is_none());
    let mut other = plan.clone();
    other.account_id = AccountId("other-account".into());
    assert!(store
        .preservation("client", &other, &key)
        .unwrap()
        .is_none());
    other = plan.clone();
    other.remote_root = "other-root".into();
    assert!(store
        .preservation("client", &other, &key)
        .unwrap()
        .is_none());
    other = plan.clone();
    other.local_root = f.base.join("other-local");
    assert!(store
        .preservation("client", &other, &key)
        .unwrap()
        .is_none());
    assert_eq!(store.operations("client", &plan).unwrap().len(), 1);
}

#[test]
fn p02_sqlite_intent_and_ready_failures_never_publish_unjournaled_copies() {
    for boundary in ["intent", "ready"] {
        let (f, inv, mut source, plan) = setup();
        let conn = rusqlite::Connection::open(f.db()).unwrap();
        let trigger = if boundary == "intent" {
            "CREATE TRIGGER stop_write BEFORE INSERT ON conflict_copies BEGIN SELECT RAISE(ABORT,'injected'); END;"
        } else {
            "CREATE TRIGGER stop_write BEFORE UPDATE ON conflict_copies WHEN json_type(NEW.record,'$.operation.phase.Ready') IS NOT NULL BEGIN SELECT RAISE(ABORT,'injected'); END;"
        };
        conn.execute_batch(trigger).unwrap();
        assert!(matches!(
            execute(&f, &inv, &mut source, &plan),
            Err(EngineError::StateStorage)
        ));
        assert!(!f.root.join(copy_path(&inv)).exists());
        originals(&f, &source);
        conn.execute_batch("DROP TRIGGER stop_write").unwrap();
        assert_eq!(
            execute(&f, &inv, &mut source, &plan)
                .unwrap()
                .conflict_copies
                .len(),
            1
        );
    }
}
#[test]
fn p02_cancellation_during_transfer_and_root_lock_preserve_originals() {
    let (f, inv, mut source, plan) = setup();
    let store = Store::open(&f.db()).unwrap();
    let cancel = Cancellation::new();
    let signal = cancel.clone();
    source.on_media = Some(Box::new(move || signal.cancel()));
    assert!(matches!(
        run(
            Context {
                client: "client",
                scope: &plan,
                store: &store,
                cancel: &cancel
            },
            &inv,
            &mut source,
            |_| {}
        ),
        Err(EngineError::Authentication(crate::AuthError::Cancelled))
    ));
    assert!(!f.root.join(copy_path(&inv)).exists());
    originals(&f, &source);
    source.on_media = None;
    let locked = Root::open(&f.root).unwrap();
    assert!(matches!(
        execute(&f, &inv, &mut source, &plan),
        Err(EngineError::SyncBusy)
    ));
    drop(locked);
    assert_eq!(
        execute(&f, &inv, &mut source, &plan)
            .unwrap()
            .conflict_copies
            .len(),
        1
    );
}
#[test]
fn p02_corrupt_staging_and_missing_or_changed_copies_fail_explicitly() {
    for damage in ["staging", "missing", "edited"] {
        let (f, inv, mut source, plan) = setup();
        execute(&f, &inv, &mut source, &plan).unwrap();
        let mut saved = record(&f, &inv, &plan);
        let path = f.root.join(&saved.operation.relative);
        match damage {
            "staging" => {
                let Phase::Installed { identity, sha256 } = saved.operation.phase.clone() else {
                    panic!()
                };
                saved.operation.phase = Phase::Ready { identity, sha256 };
                let stage = f.root.join(&saved.operation.temporary);
                fs::rename(&path, &stage).unwrap();
                fs::write(&stage, b"bad").unwrap();
                rewrite(&f, &saved);
            }
            "missing" => fs::remove_file(&path).unwrap(),
            "edited" => fs::write(&path, b"edited copy").unwrap(),
            _ => unreachable!(),
        }
        let calls = source.media_calls;
        let result = execute(&f, &inv, &mut source, &plan).unwrap();
        assert!(result.conflict_copies.is_empty());
        assert_eq!(result.issues.len(), 1);
        assert_eq!(source.media_calls, calls);
        originals(&f, &source);
        if damage == "edited" {
            assert_eq!(fs::read(&path).unwrap(), b"edited copy");
        }
    }
}
#[test]
fn p02_old_database_format_is_rejected_without_reset() {
    let (f, _, _, _) = setup();
    let conn = rusqlite::Connection::open(f.db()).unwrap();
    conn.pragma_update(None, "user_version", 2).unwrap();
    assert!(matches!(
        Store::open(&f.db()),
        Err(EngineError::StateStorage)
    ));
    let count: u64 = conn
        .query_row("SELECT count(*) FROM downloads", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"local edit");
}

#[test]
fn p02_conflict_preservation_does_not_block_independent_downloads() {
    let (f, mut inv, mut source, _) = setup();
    let mut other = inv.entries[0].clone();
    other.id = "independent".into();
    other.name = "independent.txt".into();
    source.entries.insert(other.id.clone(), other.clone());
    source.content.insert(other.id.clone(), b"def".to_vec());
    inv.entries.push(other);
    let plan = baseline_tests::prepare(&f, &inv).unwrap();
    let result = execute(&f, &inv, &mut source, &plan).unwrap();
    assert_eq!(result.conflict_copies.len(), 1);
    assert_eq!(result.downloaded, 1);
    assert_eq!(result.issues.len(), 1);
    assert_eq!(fs::read(f.root.join("independent.txt")).unwrap(), b"def");
    originals(&f, &source);
}
