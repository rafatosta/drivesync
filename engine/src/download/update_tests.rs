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
