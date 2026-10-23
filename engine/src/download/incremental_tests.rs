//! Feed/checkpoint/download integration using isolated files and fake providers.
use super::*;
use crate::{
    changes,
    remote::{Change, ChangePage},
    state::{CheckpointScope, Checkpoints},
};
struct Feed {
    inventory: RemoteInventory,
    events: Vec<Change>,
    scans: usize,
}
impl changes::Provider for Feed {
    fn start(&mut self) -> Result<String, EngineError> {
        Ok("start".into())
    }
    fn scan(&mut self) -> Result<RemoteInventory, EngineError> {
        self.scans += 1;
        Ok(self.inventory.clone())
    }
    fn page(&mut self, _: &str) -> Result<ChangePage, EngineError> {
        Ok(ChangePage {
            changes: std::mem::take(&mut self.events),
            next: None,
            terminal: Some(oauth2::CsrfToken::new_random().secret().clone()),
        })
    }
}
fn cycle(
    f: &Fixture,
    feed: &mut Feed,
    source: &mut Fake,
) -> Result<(DownloadReport, changes::Checkpoint), EngineError> {
    let path = f.base.join("private/changes.sqlite3");
    let account = feed.inventory.account_id.clone();
    let scope = CheckpointScope {
        client: "client",
        account: &account,
        remote: "root",
        local: &f.root,
    };
    let mut store = Checkpoints::open(&path)?;
    let old = store.load(&scope)?;
    let checkpoint = changes::collect(
        &account,
        "root",
        old.as_ref().map(|(_, c)| c),
        &Cancellation::new(),
        feed,
    )?;
    store.save(
        &scope,
        old.as_ref().map(|(r, _)| r.as_str()),
        &checkpoint,
        &Cancellation::new(),
    )?;
    let result = f.execute(&checkpoint.inventory, source, &Cancellation::new(), |_| {})?;
    Ok((result, checkpoint))
}
#[test]
fn restart_no_changes_local_deletion_and_local_edit_are_reconciled() {
    let f = Fixture::new();
    let inv = inventory();
    let mut feed = Feed {
        inventory: inv.clone(),
        events: vec![],
        scans: 0,
    };
    let mut source = Fake::new(&inv);
    assert_eq!(cycle(&f, &mut feed, &mut source).unwrap().0.downloaded, 1);
    assert_eq!(feed.scans, 1);
    source.cached = true;
    source.metadata_calls = 0;
    assert_eq!(cycle(&f, &mut feed, &mut source).unwrap().0.unchanged, 1);
    assert_eq!(source.metadata_calls, 0);
    assert_eq!(source.media_calls, 1);
    assert_eq!(feed.scans, 1);
    fs::remove_file(f.root.join("file.txt")).unwrap();
    assert_eq!(cycle(&f, &mut feed, &mut source).unwrap().0.downloaded, 1);
    assert!(source.metadata_calls > 0);
    assert_eq!(source.media_calls, 2);
    fs::write(f.root.join("file.txt"), b"my local edits").unwrap();
    assert_eq!(
        cycle(&f, &mut feed, &mut source).unwrap().0.issues[0].error,
        EngineError::DownloadConflict
    );
    assert_eq!(
        fs::read(f.root.join("file.txt")).unwrap(),
        b"my local edits"
    );
    assert_eq!(feed.scans, 1);
}
#[test]
fn observed_update_survives_failed_transfer_and_retries_without_another_event() {
    let f = Fixture::new();
    let inv = inventory();
    let mut feed = Feed {
        inventory: inv.clone(),
        events: vec![],
        scans: 0,
    };
    let mut source = Fake::new(&inv);
    cycle(&f, &mut feed, &mut source).unwrap();
    let mut newer = inv.entries[0].clone();
    newer.version = Some(2);
    newer.md5_checksum = Some("4ed9407630eb1000c0f6b63842defa7d".into());
    source.entries.insert(newer.id.clone(), newer.clone());
    source.bytes = b"def".to_vec();
    source.fail_stream = true;
    feed.events.push(Change {
        id: newer.id.clone(),
        removed: false,
        item: Some(newer.clone()),
    });
    let (failed, checkpoint) = cycle(&f, &mut feed, &mut source).unwrap();
    assert_eq!(failed.issues.len(), 1);
    assert_eq!(checkpoint.inventory.entries, [newer]);
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"abc");
    source.fail_stream = false;
    source.cached = true;
    let (retry, _) = cycle(&f, &mut feed, &mut source).unwrap();
    assert_eq!(retry.updated, 1);
    assert_eq!(retry.preserved_copies.len(), 1);
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"def");
    assert_eq!(
        fs::read(f.root.join(&retry.preserved_copies[0])).unwrap(),
        b"abc"
    );
    assert_eq!(feed.scans, 1);
}
#[test]
fn removal_is_durable_unavailability_and_preserves_user_bytes() {
    let f = Fixture::new();
    let inv = inventory();
    let mut feed = Feed {
        inventory: inv.clone(),
        events: vec![],
        scans: 0,
    };
    let mut source = Fake::new(&inv);
    cycle(&f, &mut feed, &mut source).unwrap();
    feed.events.push(Change {
        id: "file".into(),
        removed: true,
        item: None,
    });
    let (removed, checkpoint) = cycle(&f, &mut feed, &mut source).unwrap();
    assert_eq!(removed.downloaded, 0);
    assert!(checkpoint.inventory.entries.is_empty());
    assert_eq!(
        checkpoint.notices[0].path.as_deref(),
        Some(std::path::Path::new("file.txt"))
    );
    let (_, reopened) = cycle(&f, &mut feed, &mut source).unwrap();
    assert_eq!(reopened.notices, checkpoint.notices);
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"abc");
    assert_eq!(source.media_calls, 1);
    assert_eq!(feed.scans, 1);
}
#[test]
fn unchanged_directory_fast_path_never_creates_missing_directories() {
    let f = Fixture::new();
    let mut inv = inventory();
    let folder = InventoryEntry {
        id: "folder".into(),
        parent_id: "root".into(),
        name: "folder".into(),
        mime_type: "application/vnd.google-apps.folder".into(),
        version: Some(1),
        size: None,
        md5_checksum: None,
        kind: InventoryKind::Folder,
    };
    inv.entries.insert(0, folder);
    inv.entries[1].parent_id = "folder".into();
    let mut source = Fake::new(&inv);
    f.execute(&inv, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    source.cached = true;
    source.metadata_calls = 0;
    let result = f
        .execute(&inv, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    assert_eq!(result.unchanged, 1);
    assert_eq!(source.metadata_calls, 0);
    fs::remove_file(f.root.join("folder/file.txt")).unwrap();
    fs::remove_dir(f.root.join("folder")).unwrap();
    f.execute(&inv, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    assert!(source.metadata_calls > 0);
    assert_eq!(fs::read(f.root.join("folder/file.txt")).unwrap(), b"abc");
}
