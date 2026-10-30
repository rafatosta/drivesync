//! Integrated feed, local scan, independent observation stores and executor oracles.
use super::*;
use crate::{
    changes,
    remote::{Change, ChangePage},
    state::{CheckpointScope, Checkpoints, Reconciliation, Reconciliations},
    ChangeKind,
};
struct Feed {
    inventory: RemoteInventory,
    events: Vec<Change>,
    offline: bool,
}
impl changes::Provider for Feed {
    fn start(&mut self) -> Result<String, EngineError> {
        Ok("start".into())
    }
    fn scan(&mut self) -> Result<RemoteInventory, EngineError> {
        Ok(self.inventory.clone())
    }
    fn page(&mut self, _: &str) -> Result<ChangePage, EngineError> {
        if self.offline {
            return Err(EngineError::Authentication(crate::AuthError::Network));
        }
        Ok(ChangePage {
            changes: std::mem::take(&mut self.events),
            next: None,
            terminal: Some(oauth2::CsrfToken::new_random().secret().clone()),
        })
    }
}
fn observed_cycle(
    f: &Fixture,
    feed: &mut Feed,
    source: &mut Fake,
) -> Result<DownloadReport, EngineError> {
    observed_cycle_reviewed(f, feed, source, None)
}
fn observed_cycle_reviewed(
    f: &Fixture,
    feed: &mut Feed,
    source: &mut Fake,
    approval: Option<&crate::BulkReview>,
) -> Result<DownloadReport, EngineError> {
    let cancel = Cancellation::new();
    let account = feed.inventory.account_id.clone();
    let scope = CheckpointScope {
        client: "client",
        account: &account,
        remote: "root",
        local: &f.root,
    };
    let mut journal = Store::open(&f.db())?;
    let mut checkpoints = Checkpoints::open(&f.db().with_file_name("changes.sqlite3"))?;
    let previous = checkpoints.load(&scope)?;
    let checkpoint = changes::collect(
        &account,
        "root",
        previous.as_ref().map(|(_, record)| record),
        &cancel,
        feed,
    )?;
    let root = Root::open(&f.root)?;
    let local = crate::local::scan(&root, &cancel)?;
    let bases = journal.confirmed_bases("client", &account, "root", &f.root)?;
    let plan = changes::prepare(
        &checkpoint.inventory,
        &root,
        &bases,
        &local,
        source,
        &cancel,
    )?;
    let mut history = checkpoint.notices.clone();
    history.extend(
        journal
            .operations("client", &plan)?
            .into_iter()
            .map(|op| changes::Notice {
                deleted: false,
                id: op.item.id,
                path: Some(op.relative),
            }),
    );
    let mut observations = Reconciliations::open(&f.db().with_file_name("reconciliation.sqlite3"))?;
    let old = observations.load(&scope)?;
    let record = Reconciliation::new(
        local,
        checkpoint.inventory.clone(),
        plan.clone(),
        bases,
        history,
        old.as_ref().map(|(_, record)| record),
    );
    checkpoints.save(
        &scope,
        previous.as_ref().map(|(revision, _)| revision.as_str()),
        &checkpoint,
        &cancel,
    )?;
    journal.save("client", &checkpoint.inventory, &plan, &cancel)?;
    root.validate()?;
    observations.save(
        &scope,
        old.as_ref().map(|(revision, _)| revision.as_str()),
        &record,
        &cancel,
    )?;
    crate::coordinator::bulk_gate("client", &record, approval)?;
    drop(root);
    let mut report = run(
        Context {
            client: "client",
            scope: &plan,
            store: &journal,
            cancel: &cancel,
        },
        &checkpoint.inventory,
        source,
        |_| {},
    )?;
    report.reconciliation = record.entries;
    report.previous_paths = changes::previous_paths(&checkpoint)?;
    Ok(report)
}
#[test]
fn c06_c21_n10_reconnect_after_observation_commit_failure_and_repeat_without_new_events() {
    let f = Fixture::new();
    let inv = inventory();
    let mut feed = Feed {
        inventory: inv.clone(),
        events: vec![],
        offline: false,
    };
    let mut remote = Fake::new(&inv);
    assert_eq!(
        observed_cycle(&f, &mut feed, &mut remote)
            .unwrap()
            .downloaded,
        1
    );
    let baseline = baseline_tests::base(&f, &inv).unwrap();
    fs::write(f.root.join("file.txt"), b"same edit").unwrap();
    fs::write(f.root.join("new.part"), b"partial").unwrap();
    feed.offline = true;
    assert!(matches!(
        observed_cycle(&f, &mut feed, &mut remote),
        Err(EngineError::Authentication(crate::AuthError::Network))
    ));
    feed.offline = false;
    let mut next = inv.clone();
    next.entries[0].version = Some(2);
    next.entries[0].size = Some(9);
    next.entries[0].md5_checksum = Some(format!("{:x}", md5::Md5::digest(b"same edit")));
    remote
        .entries
        .insert("file".into(), next.entries[0].clone());
    remote.content.insert("file".into(), b"same edit".to_vec());
    feed.events.push(Change {
        confirmed_trashed: false,
        id: "file".into(),
        removed: false,
        item: Some(next.entries[0].clone()),
    });
    let account = inv.account_id.clone();
    let scope = CheckpointScope {
        client: "client",
        account: &account,
        remote: "root",
        local: &f.root,
    };
    let observation_path = f.db().with_file_name("reconciliation.sqlite3");
    let old = Reconciliations::open(&observation_path)
        .unwrap()
        .load(&scope)
        .unwrap()
        .unwrap();
    let conn = rusqlite::Connection::open(&observation_path).unwrap();
    conn.execute_batch("CREATE TRIGGER refuse BEFORE UPDATE ON snapshots BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert!(matches!(
        observed_cycle(&f, &mut feed, &mut remote),
        Err(EngineError::StateStorage)
    ));
    assert!(feed.events.is_empty());
    assert_eq!(
        Reconciliations::open(&observation_path)
            .unwrap()
            .load(&scope)
            .unwrap()
            .unwrap(),
        old
    );
    assert_eq!(baseline_tests::base(&f, &next), Some(baseline.clone()));
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"same edit");
    conn.execute_batch("DROP TRIGGER refuse").unwrap();
    drop(conn);
    let mut last = None;
    for _ in 0..2 {
        let report = observed_cycle(&f, &mut feed, &mut remote).unwrap();
        assert_eq!(report.unchanged, 1);
        assert!(report.issues.is_empty());
        assert_eq!(report.attention().len(), 1);
        assert!(report
            .reconciliation
            .iter()
            .any(|e| e.kind == ChangeKind::Equivalent));
        assert!(report
            .reconciliation
            .iter()
            .any(|e| e.kind == ChangeKind::Temporary));
        let loaded = Reconciliations::open(&observation_path)
            .unwrap()
            .load(&scope)
            .unwrap()
            .unwrap()
            .1;
        assert_eq!(loaded.plan.entries[0].action, PlanAction::Equivalent);
        assert_eq!(loaded.bases.get("file"), Some(&baseline));
        if let Some(previous) = last {
            assert_eq!(loaded, previous);
        }
        last = Some(loaded);
    }
    feed.events.push(Change {
        confirmed_trashed: false,
        id: "file".into(),
        removed: true,
        item: None,
    });
    let report = observed_cycle(&f, &mut feed, &mut remote).unwrap();
    assert!(report
        .reconciliation
        .iter()
        .any(|e| e.remote_id.as_deref() == Some("file")
            && e.kind == ChangeKind::LocalChangedRemoteUnavailable));
    assert!(!report
        .reconciliation
        .iter()
        .any(
            |e| e.relative_path.as_deref() == Some(std::path::Path::new("file.txt"))
                && e.kind == ChangeKind::LocalNew
        ));
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"same edit");
    assert_eq!(fs::read(f.root.join("new.part")).unwrap(), b"partial");
    assert_eq!(baseline_tests::base(&f, &inv), Some(baseline));
}

