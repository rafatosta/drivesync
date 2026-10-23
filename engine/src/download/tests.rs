//! Isolated filesystem/SQLite fixtures and fake provider; no real account access.
use super::filesystem::Root;
use super::*;
use crate::AccountId;
use crate::{
    state::Store, Cancellation, EngineError, InventoryEntry, InventoryKind, RemoteInventory,
};
use std::io::Read;
use std::{
    collections::HashMap,
    fs,
    io::{self, Cursor},
    path::PathBuf,
};

struct Fixture {
    base: PathBuf,
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "drivesync-download-{}",
            oauth2::CsrfToken::new_random().secret()
        ));
        let root = base.join("files");
        fs::create_dir_all(&root).unwrap();
        Self { base, root }
    }
    fn db(&self) -> PathBuf {
        self.base.join("private/state.sqlite3")
    }
    fn execute(
        &self,
        inv: &RemoteInventory,
        source: &mut Fake,
        cancel: &Cancellation,
        progress: impl FnMut(DownloadProgress),
    ) -> Result<DownloadReport, EngineError> {
        self.execute_client("client", inv, source, cancel, progress)
    }
    fn execute_client(
        &self,
        client: &str,
        inv: &RemoteInventory,
        source: &mut Fake,
        cancel: &Cancellation,
        progress: impl FnMut(DownloadProgress),
    ) -> Result<DownloadReport, EngineError> {
        let plan =
            crate::planner::plan(inv, &self.root, |p| crate::planner::observe(&self.root, p))
                .unwrap();
        let mut store = Store::open(&self.db()).unwrap();
        store.save(client, inv, &plan, cancel)?;
        run(
            Context {
                client,
                scope: &plan,
                store: &store,
                cancel,
            },
            inv,
            source,
            progress,
        )
    }
    fn reset_to_ready(&self, move_to_temp: bool) {
        let conn = rusqlite::Connection::open(self.db()).unwrap();
        let json: String = conn
            .query_row("SELECT record FROM downloads", [], |r| r.get(0))
            .unwrap();
        let mut op: Operation = serde_json::from_str(&json).unwrap();
        let Phase::Installed { identity, sha256 } = op.phase else {
            panic!("not installed")
        };
        op.phase = Phase::Ready { identity, sha256 };
        if move_to_temp {
            fs::rename(self.root.join(&op.relative), self.root.join(&op.temporary)).unwrap();
        }
        conn.execute(
            "UPDATE downloads SET record=?1",
            [serde_json::to_string(&op).unwrap()],
        )
        .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}
