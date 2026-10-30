//! I25-I28: real journal failure boundaries, verified replay and stale events.
use super::baseline_tests::{base, cycle, prepare};
use super::concurrency_tests::{execute_plan, fake, files, installed, journal, remote, V1, V2};
use super::*;
use crate::{
    changes,
    remote::{Change, ChangePage},
    state::{CheckpointScope, Checkpoints},
};

/// Rejects only final confirmation, after the actual filesystem effect. Every
/// connection closes before replay; no successful journal is rewritten as pending.
fn unconfirmed_effect(f: &Fixture, inv: &RemoteInventory, source: &mut Fake) -> Operation {
    let plan = prepare(f, inv).unwrap();
    let conn = rusqlite::Connection::open(f.db()).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_confirm BEFORE UPDATE ON downloads WHEN json_extract(NEW.record,'$.phase.Installed') IS NOT NULL OR json_extract(NEW.record,'$.phase.Updated') IS NOT NULL BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert_eq!(
        execute_plan(f, inv, &plan, source, &Cancellation::new(), |_| {}).unwrap_err(),
        EngineError::StateStorage
    );
    conn.execute_batch("DROP TRIGGER fail_confirm").unwrap();
    drop(conn);
    assert!(base(f, inv).is_none());
    let op = journal(f, inv).pop().unwrap();
    assert!(matches!(
        op.phase,
        Phase::Ready { .. } | Phase::UpdateReady { .. }
    ));
    op
}

#[test]
fn i26_actual_effect_without_confirmation_replays_only_the_verified_identity() {
    for update in [false, true] {
        for substitution in 0..3 {
            let (f, _) = if update {
                installed()
            } else {
                (Fixture::new(), remote(V1, 1))
            };
            let next = remote(V2, 2);
            let mut source = fake(&next, V2);
            let op = unconfirmed_effect(&f, &next, &mut source);
            let expected = match &op.phase {
                Phase::Ready { identity, .. } | Phase::UpdateReady { identity, .. } => *identity,
                _ => unreachable!(),
            };
            let path = f.root.join("dados.db");
            let effect = fs::metadata(&path).unwrap();
            assert_eq!(
                (effect.dev(), effect.ino()),
                (expected.device, expected.inode)
            );
            assert_eq!(fs::read(&path).unwrap(), V2);
            if substitution != 0 {
                fs::rename(&path, f.root.join("retained-effect")).unwrap();
                fs::write(
                    &path,
                    if substitution == 1 {
                        V2
                    } else {
                        &b"USER-DATA"[..]
                    },
                )
                .unwrap();
                let substitute = fs::metadata(&path).unwrap();
                assert_ne!(
                    (substitute.dev(), substitute.ino()),
                    (expected.device, expected.inode)
                );
            }
            let retained = files(&f);
            let pending = serde_json::to_value(journal(&f, &next)).unwrap();
            for iteration in 0..2 {
                let report = cycle(&f, &next, &mut source).unwrap();
                if substitution == 0 {
                    assert!(report.issues.is_empty());
                    assert_eq!(report.recovered, usize::from(iteration == 0));
                    assert_eq!(report.unchanged, usize::from(iteration != 0));
                    let confirmed = base(&f, &next).unwrap();
                    assert_eq!(confirmed.remote_version, 2);
                    assert_eq!(
                        Some(confirmed.local_fingerprint),
                        filesystem::fingerprint(
                            &f.root,
                            std::path::Path::new("dados.db"),
                            &Cancellation::new()
                        )
                        .unwrap()
                    );
                } else {
                    assert_eq!(report.issues[0].error, EngineError::DownloadConflict);
                    assert_eq!(
                        (report.recovered, report.updated, report.downloaded),
                        (0, 0, 0)
                    );
                    assert!(base(&f, &next).is_none());
                    assert_eq!(serde_json::to_value(journal(&f, &next)).unwrap(), pending);
                    assert_eq!(fs::read(f.root.join("retained-effect")).unwrap(), V2);
                }
                assert_eq!(files(&f), retained);
                assert_eq!(source.media_ids, ["file"]);
                if update {
                    let Phase::UpdateReady { backup, .. } = &op.phase else {
                        unreachable!()
                    };
                    assert_eq!(fs::read(f.root.join(backup)).unwrap(), V1);
                }
            }
        }
    }
}

