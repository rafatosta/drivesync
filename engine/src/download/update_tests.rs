//! Managed update fixtures; no Google, user data or account vault access.
use super::*;
use md5::Digest;
fn changed(inv: &RemoteInventory, bytes: &[u8]) -> (RemoteInventory, Fake) {
    let mut next = inv.clone();
    let entry = &mut next.entries[0];
    entry.version = Some(entry.version.unwrap() + 1);
    entry.size = Some(bytes.len() as u64);
    entry.md5_checksum = Some(format!("{:x}", md5::Md5::digest(bytes)));
    let mut source = Fake::new(&next);
    source.bytes = bytes.to_vec();
    (next, source)
}
fn installed() -> (Fixture, RemoteInventory) {
    let f = Fixture::new();
    let inv = inventory();
    f.execute(&inv, &mut Fake::new(&inv), &Cancellation::new(), |_| {})
        .unwrap();
    (f, inv)
}
#[test]
fn updates_reopens_and_preserves_prior_versions_including_empty_updates() {
    let (f, inv) = installed();
    let (next, mut source) = changed(&inv, b"new content");
    let report = f
        .execute(&next, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    assert!(report.issues.is_empty());
    assert_eq!(report.updated, 1);
    assert_eq!(report.downloaded, 0);
    assert_eq!(report.preserved_copies.len(), 1);
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"new content");
    assert_eq!(
        fs::read(f.root.join(&report.preserved_copies[0])).unwrap(),
        b"abc"
    );
    assert_eq!(
        f.execute(&next, &mut source, &Cancellation::new(), |_| {})
            .unwrap()
            .unchanged,
        1
    );
    assert_eq!(source.media_calls, 1);
    let (empty, mut source) = changed(&next, b"");
    let report = f
        .execute(&empty, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    assert_eq!(report.updated, 1);
    assert!(report.issues.is_empty());
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"");
    assert_eq!(
        fs::read(f.root.join(&report.preserved_copies[0])).unwrap(),
        b"new content"
    );
}
#[test]
fn edited_local_files_are_not_downloaded_over() {
    let (f, inv) = installed();
    fs::write(f.root.join("file.txt"), b"local edit").unwrap();
    let (next, mut source) = changed(&inv, b"remote edit");
    let report = f
        .execute(&next, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    assert_eq!(report.issues[0].error, EngineError::DownloadConflict);
    assert_eq!(source.media_calls, 0);
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"local edit");
    assert!(report.preserved_copies.is_empty());
}
#[test]
fn edits_and_inode_swaps_during_download_remain_untouched() {
    for swap in [false, true] {
        let (f, inv) = installed();
        let (next, mut source) = changed(&inv, b"remote edit");
        let root = f.root.clone();
        source.on_media = Some(Box::new(move || {
            if swap {
                fs::rename(root.join("file.txt"), root.join("user-copy")).unwrap();
            }
            fs::write(
                root.join("file.txt"),
                if swap { b"abc" as &[u8] } else { b"local edit" },
            )
            .unwrap();
        }));
        let report = f
            .execute(&next, &mut source, &Cancellation::new(), |_| {})
            .unwrap();
        assert_eq!(report.issues[0].error, EngineError::DownloadConflict);
        assert_eq!(report.updated, 0);
        assert_eq!(
            fs::read(f.root.join("file.txt")).unwrap(),
            if swap { b"abc" as &[u8] } else { b"local edit" }
        );
    }
}
#[test]
fn network_checksum_and_cancellation_keep_old_file_at_its_path() {
    for failure in [0, 1, 2] {
        let (f, inv) = installed();
        let (next, mut source) = changed(&inv, b"remote edit");
        source.fail_stream = failure == 0;
        if failure == 1 {
            source.bytes = b"wrong bytes".to_vec();
        }
        let cancel = Cancellation::new();
        let result = f.execute(&next, &mut source, &cancel, |p| {
            if failure == 2
                && p.current.is_some_and(|item| {
                    item.stage == DownloadStage::Downloading && item.received > 0
                })
            {
                cancel.cancel();
            }
        });
        if failure == 2 {
            assert!(matches!(
                result,
                Err(EngineError::Authentication(crate::AuthError::Cancelled))
            ));
        } else {
            let report = result.unwrap();
            assert_eq!(report.updated, 0);
            assert_eq!(report.issues.len(), 1);
        }
        assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"abc");
    }
}
/// Reconstructs each interruption boundary from a verified successful update.
fn pending(f: &Fixture, boundary: u8) -> Operation {
    let conn = rusqlite::Connection::open(f.db()).unwrap();
    let json: String = conn
        .query_row(
            "SELECT record FROM downloads ORDER BY rowid DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let mut op: Operation = serde_json::from_str(&json).unwrap();
    let Phase::Updated {
        identity,
        sha256,
        previous,
        backup,
    } = op.phase
    else {
        panic!("not updated")
    };
    if boundary < 2 {
        fs::rename(f.root.join(&op.relative), f.root.join(&op.temporary)).unwrap();
    }
    if boundary == 0 {
        fs::rename(f.root.join(&backup), f.root.join(&op.relative)).unwrap();
    }
    op.phase = Phase::UpdateReady {
        identity,
        sha256,
        previous,
        backup,
    };
    conn.execute(
        "UPDATE downloads SET record=?1 WHERE operation=?2",
        rusqlite::params![serde_json::to_string(&op).unwrap(), op.id],
    )
    .unwrap();
    op
}
#[test]
fn recovery_covers_both_renames_and_does_not_redownload() {
    for boundary in 0..3 {
        let (f, inv) = installed();
        let (next, mut source) = changed(&inv, b"new content");
        f.execute(&next, &mut source, &Cancellation::new(), |_| {})
            .unwrap();
        pending(&f, boundary);
        let report = f
            .execute(&next, &mut source, &Cancellation::new(), |_| {})
            .unwrap();
        assert!(report.issues.is_empty(), "{report:?}");
        assert_eq!(report.recovered, 1);
        assert_eq!(source.media_calls, 1);
        assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"new content");
        assert_eq!(
            f.execute(&next, &mut source, &Cancellation::new(), |_| {})
                .unwrap()
                .unchanged,
            1
        );
    }
}
#[test]
fn new_destination_after_backup_is_not_replaced_or_restored_over() {
    let (f, inv) = installed();
    let (next, mut source) = changed(&inv, b"new content");
    f.execute(&next, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    let op = pending(&f, 1);
    fs::write(f.root.join("file.txt"), b"new user data").unwrap();
    let report = f
        .execute(&next, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    assert_eq!(report.issues[0].error, EngineError::DownloadConflict);
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"new user data");
    assert_eq!(report.preserved_copies.len(), 1);
    assert_eq!(
        fs::read(f.root.join(&report.preserved_copies[0])).unwrap(),
        b"abc"
    );
    assert!(f.root.join(op.temporary).exists());
}
#[test]
fn confirmation_failure_leaves_both_versions_and_recovers() {
    let (f, inv) = installed();
    let (next, mut source) = changed(&inv, b"new content");
    let conn = rusqlite::Connection::open(f.db()).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_update BEFORE UPDATE ON downloads WHEN json_extract(NEW.record,'$.phase.Updated') IS NOT NULL BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert_eq!(
        f.execute(&next, &mut source, &Cancellation::new(), |_| {})
            .unwrap_err(),
        EngineError::StateStorage
    );
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"new content");
    conn.execute_batch("DROP TRIGGER fail_update").unwrap();
    drop(conn);
    assert_eq!(
        f.execute(&next, &mut source, &Cancellation::new(), |_| {})
            .unwrap()
            .recovered,
        1
    );
    assert_eq!(source.media_calls, 1);
}
#[test]
fn stale_interrupted_update_restores_prior_file_and_replans_next_cycle() {
    let (f, inv) = installed();
    let (next, mut source) = changed(&inv, b"first remote");
    f.execute(&next, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    pending(&f, 1);
    let (newest, mut source) = changed(&next, b"second remote");
    let report = f
        .execute(&newest, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    assert_eq!(report.issues[0].error, EngineError::RemoteChanged);
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"abc");
    let report = f
        .execute(&newest, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    assert_eq!(report.updated, 1);
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"second remote");
}

#[test]
fn interrupted_update_is_retired_when_remote_item_disappears_or_becomes_unsupported() {
    for unsupported in [false, true] {
        let (f, inv) = installed();
        let (next, mut source) = changed(&inv, b"new content");
        f.execute(&next, &mut source, &Cancellation::new(), |_| {})
            .unwrap();
        pending(&f, 1);
        let mut absent = next.clone();
        if unsupported {
            absent.entries[0].kind = InventoryKind::Skipped(crate::SkipReason::DownloadNotAllowed);
        } else {
            absent.entries.clear();
        }
        let mut source = Fake::new(&absent);
        let report = f
            .execute(&absent, &mut source, &Cancellation::new(), |_| {})
            .unwrap();
        assert_eq!(report.issues[0].error, EngineError::RemoteChanged);
        assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"abc");
    }
}
#[test]
fn regressed_versions_and_changed_bytes_without_new_revision_are_blocked() {
    for (base_version, remote_version) in [(1, 1), (3, 2)] {
        let f = Fixture::new();
        let mut inv = inventory();
        inv.entries[0].version = Some(base_version);
        f.execute(&inv, &mut Fake::new(&inv), &Cancellation::new(), |_| {})
            .unwrap();
        let (mut next, _) = changed(&inv, b"new content");
        next.entries[0].version = Some(remote_version);
        let mut source = Fake::new(&next);
        source.bytes = b"new content".to_vec();
        let report = f
            .execute(&next, &mut source, &Cancellation::new(), |_| {})
            .unwrap();
        assert_eq!(report.issues[0].error, EngineError::RemoteChanged);
        assert_eq!(source.media_calls, 0);
        assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"abc");
    }
}
#[test]
fn remote_change_during_update_preserves_old_file() {
    let (f, inv) = installed();
    let (next, mut source) = changed(&inv, b"remote edit");
    source.changed = true;
    let report = f
        .execute(&next, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    assert_eq!(report.issues[0].error, EngineError::RemoteChanged);
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"abc");
}
#[test]
fn another_scope_cannot_update_a_managed_file() {
    let (f, inv) = installed();
    let (next, mut source) = changed(&inv, b"remote edit");
    let report = f
        .execute_client(
            "other-client",
            &next,
            &mut source,
            &Cancellation::new(),
            |_| {},
        )
        .unwrap();
    assert_eq!(report.issues[0].error, EngineError::DownloadConflict);
    assert_eq!(source.media_calls, 0);
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"abc");
}
#[test]
fn backup_collision_and_symlinks_do_not_move_or_replace_unrelated_data() {
    use std::os::unix::fs::symlink;
    for linked in [false, true] {
        let (f, inv) = installed();
        let (next, mut source) = changed(&inv, b"remote edit");
        let root = f.root.clone();
        let report = f
            .execute(&next, &mut source, &Cancellation::new(), |p| {
                if p.current
                    .is_some_and(|item| item.stage == DownloadStage::Installing)
                {
                    if linked {
                        fs::rename(root.join("file.txt"), root.join("user-original")).unwrap();
                        symlink(root.join("user-original"), root.join("file.txt")).unwrap();
                    } else {
                        let conn = rusqlite::Connection::open(f.db()).unwrap();
                        let json: String = conn
                            .query_row(
                                "SELECT record FROM downloads ORDER BY rowid DESC LIMIT 1",
                                [],
                                |r| r.get(0),
                            )
                            .unwrap();
                        let op: Operation = serde_json::from_str(&json).unwrap();
                        let Phase::UpdateReady { backup, .. } = op.phase else {
                            panic!()
                        };
                        fs::write(root.join(backup), b"unrelated data").unwrap();
                    }
                }
            })
            .unwrap();
        assert_eq!(report.issues[0].error, EngineError::DownloadConflict);
        assert_eq!(report.updated, 0);
        assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"abc");
    }
}