#[test]
fn c03_c16_folder_trash_or_access_loss_preserves_descendants_through_failure_and_restart() {
    for folder in [false, true] {
        for trash in [false, true] {
            let f = Fixture::new();
            let mut inv = inventory();
            let path = if folder {
                let mut directory = inv.entries[0].clone();
                directory.id = "folder".into();
                directory.name = "folder".into();
                directory.kind = InventoryKind::Folder;
                directory.mime_type = "application/vnd.google-apps.folder".into();
                inv.entries[0].parent_id = "folder".into();
                inv.entries.push(directory);
                "folder/file.txt"
            } else {
                "file.txt"
            };
            let mut feed = Feed {
                inventory: inv.clone(),
                events: vec![],
                offline: false,
            };
            let mut remote = Fake::new(&inv);
            assert_eq!(
                observed_cycle(&f, &mut feed, &mut remote)
                    .unwrap()
                    .downloaded,
                1
            );
            let base = baseline_tests::base(&f, &inv).unwrap();
            fs::write(f.root.join(path), b"offline work").unwrap();
            feed.events.push(Change {
                id: if folder { "folder" } else { "file" }.into(),
                removed: true,
                confirmed_trashed: trash,
                item: None,
            });
            feed.offline = true;
            assert!(observed_cycle(&f, &mut feed, &mut remote).is_err());
            assert_eq!(fs::read(f.root.join(path)).unwrap(), b"offline work");
            feed.offline = false;
            // Fail the observation transaction after the feed checkpoint commits.
            // The next cycle must recover the trash/access distinction from SQLite.
            let db = f.db().with_file_name("reconciliation.sqlite3");
            let conn = rusqlite::Connection::open(&db).unwrap();
            conn.execute_batch("CREATE TRIGGER fail_observation BEFORE UPDATE ON snapshots BEGIN SELECT RAISE(ABORT, 'injected'); END;").unwrap();
            assert_eq!(
                observed_cycle(&f, &mut feed, &mut remote).unwrap_err(),
                EngineError::StateStorage
            );
            conn.execute_batch("DROP TRIGGER fail_observation;")
                .unwrap();
            for _ in 0..3 {
                let report = observed_cycle(&f, &mut feed, &mut remote).unwrap();
                let child = report
                    .reconciliation
                    .iter()
                    .find(|e| e.remote_id.as_deref() == Some("file"))
                    .unwrap();
                assert_eq!(
                    child.kind,
                    if trash && !folder {
                        ChangeKind::EditDeleteConflict
                    } else {
                        ChangeKind::LocalChangedRemoteUnavailable
                    }
                );
                if folder {
                    let parent = report
                        .reconciliation
                        .iter()
                        .find(|e| e.remote_id.as_deref() == Some("folder"))
                        .unwrap();
                    assert_eq!(
                        parent.kind,
                        if trash {
                            ChangeKind::EditDeleteConflict
                        } else {
                            ChangeKind::LocalChangedRemoteUnavailable
                        }
                    );
                }
                assert!(!report.attention().is_empty());
                assert_eq!(report.downloaded + report.updated, 0);
                assert!(report.preserved_copies.is_empty());
                assert_eq!(fs::read(f.root.join(path)).unwrap(), b"offline work");
                assert_eq!(baseline_tests::base(&f, &inv), Some(base.clone()));
            }
            assert_eq!(remote.media_calls, 1);
            assert_eq!(remote.content["file"], b"abc");
        }
    }
}

#[path = "bulk_tests.rs"]
mod bulk_tests;