#[test]
fn i25_incomplete_stream_and_verified_staging_never_confirm_a_base() {
    for update in [false, true] {
        for failure in 0..4 {
            let (f, _) = if update {
                installed()
            } else {
                (Fixture::new(), remote(V1, 1))
            };
            let next = remote(V2, 2);
            let plan = prepare(&f, &next).unwrap();
            if failure == 3 {
                rusqlite::Connection::open(f.db()).unwrap().execute_batch("CREATE TRIGGER fail_ready BEFORE UPDATE ON downloads WHEN json_extract(NEW.record,'$.phase.Ready') IS NOT NULL OR json_extract(NEW.record,'$.phase.UpdateReady') IS NOT NULL BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
            }
            let mut source = fake(&next, V2);
            source.fail_stream = failure == 0;
            let cancel = Cancellation::new();
            let mut cancelled = false;
            let result = execute_plan(&f, &next, &plan, &mut source, &cancel, |p| {
                if p.current.is_some_and(|p| {
                    (failure == 1 && p.stage == DownloadStage::Downloading && p.received > 0)
                        || (failure == 2 && p.stage == DownloadStage::Verifying)
                }) {
                    cancel.cancel();
                    cancelled = true;
                }
            });
            match failure {
                0 => assert_eq!(
                    result.unwrap().issues[0].error,
                    EngineError::Authentication(crate::AuthError::Network)
                ),
                1 | 2 => {
                    assert!(cancelled);
                    assert!(matches!(
                        result,
                        Err(EngineError::Authentication(crate::AuthError::Cancelled))
                    ));
                }
                _ => assert_eq!(result.unwrap_err(), EngineError::StateStorage),
            }
            assert!(base(&f, &next).is_none());
            let operations = journal(&f, &next);
            let last = operations.last().unwrap();
            assert!(matches!(
                last.phase,
                Phase::Started | Phase::Updating { .. }
            ));
            assert_eq!(last.item.id, "file");
            assert_eq!(last.item.version, Some(2));
            assert!(f.root.join(&last.temporary).is_file());
            if failure == 3 {
                assert_eq!(fs::read(f.root.join(&last.temporary)).unwrap(), V2);
            }
            if update {
                assert_eq!(fs::read(f.root.join("dados.db")).unwrap(), V1);
            } else {
                assert!(!f.root.join("dados.db").exists());
            }
            assert!(!fs::read_dir(&f.root).unwrap().any(|p| p
                .unwrap()
                .file_name()
                .to_str()
                .unwrap()
                .ends_with(".bak")));
            rusqlite::Connection::open(f.db())
                .unwrap()
                .execute_batch("DROP TRIGGER IF EXISTS fail_ready")
                .unwrap();
            source.fail_stream = false;
            let report = cycle(&f, &next, &mut source).unwrap();
            assert!(report.issues.is_empty());
            assert_eq!(report.updated, usize::from(update));
            assert_eq!(report.downloaded, usize::from(!update));
            assert_eq!(report.preserved_copies.len(), usize::from(update));
            if update {
                assert_eq!(
                    fs::read(f.root.join(&report.preserved_copies[0])).unwrap(),
                    V1
                );
            }
            assert_eq!(base(&f, &next).unwrap().remote_version, 2);
            assert_eq!(fs::read(f.root.join("dados.db")).unwrap(), V2);
            let retained = files(&f);
            let completed = serde_json::to_value(journal(&f, &next)).unwrap();
            for _ in 0..2 {
                assert_eq!(
                    prepare(&f, &next).unwrap().entries[0].action,
                    PlanAction::NoOp
                );
                assert_eq!(cycle(&f, &next, &mut source).unwrap().unchanged, 1);
                assert_eq!(files(&f), retained);
                assert_eq!(serde_json::to_value(journal(&f, &next)).unwrap(), completed);
                assert_eq!(source.media_ids, ["file", "file"]);
            }
        }
    }
}

#[test]
fn i27_corrupt_incompatible_and_incomplete_state_never_resets_or_authorizes_media() {
    for corruption in 0..5 {
        let (f, inv) = installed();
        match corruption {
            0 => fs::write(f.db(), b"invalid SQLite state").unwrap(),
            1 => rusqlite::Connection::open(f.db())
                .unwrap()
                .pragma_update(None, "user_version", 99)
                .unwrap(),
            _ => {
                let conn = rusqlite::Connection::open(f.db()).unwrap();
                let record: String = conn
                    .query_row("SELECT record FROM downloads", [], |r| r.get(0))
                    .unwrap();
                let mut json: serde_json::Value = serde_json::from_str(&record).unwrap();
                let installed = json["phase"]["Installed"].as_object_mut().unwrap();
                match corruption {
                    2 => {
                        installed
                            .get_mut("identity")
                            .unwrap()
                            .as_object_mut()
                            .unwrap()
                            .remove("inode");
                    }
                    3 => {
                        installed.insert("sha256".into(), serde_json::json!("incomplete"));
                    }
                    _ => {
                        installed["identity"]["inode"] = serde_json::json!(0);
                    }
                }
                conn.execute(
                    "UPDATE downloads SET record=?1",
                    [serde_json::to_string(&json).unwrap()],
                )
                .unwrap();
            }
        }
        let db = fs::read(f.db()).unwrap();
        let retained = files(&f);
        let record = if corruption == 4 {
            Some(serde_json::to_value(journal(&f, &inv)).unwrap())
        } else {
            None
        };
        let mut source = fake(&inv, V1);
        for _ in 0..2 {
            let result = cycle(&f, &inv, &mut source);
            if corruption == 4 {
                assert_eq!(
                    result.unwrap().issues[0].error,
                    EngineError::DownloadConflict
                );
                assert_eq!(
                    prepare(&f, &inv).unwrap().entries[0].action,
                    PlanAction::DeferredUnknown
                );
                assert_eq!(
                    Some(serde_json::to_value(journal(&f, &inv)).unwrap()),
                    record
                );
            } else {
                assert_eq!(result.unwrap_err(), EngineError::StateStorage);
                assert_eq!(fs::read(f.db()).unwrap(), db);
            }
            assert_eq!(files(&f), retained);
            assert!(source.media_ids.is_empty());
            assert!(source.metadata_ids.is_empty());
        }
    }
}

struct Events {
    current: RemoteInventory,
    pending: Vec<Change>,
    sequence: usize,
    scans: usize,
}
impl changes::Provider for Events {
    fn start(&mut self) -> Result<String, EngineError> {
        Ok("start".into())
    }
    fn scan(&mut self) -> Result<RemoteInventory, EngineError> {
        self.scans += 1;
        Ok(self.current.clone())
    }
    fn page(&mut self, _: &str) -> Result<ChangePage, EngineError> {
        self.sequence += 1;
        Ok(ChangePage {
            changes: std::mem::take(&mut self.pending),
            next: None,
            terminal: Some(format!("cursor-{}", self.sequence)),
        })
    }
}
fn checkpoint(f: &Fixture, inv: &RemoteInventory) -> serde_json::Value {
    let scope = CheckpointScope {
        client: "client",
        account: &inv.account_id,
        remote: &inv.root_id,
        local: &f.root,
    };
    serde_json::to_value(
        Checkpoints::open(&f.base.join("private/changes.sqlite3"))
            .unwrap()
            .load(&scope)
            .unwrap(),
    )
    .unwrap()
}
/// Reopens the cursor, journal and plan each cycle. An explicit rescan uses the
/// real reconciliation path; malformed/regressive batches cannot commit a cursor.
fn event_cycle(
    f: &Fixture,
    feed: &mut Events,
    source: &mut Fake,
    rescan: bool,
) -> Result<(DownloadReport, crate::DownloadPlan), EngineError> {
    let account = feed.current.account_id.clone();
    let root = feed.current.root_id.clone();
    let scope = CheckpointScope {
        client: "client",
        account: &account,
        remote: &root,
        local: &f.root,
    };
    let mut store = Checkpoints::open(&f.base.join("private/changes.sqlite3"))?;
    let old = store.load(&scope)?;
    let next = if rescan {
        changes::reconcile(
            &account,
            &root,
            &old.as_ref().unwrap().1,
            &Cancellation::new(),
            feed,
        )?
    } else {
        changes::collect(
            &account,
            &root,
            old.as_ref().map(|(_, c)| c),
            &Cancellation::new(),
            feed,
        )?
    };
    store.save(
        &scope,
        old.as_ref().map(|(revision, _)| revision.as_str()),
        &next,
        &Cancellation::new(),
    )?;
    drop(store);
    let plan = prepare(f, &next.inventory)?;
    let reopened = Store::open(&f.db())?
        .load("client", &account, &root, &f.root)?
        .unwrap();
    assert_eq!(reopened.entries.len(), plan.entries.len());
    for entry in &plan.entries {
        assert_eq!(
            reopened
                .entries
                .iter()
                .find(|e| e.remote_id == entry.remote_id),
            Some(entry)
        );
    }
    Ok((
        execute_plan(
            f,
            &next.inventory,
            &plan,
            source,
            &Cancellation::new(),
            |_| {},
        )?,
        plan,
    ))
}
fn change(item: &InventoryEntry) -> Change {
    Change {
        confirmed_trashed: false,
        id: item.id.clone(),
        removed: false,
        item: Some(item.clone()),
    }
}

#[test]
fn i28_duplicate_and_stale_events_preserve_bindings_after_conflict_history_and_replay() {
    for scenario in 0..5 {
        let (f, first) = installed();
        let mut current = first.clone();
        match scenario {
            0 => {
                fs::rename(f.root.join("dados.db"), f.root.join("user-original")).unwrap();
                fs::write(f.root.join("dados.db"), V1).unwrap();
                let a = fs::metadata(f.root.join("dados.db")).unwrap();
                let b = fs::metadata(f.root.join("user-original")).unwrap();
                assert_ne!((a.dev(), a.ino()), (b.dev(), b.ino()));
            }
            1 => {
                let mut other = remote(V2, 2).entries.remove(0);
                other.id = "other-id".into();
                current.entries.push(other);
            }
            2 => {
                let next = remote(V2, 2);
                assert_eq!(cycle(&f, &next, &mut fake(&next, V2)).unwrap().updated, 1);
                current = remote(V1, 3);
                assert_eq!(
                    cycle(&f, &current, &mut fake(&current, V1))
                        .unwrap()
                        .updated,
                    1
                );
            }
            3 => {
                current.entries[0].id = "other-id".into();
            }
            _ => {
                current = remote(V2, 2);
                unconfirmed_effect(&f, &current, &mut fake(&current, V2));
                fs::rename(f.root.join("dados.db"), f.root.join("retained-effect")).unwrap();
                fs::write(f.root.join("dados.db"), V2).unwrap();
                let a = fs::metadata(f.root.join("dados.db")).unwrap();
                let b = fs::metadata(f.root.join("retained-effect")).unwrap();
                assert_ne!((a.dev(), a.ino()), (b.dev(), b.ino()));
            }
        }
        let mut source = fake(&current, if scenario == 4 { V2 } else { V1 });
        if scenario == 1 {
            source.content.insert("other-id".into(), V2.to_vec());
        }
        let mut feed = Events {
            current: current.clone(),
            pending: vec![],
            sequence: 0,
            scans: 0,
        };
        event_cycle(&f, &mut feed, &mut source, false).unwrap();
        let retained = files(&f);
        let operations = serde_json::to_value(journal(&f, &current)).unwrap();
        let bases = Store::open(&f.db())
            .unwrap()
            .confirmed_bases("client", &current.account_id, &current.root_id, &f.root)
            .unwrap();
        let remote_content = source.content.clone();
        for reverse in [false, true] {
            let latest = change(current.entries.last().unwrap());
            feed.pending = vec![
                change(&first.entries[0]),
                change(&first.entries[0]),
                latest.clone(),
            ];
            if reverse {
                feed.pending.reverse();
            }
            let before = checkpoint(&f, &current);
            let result = event_cycle(&f, &mut feed, &mut source, false);
            if reverse && (scenario == 2 || scenario == 4) {
                assert_eq!(result.unwrap_err(), EngineError::RemoteChanged);
                assert_eq!(checkpoint(&f, &current), before);
            } else {
                let (report, _) = result.unwrap();
                if scenario == 2 {
                    assert_eq!(report.unchanged, 1);
                    assert!(report.issues.is_empty());
                } else {
                    assert!(!report.issues.is_empty());
                    assert!(report
                        .issues
                        .iter()
                        .all(|i| i.error == EngineError::DownloadConflict));
                }
            }
            // A repeated current event must not resolve a local/pending conflict
            // or install another remote ID merely because its bytes/name match.
            feed.pending = vec![latest.clone(), latest];
            let (report, plan) = event_cycle(&f, &mut feed, &mut source, false).unwrap();
            match scenario {
                0 => assert_eq!(
                    plan.entries[0].action,
                    PlanAction::Blocked(crate::ConflictReason::LocalIdentityChanged)
                ),
                1 | 3 => assert!(plan.entries.iter().all(|e| e.action
                    == PlanAction::Blocked(crate::ConflictReason::UnsafeOrAmbiguousPath))),
                2 => {
                    assert_eq!(plan.entries[0].action, PlanAction::NoOp);
                    assert_eq!(report.unchanged, 1);
                }
                _ => assert_eq!(
                    plan.entries[0].action,
                    PlanAction::Blocked(crate::ConflictReason::ExistingLocalData)
                ),
            }
            assert_eq!(files(&f), retained);
            assert_eq!(
                serde_json::to_value(journal(&f, &current)).unwrap(),
                operations
            );
            assert_eq!(
                Store::open(&f.db())
                    .unwrap()
                    .confirmed_bases("client", &current.account_id, &current.root_id, &f.root)
                    .unwrap(),
                bases
            );
            assert_eq!(source.content, remote_content);
            assert!(source.media_ids.is_empty());
        }
        if scenario == 3 {
            // A stale returning ID is ambiguous observation, never a confirmed
            // binding. An authoritative rescan removes that observation safely.
            let (report, plan) = event_cycle(&f, &mut feed, &mut source, true).unwrap();
            assert_eq!(plan.entries.len(), 1);
            assert_eq!(plan.entries[0].remote_id, "other-id");
            assert_eq!(
                plan.entries[0].action,
                PlanAction::Blocked(crate::ConflictReason::ExistingLocalData)
            );
            assert_eq!(report.issues[0].error, EngineError::DownloadConflict);
            assert_eq!(feed.scans, 2);
            assert_eq!(files(&f), retained);
            assert_eq!(
                serde_json::to_value(journal(&f, &current)).unwrap(),
                operations
            );
            assert!(base(&f, &current).is_none());
            assert!(source.media_ids.is_empty());
        }
    }
}

#[test]
fn sync005_rebuilt_state_preserves_preexisting_bytes_and_retries_integrity_failures() {
    let f = Fixture::new();
    let inv = inventory();
    let folder = crate::FolderConfiguration {
        account_id: inv.account_id.clone(),
        remote: crate::DriveFolder {
            id: inv.root_id.clone(),
            name: "Root".into(),
        },
        local: f.root.clone(),
        local_display: f.root.clone(),
    };
    crate::configuration::identity::ensure(&folder, "client").unwrap();
    let original = f.root.join(".drivesync/state.sqlite3");
    fs::write(&original, b"corrupt state").unwrap();
    fs::write(f.root.join("file.txt"), b"USER DATA").unwrap();
    let locked = Root::open(&f.root).unwrap();
    let recovery =
        crate::state::recovery::rebuild(&folder, "client", &Cancellation::new()).unwrap();
    assert_eq!(recovery.rebuilt_stores, 4);
    assert_eq!(Root::open(&f.root).err(), Some(EngineError::SyncBusy));
    drop(locked);
    let path = crate::configuration::identity::state_path(&folder, "client").unwrap();
    let mut source = Fake::new(&inv);
    for _ in 0..2 {
        let mut store = Store::open(&path).unwrap();
        let bases = store
            .confirmed_bases("client", &inv.account_id, &inv.root_id, &f.root)
            .unwrap();
        assert!(bases.is_empty());
        let plan = crate::planner::plan_with_bases(&inv, &f.root, &bases, |_, relative, _| {
            Ok(crate::planner::observe(&f.root, relative))
        })
        .unwrap();
        store
            .save("client", &inv, &plan, &Cancellation::new())
            .unwrap();
        let report = run(
            Context {
                client: "client",
                scope: &plan,
                store: &store,
                cancel: &Cancellation::new(),
            },
            &inv,
            &mut source,
            |_| {},
        )
        .unwrap();
        assert_eq!(report.downloaded, 0);
        assert!(!report.issues.is_empty());
        assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"USER DATA");
        assert_eq!(source.media_calls, 0);
    }
    // An independent absent destination can still be installed, but bad bytes
    // never confirm a base and a later valid retry is idempotent.
    let mut next = inv.clone();
    next.entries[0].name = "new.txt".into();
    source.content.insert("file".into(), b"bad".to_vec());
    for valid in [false, true, true] {
        if valid {
            source.content.insert("file".into(), b"abc".to_vec());
        }
        source.entries = next
            .entries
            .iter()
            .map(|entry| (entry.id.clone(), entry.clone()))
            .collect();
        let mut store = Store::open(&path).unwrap();
        let plan = crate::planner::plan(&next, &f.root, |relative| {
            crate::planner::observe(&f.root, relative)
        })
        .unwrap();
        store
            .save("client", &next, &plan, &Cancellation::new())
            .unwrap();
        let report = run(
            Context {
                client: "client",
                scope: &plan,
                store: &store,
                cancel: &Cancellation::new(),
            },
            &next,
            &mut source,
            |_| {},
        )
        .unwrap();
        if !valid {
            assert!(report
                .issues
                .iter()
                .any(|issue| issue.error == EngineError::DownloadIntegrity));
            assert!(store
                .confirmed_bases("client", &next.account_id, &next.root_id, &f.root)
                .unwrap()
                .is_empty());
            assert!(!f.root.join("new.txt").exists());
        } else {
            assert_eq!(fs::read(f.root.join("new.txt")).unwrap(), b"abc");
        }
    }
    assert_eq!(fs::read(original).unwrap(), b"corrupt state");
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"USER DATA");
}