#[test]
fn confirmed_new_base_does_not_depend_on_user_retaining_old_backups() {
    let (f, inv) = installed();
    let (next, mut source) = changed(&inv, b"new content");
    let report = f
        .execute(&next, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    fs::remove_file(f.root.join(&report.preserved_copies[0])).unwrap();
    let report = f
        .execute(&next, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    assert_eq!(report.unchanged, 1);
    assert!(report.issues.is_empty());
    let (newest, mut source) = changed(&next, b"newer content");
    assert_eq!(
        f.execute(&newest, &mut source, &Cancellation::new(), |_| {})
            .unwrap()
            .updated,
        1
    );
}

#[test]
fn metadata_versions_advance_without_media_backups_or_inode_changes() {
    use std::os::unix::fs::MetadataExt;
    for updated_base in [false, true] {
        let (f, mut inv) = installed();
        if updated_base {
            let (next, mut source) = changed(&inv, b"actual new bytes");
            f.execute(&next, &mut source, &Cancellation::new(), |_| {})
                .unwrap();
            inv = next;
        }
        let before = fs::metadata(f.root.join("file.txt")).unwrap().ino();
        let paths_before = fs::read_dir(&f.root).unwrap().count();
        let mut newer = inv.clone();
        newer.entries[0].version = Some(inv.entries[0].version.unwrap() + 2);
        newer.entries[0].md5_checksum = newer.entries[0]
            .md5_checksum
            .as_ref()
            .map(|h| h.to_uppercase());
        let mut source = Fake::new(&newer);
        for _ in 0..2 {
            let result = f
                .execute(&newer, &mut source, &Cancellation::new(), |_| {})
                .unwrap();
            assert_eq!(result.unchanged, 1);
            assert_eq!(result.updated, 0);
            assert!(result.issues.is_empty());
            assert!(result.preserved_copies.is_empty());
        }
        assert_eq!(source.media_calls, 0);
        assert_eq!(fs::metadata(f.root.join("file.txt")).unwrap().ino(), before);
        assert_eq!(fs::read_dir(&f.root).unwrap().count(), paths_before);
        let plan =
            crate::planner::plan(&newer, &f.root, |p| crate::planner::observe(&f.root, p)).unwrap();
        let store = Store::open(&f.db()).unwrap();
        let operations = store.operations("client", &plan).unwrap();
        assert_eq!(operations.last().unwrap().item, newer.entries[0]);
        drop(store);
        let (content, mut source) = changed(&newer, b"later content change");
        let report = f
            .execute(&content, &mut source, &Cancellation::new(), |_| {})
            .unwrap();
        assert_eq!(report.updated, 1);
        assert_eq!(source.media_calls, 1);
        assert_eq!(report.preserved_copies.len(), 1);
    }
}
#[test]
fn metadata_only_changes_do_not_adopt_local_edits_swaps_or_regressions() {
    for failure in 0..4 {
        let (f, inv) = installed();
        let mut newer = inv.clone();
        newer.entries[0].version = Some(if failure == 3 { 1 } else { 3 });
        let mut source = Fake::new(&newer);
        if failure == 0 {
            fs::write(f.root.join("file.txt"), b"edited").unwrap();
        }
        if failure == 1 || failure == 2 {
            let path = f.root.join("file.txt");
            source.on_metadata = Some(Box::new(move || {
                if failure == 2 {
                    fs::rename(&path, path.with_extension("saved")).unwrap();
                }
                fs::write(&path, b"changed during metadata request").unwrap();
            }));
        }
        if failure == 3 {
            // Establish a higher confirmed revision before testing a regression.
            let mut higher = inv.clone();
            higher.entries[0].version = Some(4);
            f.execute(
                &higher,
                &mut Fake::new(&higher),
                &Cancellation::new(),
                |_| {},
            )
            .unwrap();
        }
        let result = f
            .execute(&newer, &mut source, &Cancellation::new(), |_| {})
            .unwrap();
        assert_eq!(result.unchanged, 0);
        assert_eq!(result.issues.len(), 1);
        assert_eq!(source.media_calls, 0);
        assert!(result.preserved_copies.is_empty());
    }
}
#[test]
fn metadata_confirmation_failure_and_cancellation_leave_base_retryable() {
    for cancelled in [false, true] {
        let (f, inv) = installed();
        let mut newer = inv.clone();
        newer.entries[0].version = Some(3);
        let mut source = Fake::new(&newer);
        let cancel = Cancellation::new();
        if cancelled {
            let flag = cancel.clone();
            source.on_metadata = Some(Box::new(move || flag.cancel()));
        } else {
            rusqlite::Connection::open(f.db()).unwrap().execute_batch("CREATE TRIGGER fail_metadata BEFORE UPDATE ON downloads BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
        }
        assert!(f.execute(&newer, &mut source, &cancel, |_| {}).is_err());
        assert_eq!(source.media_calls, 0);
        assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"abc");
        let conn = rusqlite::Connection::open(f.db()).unwrap();
        let json: String = conn
            .query_row("SELECT record FROM downloads", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Operation>(&json)
                .unwrap()
                .item
                .version,
            Some(1)
        );
        conn.execute_batch("DROP TRIGGER IF EXISTS fail_metadata")
            .unwrap();
        drop(conn);
        let result = f
            .execute(&newer, &mut Fake::new(&newer), &Cancellation::new(), |_| {})
            .unwrap();
        assert_eq!(result.unchanged, 1);
    }
}
#[test]
fn empty_binary_metadata_change_does_not_download_again() {
    let f = Fixture::new();
    let mut inv = inventory();
    inv.entries[0].size = Some(0);
    inv.entries[0].md5_checksum = Some("d41d8cd98f00b204e9800998ecf8427e".into());
    let mut source = Fake::new(&inv);
    source.bytes.clear();
    f.execute(&inv, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    inv.entries[0].version = Some(5);
    let mut source = Fake::new(&inv);
    let result = f
        .execute(&inv, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    assert_eq!(result.unchanged, 1);
    assert_eq!(source.media_calls, 0);
    assert!(result.preserved_copies.is_empty());
}