fn inventory() -> RemoteInventory {
    RemoteInventory {
        account_id: AccountId("google:test".into()),
        root_id: "root".into(),
        entries: vec![InventoryEntry {
            id: "file".into(),
            parent_id: "root".into(),
            name: "file.txt".into(),
            mime_type: "text/plain".into(),
            version: Some(1),
            size: Some(3),
            md5_checksum: Some("900150983cd24fb0d6963f7d28e17f72".into()),
            kind: InventoryKind::Binary,
        }],
    }
}
struct Fake {
    entries: HashMap<String, InventoryEntry>,
    bytes: Vec<u8>,
    media_calls: usize,
    changed: bool,
    fail_stream: bool,
    on_media: Option<Box<dyn FnMut()>>,
    configure_error: bool,
}
impl Fake {
    fn new(inv: &RemoteInventory) -> Self {
        Self {
            entries: inv
                .entries
                .iter()
                .map(|e| (e.id.clone(), e.clone()))
                .collect(),
            bytes: b"abc".to_vec(),
            media_calls: 0,
            changed: false,
            fail_stream: false,
            on_media: None,
            configure_error: false,
        }
    }
}
impl Source for Fake {
    fn metadata(&mut self, item: &InventoryEntry) -> Result<InventoryEntry, EngineError> {
        let mut value = self.entries[&item.id].clone();
        if self.changed && self.media_calls > 0 {
            value.version = Some(2);
        }
        Ok(value)
    }
    fn media(&mut self, _: &str) -> Result<Box<dyn Read>, EngineError> {
        self.media_calls += 1;
        if let Some(f) = &mut self.on_media {
            f();
        }
        if self.fail_stream {
            Ok(Box::new(BrokenReader(false)))
        } else {
            Ok(Box::new(Cursor::new(self.bytes.clone())))
        }
    }
    fn configured(&mut self) -> Result<(), EngineError> {
        if self.configure_error {
            Err(EngineError::InvalidPlan)
        } else {
            Ok(())
        }
    }
}
struct BrokenReader(bool);
impl Read for BrokenReader {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        if self.0 {
            Err(io::Error::other("injected network failure"))
        } else {
            self.0 = true;
            b[0] = b'a';
            Ok(1)
        }
    }
}
#[test]
fn installs_verifies_reopens_and_never_overwrites_local_changes() {
    let f = Fixture::new();
    let inv = inventory();
    let mut source = Fake::new(&inv);
    let mut progress = Vec::new();
    let result = f
        .execute(&inv, &mut source, &Cancellation::new(), |p| {
            progress.push(p.received)
        })
        .unwrap();
    assert_eq!(result.downloaded, 1);
    assert!(result.issues.is_empty());
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"abc");
    assert_eq!(progress, vec![0, 3]);
    let result = f
        .execute(&inv, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    assert_eq!(result.unchanged, 1);
    assert_eq!(source.media_calls, 1);
    fs::write(f.root.join("file.txt"), b"xyz").unwrap();
    let result = f
        .execute(&inv, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    assert_eq!(result.issues[0].error, EngineError::DownloadConflict);
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"xyz");
    assert_eq!(source.media_calls, 1);
}
#[test]
fn rejects_short_long_and_corrupt_streams_without_final_file() {
    for bytes in [b"ab".to_vec(), b"abcd".to_vec(), b"xyz".to_vec()] {
        let f = Fixture::new();
        let inv = inventory();
        let mut source = Fake::new(&inv);
        source.bytes = bytes;
        let result = f
            .execute(&inv, &mut source, &Cancellation::new(), |_| {})
            .unwrap();
        assert_eq!(result.issues[0].error, EngineError::DownloadIntegrity);
        assert!(!f.root.join("file.txt").exists());
    }
}
#[test]
fn accepts_empty_binary_and_creates_nested_folders() {
    let f = Fixture::new();
    let mut inv = inventory();
    let mut folder = inv.entries[0].clone();
    folder.id = "directory".into();
    folder.name = "nested".into();
    folder.kind = InventoryKind::Folder;
    inv.entries[0].parent_id = folder.id.clone();
    inv.entries[0].size = Some(0);
    inv.entries[0].md5_checksum = Some("d41d8cd98f00b204e9800998ecf8427e".into());
    inv.entries.push(folder);
    let mut source = Fake::new(&inv);
    source.bytes.clear();
    let result = f
        .execute(&inv, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    assert_eq!(result.downloaded, 1);
    assert_eq!(result.directories, 1);
    assert!(result.issues.is_empty());
    assert_eq!(fs::read(f.root.join("nested/file.txt")).unwrap(), b"");
}
#[test]
fn network_failure_and_cancellation_preserve_final_namespace() {
    let f = Fixture::new();
    let inv = inventory();
    let mut source = Fake::new(&inv);
    source.fail_stream = true;
    let result = f
        .execute(&inv, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    assert!(matches!(
        result.issues[0].error,
        EngineError::Authentication(crate::AuthError::Network)
    ));
    assert!(!f.root.join("file.txt").exists());
    source.fail_stream = false;
    let cancel = Cancellation::new();
    assert!(matches!(
        f.execute(&inv, &mut source, &cancel, |p| if p.received > 0 {
            cancel.cancel();
        }),
        Err(EngineError::Authentication(crate::AuthError::Cancelled))
    ));
    assert!(!f.root.join("file.txt").exists());
    assert_eq!(
        f.execute(&inv, &mut source, &Cancellation::new(), |_| {})
            .unwrap()
            .downloaded,
        1
    );
}
#[test]
fn remote_version_changes_and_concurrent_local_creation_are_conflicts() {
    let inv = inventory();
    let f = Fixture::new();
    let mut source = Fake::new(&inv);
    source.changed = true;
    let result = f
        .execute(&inv, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    assert_eq!(result.issues[0].error, EngineError::RemoteChanged);
    assert!(!f.root.join("file.txt").exists());
    let f = Fixture::new();
    let mut source = Fake::new(&inv);
    let dest = f.root.join("file.txt");
    source.on_media = Some(Box::new(move || fs::write(&dest, b"keep").unwrap()));
    let result = f
        .execute(&inv, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    assert_eq!(result.issues[0].error, EngineError::DownloadConflict);
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"keep");
}
#[test]
fn existing_identical_file_is_not_adopted_and_accounts_are_isolated() {
    let f = Fixture::new();
    let inv = inventory();
    let mut source = Fake::new(&inv);
    fs::write(f.root.join("file.txt"), b"abc").unwrap();
    assert_eq!(
        f.execute(&inv, &mut source, &Cancellation::new(), |_| {})
            .unwrap()
            .issues[0]
            .error,
        EngineError::DownloadConflict
    );
    assert_eq!(source.media_calls, 0);
    fs::remove_file(f.root.join("file.txt")).unwrap();
    f.execute(&inv, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    let result = f
        .execute_client(
            "other-client",
            &inv,
            &mut source,
            &Cancellation::new(),
            |_| {},
        )
        .unwrap();
    assert_eq!(result.issues[0].error, EngineError::DownloadConflict);
    let mut other = inv.clone();
    other.account_id = AccountId("other".into());
    assert_eq!(
        f.execute(&other, &mut source, &Cancellation::new(), |_| {})
            .unwrap()
            .issues[0]
            .error,
        EngineError::DownloadConflict
    );
}
#[test]
fn symlinks_never_redirect_downloads() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let outside = f.base.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("keep"), b"keep").unwrap();
    let mut inv = inventory();
    inv.entries[0].name = "keep".into();
    symlink(outside.join("keep"), f.root.join("keep")).unwrap();
    let mut source = Fake::new(&inv);
    assert_eq!(
        f.execute(&inv, &mut source, &Cancellation::new(), |_| {})
            .unwrap()
            .issues[0]
            .error,
        EngineError::DownloadConflict
    );
    assert_eq!(source.media_calls, 0);
    inv.entries[0].parent_id = "dir".into();
    let mut dir = inv.entries[0].clone();
    dir.id = "dir".into();
    dir.parent_id = "root".into();
    dir.name = "linked".into();
    dir.kind = InventoryKind::Folder;
    inv.entries.push(dir);
    symlink(&outside, f.root.join("linked")).unwrap();
    let mut source = Fake::new(&inv);
    assert_eq!(
        f.execute(&inv, &mut source, &Cancellation::new(), |_| {})
            .unwrap()
            .issues
            .len(),
        2
    );
    assert_eq!(source.media_calls, 0);
    assert_eq!(fs::read(outside.join("keep")).unwrap(), b"keep");
}
#[test]
fn ready_replays_before_and_after_atomic_rename() {
    for move_to_temp in [true, false] {
        let f = Fixture::new();
        let inv = inventory();
        let mut source = Fake::new(&inv);
        f.execute(&inv, &mut source, &Cancellation::new(), |_| {})
            .unwrap();
        f.reset_to_ready(move_to_temp);
        let result = f
            .execute(&inv, &mut source, &Cancellation::new(), |_| {})
            .unwrap();
        assert_eq!(result.recovered, 1);
        assert!(result.issues.is_empty());
        assert_eq!(source.media_calls, 1);
        assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"abc");
        assert_eq!(
            f.execute(&inv, &mut source, &Cancellation::new(), |_| {})
                .unwrap()
                .unchanged,
            1
        );
    }
}
#[test]
fn journal_confirmation_failure_is_recoverable_without_redownload() {
    let f = Fixture::new();
    let inv = inventory();
    let plan = crate::planner::plan(&inv, &f.root, |_| crate::LocalObservation::Absent).unwrap();
    let mut store = Store::open(&f.db()).unwrap();
    store
        .save("client", &inv, &plan, &Cancellation::new())
        .unwrap();
    drop(store);
    let conn = rusqlite::Connection::open(f.db()).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_confirm BEFORE UPDATE ON downloads WHEN json_extract(NEW.record,'$.phase.Installed') IS NOT NULL BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    let mut source = Fake::new(&inv);
    assert_eq!(
        f.execute(&inv, &mut source, &Cancellation::new(), |_| {})
            .unwrap_err(),
        EngineError::StateStorage
    );
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"abc");
    conn.execute_batch("DROP TRIGGER fail_confirm").unwrap();
    drop(conn);
    assert_eq!(
        f.execute(&inv, &mut source, &Cancellation::new(), |_| {})
            .unwrap()
            .recovered,
        1
    );
    assert_eq!(source.media_calls, 1);
}
#[test]
fn root_lock_and_configuration_changes_stop_the_cycle() {
    let f = Fixture::new();
    let inv = inventory();
    let mut source = Fake::new(&inv);
    let lock = Root::open(&f.root).unwrap();
    assert_eq!(
        f.execute(&inv, &mut source, &Cancellation::new(), |_| {})
            .unwrap_err(),
        EngineError::SyncBusy
    );
    drop(lock);
    source.configure_error = true;
    assert_eq!(
        f.execute(&inv, &mut source, &Cancellation::new(), |_| {})
            .unwrap_err(),
        EngineError::InvalidPlan
    );
    assert_eq!(source.media_calls, 0);
}
#[test]
fn stream_write_failure_never_claims_integrity() {
    struct Fail;
    impl io::Write for Fail {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("injected disk full"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    assert_eq!(
        stream::copy(
            &mut Cursor::new(b"abc"),
            &mut Fail,
            3,
            "900150983cd24fb0d6963f7d28e17f72",
            &Cancellation::new(),
            |_| {}
        )
        .unwrap_err(),
        EngineError::DownloadIo
    );
}

#[test]
fn replacing_parent_during_stream_cannot_redirect_installation() {
    let f = Fixture::new();
    let mut inv = inventory();
    let mut folder = inv.entries[0].clone();
    folder.id = "folder".into();
    folder.name = "nested".into();
    folder.kind = InventoryKind::Folder;
    inv.entries[0].parent_id = "folder".into();
    inv.entries.push(folder);
    let mut source = Fake::new(&inv);
    let root = f.root.clone();
    let outside = f.base.join("outside");
    fs::create_dir(&outside).unwrap();
    let target = outside.clone();
    source.on_media = Some(Box::new(move || {
        fs::rename(root.join("nested"), root.join("moved")).unwrap();
        std::os::unix::fs::symlink(&target, root.join("nested")).unwrap();
    }));
    let result = f
        .execute(&inv, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    assert_eq!(result.issues.len(), 1);
    assert!(!outside.join("file.txt").exists());
    assert!(!f.root.join("moved/file.txt").exists());
}
#[test]
fn interrupted_staging_mutation_is_not_confirmed_or_installed() {
    let f = Fixture::new();
    let inv = inventory();
    let mut source = Fake::new(&inv);
    f.execute(&inv, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    f.reset_to_ready(true);
    let temp = fs::read_dir(&f.root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|x| x == "part"))
        .unwrap();
    fs::write(&temp, b"xyz").unwrap();
    let report = f
        .execute(&inv, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    assert_eq!(report.issues[0].error, EngineError::DownloadIntegrity);
    assert!(!f.root.join("file.txt").exists());
    assert_eq!(fs::read(temp).unwrap(), b"xyz");
}

#[test]
fn journal_identity_collision_cannot_report_a_successful_write() {
    let f = Fixture::new();
    let inv = inventory();
    let mut source = Fake::new(&inv);
    f.execute(&inv, &mut source, &Cancellation::new(), |_| {})
        .unwrap();
    let plan =
        crate::planner::plan(&inv, &f.root, |p| crate::planner::observe(&f.root, p)).unwrap();
    let store = Store::open(&f.db()).unwrap();
    let before = store.operations("client", &plan).unwrap();
    let mut forged = before[0].clone();
    forged.item.id = "different".into();
    assert_eq!(
        store.put_operation("client", &plan, &forged).unwrap_err(),
        EngineError::StateStorage
    );
    assert_eq!(
        store.operations("client", &plan).unwrap()[0].item.id,
        "file"
    );
}
