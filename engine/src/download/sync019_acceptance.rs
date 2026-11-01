//! C24 acceptance with real isolated files/SQLite and controlled remote metadata.
use super::*;
use std::os::unix::fs::PermissionsExt;

fn workspace(mime: &str, product: &str) -> RemoteInventory {
    let mut inv = inventory();
    let item = &mut inv.entries[0];
    item.name = "Workspace".into();
    item.mime_type = format!("application/vnd.google-apps.{mime}");
    item.kind = InventoryKind::GoogleWorkspaceLink;
    item.size = None;
    item.md5_checksum = None;
    item.web_view_link = Some(format!("https://docs.google.com/{product}/d/test/edit"));
    inv
}
fn execute(f: &Fixture, inv: &RemoteInventory) -> DownloadReport {
    f.execute(inv, &mut Fake::new(inv), &Cancellation::new(), |_| {})
        .unwrap()
}
fn journal(f: &Fixture, inv: &RemoteInventory) -> Vec<Operation> {
    let plan = crate::planner::plan(inv, &f.root, |_| crate::LocalObservation::Unknown).unwrap();
    Store::open(&f.db())
        .unwrap()
        .operations("client", &plan)
        .unwrap()
}
#[test]
fn c24_supported_links_html_permissions_idempotence_and_journal() {
    for (mime, product) in [
        ("document", "document"),
        ("spreadsheet", "spreadsheets"),
        ("presentation", "presentation"),
        ("shortcut", "document"),
    ] {
        let f = Fixture::new();
        let inv = workspace(mime, product);
        let path = f.root.join("Workspace.html");
        let mut source = Fake::new(&inv);
        let mut inode = None;
        for _ in 0..2 {
            assert_eq!(
                f.execute(&inv, &mut source, &Cancellation::new(), |_| {})
                    .unwrap()
                    .linked,
                1
            );
            let metadata = fs::metadata(&path).unwrap();
            assert_eq!(metadata.permissions().mode() & 0o777, 0o444);
            if let Some(inode) = inode {
                assert_eq!(metadata.ino(), inode);
            }
            inode = Some(metadata.ino());
        }
        let bytes = fs::read_to_string(&path).unwrap();
        assert!(bytes.contains("http-equiv=\"refresh\""));
        assert!(bytes.contains("<a href="));
        assert!(!bytes.contains("<script"));
        assert!(bytes.contains(inv.entries[0].web_view_link.as_ref().unwrap()));
        assert_eq!(source.media_calls, 0);
        let ops = journal(&f, &inv);
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].item, inv.entries[0]);
        assert_eq!(ops[0].relative, PathBuf::from("Workspace.html"));
        assert!(matches!(ops[0].phase, Phase::Installed { .. }));
        assert!(baseline_tests::base(&f, &inv).is_none());
    }
}
#[test]
fn c24_offline_cancel_concurrent_destination_and_remote_change() {
    let f = Fixture::new();
    let inv = workspace("spreadsheet", "spreadsheets");
    let path = f.root.join("Workspace.html");
    let mut source = Fake::new(&inv);
    source.configure_error = true;
    assert!(f
        .execute(&inv, &mut source, &Cancellation::new(), |_| {})
        .is_err());
    assert!(!path.exists());
    source.configure_error = false;
    let cancel = Cancellation::new();
    let during = cancel.clone();
    assert!(f
        .execute(&inv, &mut source, &cancel, |progress| {
            if progress
                .current
                .is_some_and(|item| item.stage == DownloadStage::Linking)
            {
                during.cancel();
            }
        })
        .is_err());
    assert!(!path.exists());
    let concurrent_path = path.clone();
    source.on_metadata = Some(Box::new(move || {
        fs::write(&concurrent_path, b"LOCAL VERSION").unwrap();
    }));
    let report = f
        .execute(&inv, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    assert_eq!(report.linked, 0);
    assert_eq!(report.issues.len(), 1);
    assert_eq!(fs::read(&path).unwrap(), b"LOCAL VERSION");
    assert!(journal(&f, &inv).is_empty());
    fs::remove_file(&path).unwrap();
    source.on_metadata = None;
    source.entries.get_mut("file").unwrap().version = Some(2);
    let report = f
        .execute(&inv, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    assert_eq!(report.issues[0].error, EngineError::RemoteChanged);
    assert!(!path.exists());
}
#[test]
fn c24_remote_url_and_name_updates_preserve_previous_versions() {
    let f = Fixture::new();
    let mut inv = workspace("document", "document");
    assert_eq!(execute(&f, &inv).linked, 1);
    let old_bytes = fs::read(f.root.join("Workspace.html")).unwrap();
    inv.entries[0].version = Some(2);
    inv.entries[0].web_view_link =
        Some("https://docs.google.com/document/d/test/edit?resourcekey=new".into());
    assert_eq!(execute(&f, &inv).linked, 1);
    let backups: Vec<_> = fs::read_dir(&f.root)
        .unwrap()
        .filter_map(|e| {
            let e = e.unwrap();
            e.file_name()
                .to_str()
                .unwrap()
                .ends_with(".bak")
                .then_some(e.path())
        })
        .collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(fs::read(&backups[0]).unwrap(), old_bytes);
    let new_bytes = fs::read(f.root.join("Workspace.html")).unwrap();
    inv.entries[0].name = "Renamed".into();
    assert_eq!(execute(&f, &inv).linked, 1);
    assert_eq!(fs::read(f.root.join("Workspace.html")).unwrap(), new_bytes);
    let inode = fs::metadata(f.root.join("Renamed.html")).unwrap().ino();
    assert_eq!(execute(&f, &inv).linked, 1);
    assert_eq!(
        fs::metadata(f.root.join("Renamed.html")).unwrap().ino(),
        inode
    );
    assert_eq!(journal(&f, &inv).len(), 3);
}
#[test]
fn c24_edited_missing_renamed_and_colliding_html_preserve_local_data() {
    for mutation in ["edit", "remove", "rename", "replace", "chmod"] {
        let f = Fixture::new();
        let mut inv = workspace("document", "document");
        assert_eq!(execute(&f, &inv).linked, 1);
        let path = f.root.join("Workspace.html");
        match mutation {
            "edit" => {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
                fs::write(&path, b"USER VERSION").unwrap();
            }
            "remove" => fs::remove_file(&path).unwrap(),
            "rename" => fs::rename(&path, f.root.join("user.html")).unwrap(),
            "replace" => {
                fs::rename(&path, f.root.join("user.html")).unwrap();
                fs::write(&path, b"USER VERSION").unwrap();
            }
            "chmod" => fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap(),
            _ => unreachable!(),
        }
        inv.entries[0].version = Some(2);
        inv.entries[0].web_view_link =
            Some("https://docs.google.com/document/d/test/edit?resourcekey=changed".into());
        for _ in 0..2 {
            let report = execute(&f, &inv);
            assert_eq!(report.linked, 0);
            assert_eq!(report.issues[0].error, EngineError::DownloadConflict);
        }
        assert_eq!(journal(&f, &inv).len(), 1);
        if matches!(mutation, "edit" | "replace") {
            assert_eq!(fs::read(&path).unwrap(), b"USER VERSION");
        }
        if matches!(mutation, "remove" | "rename") {
            assert!(!path.exists());
        }
    }
    let f = Fixture::new();
    let inv = workspace("document", "document");
    fs::write(f.root.join("Workspace.html"), b"<html>ordinary HTML</html>").unwrap();
    assert_eq!(
        execute(&f, &inv).issues[0].error,
        EngineError::DownloadConflict
    );
    assert!(journal(&f, &inv).is_empty());
}
#[test]
fn c24_sqlite_failure_after_publication_recovers_without_new_inode() {
    for update in [false, true] {
        let f = Fixture::new();
        let mut inv = workspace("document", "document");
        if update {
            assert_eq!(execute(&f, &inv).linked, 1);
            inv.entries[0].version = Some(2);
            inv.entries[0].web_view_link =
                Some("https://docs.google.com/document/d/test/edit?resourcekey=new".into());
        }
        baseline_tests::prepare(&f, &inv).unwrap();
        let conn = rusqlite::Connection::open(f.db()).unwrap();
        conn.execute_batch("CREATE TRIGGER fail_confirmation BEFORE UPDATE ON downloads WHEN json_extract(NEW.record, '$.phase.Installed') IS NOT NULL BEGIN SELECT RAISE(FAIL, 'injected confirmation failure'); END;").unwrap();
        assert_eq!(
            f.execute(&inv, &mut Fake::new(&inv), &Cancellation::new(), |_| {})
                .unwrap_err(),
            EngineError::StateStorage
        );
        let inode = fs::metadata(f.root.join("Workspace.html")).unwrap().ino();
        assert!(matches!(
            journal(&f, &inv).last().unwrap().phase,
            Phase::Ready { .. } | Phase::UpdateReady { .. }
        ));
        conn.execute_batch("DROP TRIGGER fail_confirmation;")
            .unwrap();
        drop(conn);
        assert_eq!(execute(&f, &inv).linked, 1);
        assert_eq!(
            fs::metadata(f.root.join("Workspace.html")).unwrap().ino(),
            inode
        );
        assert!(matches!(
            journal(&f, &inv).last().unwrap().phase,
            Phase::Installed { .. }
        ));
    }
}
#[test]
fn c24_html_escaping_and_invalid_urls() {
    let mut inv = workspace("document", "document");
    let item = &mut inv.entries[0];
    item.name = "<script>\"'&</script>".into();
    item.web_view_link =
        Some("https://docs.google.com/document/d/test/edit?a=1&b=\"quoted\"".into());
    let text = String::from_utf8(super::super::links::entry(item).unwrap()).unwrap();
    assert!(!text.contains("<script>"));
    assert!(text.contains("&lt;script&gt;&quot;&#39;&amp;"));
    assert!(text.contains("a=1&amp;b=&quot;quoted&quot;"));
    for url in [
        "https://evil.example/document/d/test/edit",
        "javascript:alert(1)",
        "https://docs.google.com.evil.example/document/d/test/edit",
        "https://docs.google.com/not/document/d/test",
        "https://docs.google.com/document/d//edit",
        "https://docs.google.com/spreadsheets/d/test/edit",
    ] {
        item.web_view_link = Some(url.into());
        assert!(super::super::links::entry(item).is_err());
    }
}

#[test]
fn c24_restart_from_ready_and_after_prior_backup() {
    for cut in ["ready", "backup"] {
        let f = Fixture::new();
        let mut inv = workspace("document", "document");
        assert_eq!(execute(&f, &inv).linked, 1);
        inv.entries[0].version = Some(2);
        inv.entries[0].web_view_link =
            Some("https://docs.google.com/document/d/test/edit?resourcekey=new".into());
        let conn = rusqlite::Connection::open(f.db()).unwrap();
        conn.execute_batch("CREATE TRIGGER fail_confirmation BEFORE UPDATE ON downloads WHEN json_extract(NEW.record, '$.phase.Installed') IS NOT NULL BEGIN SELECT RAISE(FAIL, 'injected'); END;").unwrap();
        assert!(f
            .execute(&inv, &mut Fake::new(&inv), &Cancellation::new(), |_| {})
            .is_err());
        let op = journal(&f, &inv).pop().unwrap();
        let Phase::UpdateReady { backup, .. } = &op.phase else {
            panic!("missing ready update")
        };
        // Reconstruct the durable filesystem at each cut point using the exact
        // recorded inodes, not invented hashes or an adopted unmanaged file.
        fs::rename(f.root.join(&op.relative), f.root.join(&op.temporary)).unwrap();
        if cut == "ready" {
            fs::rename(f.root.join(backup), f.root.join(&op.relative)).unwrap();
        }
        conn.execute_batch("DROP TRIGGER fail_confirmation;")
            .unwrap();
        drop(conn);
        for _ in 0..2 {
            assert_eq!(execute(&f, &inv).linked, 1);
        }
        assert!(fs::read_to_string(f.root.join("Workspace.html"))
            .unwrap()
            .contains("resourcekey=new"));
        assert!(f.root.join(backup).exists());
        assert!(!f.root.join(&op.temporary).exists());
        assert_eq!(journal(&f, &inv).len(), 2);
    }
}

#[test]
fn c24_failed_ready_persistence_and_write_denial_publish_nothing() {
    let f = Fixture::new();
    let inv = workspace("document", "document");
    baseline_tests::prepare(&f, &inv).unwrap();
    let conn = rusqlite::Connection::open(f.db()).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_ready BEFORE INSERT ON downloads BEGIN SELECT RAISE(FAIL, 'injected'); END;").unwrap();
    assert_eq!(
        f.execute(&inv, &mut Fake::new(&inv), &Cancellation::new(), |_| {})
            .unwrap_err(),
        EngineError::StateStorage
    );
    assert!(!f.root.join("Workspace.html").exists());
    assert!(journal(&f, &inv).is_empty());
    conn.execute_batch("DROP TRIGGER fail_ready;").unwrap();
    drop(conn);
    assert_eq!(execute(&f, &inv).linked, 1);

    let f = Fixture::new();
    let mut inv = workspace("document", "document");
    inv.entries[0].parent_id = "folder".into();
    let mut folder = inv.entries[0].clone();
    folder.id = "folder".into();
    folder.parent_id = "root".into();
    folder.name = "readonly".into();
    folder.kind = InventoryKind::Folder;
    folder.mime_type = "application/vnd.google-apps.folder".into();
    folder.web_view_link = None;
    inv.entries.push(folder);
    fs::create_dir(f.root.join("readonly")).unwrap();
    fs::set_permissions(f.root.join("readonly"), fs::Permissions::from_mode(0o555)).unwrap();
    let report = execute(&f, &inv);
    fs::set_permissions(f.root.join("readonly"), fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(report.linked, 0);
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.error == EngineError::LocalAccessDenied));
    assert!(!f.root.join("readonly/Workspace.html").exists());
    assert!(journal(&f, &inv).is_empty());
}

#[test]
fn c24_pending_html_intent_cannot_be_consumed_by_binary_replay_at_same_path() {
    for update in [false, true] {
        let f = Fixture::new();
        let mut inv = workspace("document", "document");
        if update {
            assert_eq!(execute(&f, &inv).linked, 1);
            inv.entries[0].version = Some(2);
            inv.entries[0].web_view_link =
                Some("https://docs.google.com/document/d/test/edit?resourcekey=new".into());
        }
        baseline_tests::prepare(&f, &inv).unwrap();
        let conn = rusqlite::Connection::open(f.db()).unwrap();
        conn.execute_batch("CREATE TRIGGER fail_confirmation BEFORE UPDATE ON downloads WHEN json_extract(NEW.record, '$.phase.Installed') IS NOT NULL BEGIN SELECT RAISE(FAIL, 'injected'); END;").unwrap();
        assert!(f
            .execute(&inv, &mut Fake::new(&inv), &Cancellation::new(), |_| {})
            .is_err());
        let op = journal(&f, &inv).pop().unwrap();
        fs::rename(f.root.join(&op.relative), f.root.join(&op.temporary)).unwrap();
        let staged = fs::read(f.root.join(&op.temporary)).unwrap();
        conn.execute_batch("DROP TRIGGER fail_confirmation;")
            .unwrap();
        drop(conn);
        let mut binary = inventory();
        binary.entries[0].id = "ordinary".into();
        binary.entries[0].name = "Workspace.html".into();
        let mut source = Fake::new(&binary);
        let report = f
            .execute(&binary, &mut source, &Cancellation::new(), |_| {})
            .unwrap();
        assert_eq!(report.downloaded, 0);
        assert_eq!(report.issues[0].error, EngineError::DownloadConflict);
        assert_eq!(source.media_calls, 0);
        assert!(!f.root.join("Workspace.html").exists());
        assert_eq!(fs::read(f.root.join(&op.temporary)).unwrap(), staged);
        assert_eq!(execute(&f, &inv).linked, 1);
    }
}

#[test]
fn c24_metadata_only_revision_updates_journal_without_replacing_html() {
    let f = Fixture::new();
    let mut inv = workspace("document", "document");
    assert_eq!(execute(&f, &inv).linked, 1);
    let inode = fs::metadata(f.root.join("Workspace.html")).unwrap().ino();
    inv.entries[0].version = Some(2);
    assert_eq!(execute(&f, &inv).linked, 1);
    assert_eq!(
        fs::metadata(f.root.join("Workspace.html")).unwrap().ino(),
        inode
    );
    let operations = journal(&f, &inv);
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].item, inv.entries[0]);
}
