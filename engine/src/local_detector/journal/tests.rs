use super::*;
use crate::local_detector::{LocalMetadata, LocalObject};
use std::{os::unix::ffi::OsStrExt, time::Instant};

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "drivesync-local-journal-{}",
            oauth2::CsrfToken::new_random().secret()
        )))
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        if self.0.exists() {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

fn scope(account: &str, root: &str) -> LocalScope {
    LocalScope::new("client", account, Path::new(root)).unwrap()
}

fn entry(inode: u64, object: LocalObject) -> LocalEntry {
    LocalEntry {
        metadata: LocalMetadata {
            device: 1,
            inode,
            mode: 0,
            size: 0,
            modified_ns: 20_000_000_000_000_000_000,
            changed_ns: -20_000_000_000_000_000_000,
        },
        object,
    }
}

fn inventory(digest: u8) -> LocalInventory {
    LocalInventory::new(vec![
        (vec![], entry(1, LocalObject::Directory)),
        (
            vec![0xff],
            entry(
                2,
                LocalObject::File {
                    sha256: [digest; 32],
                },
            ),
        ),
    ])
    .unwrap()
}

#[test]
fn persists_inventory_and_redelivers_until_scoped_acknowledgement() {
    let directory = Directory::new();
    let journal = LocalJournal::open(&directory.0).unwrap();
    let a = scope("a", "/root");
    let b = scope("b", "/root");
    let other_root = scope("a", "/other");
    let other_client = LocalScope::new("other", "a", Path::new("/root")).unwrap();
    let byte_root = LocalScope::new(
        "client",
        "a",
        Path::new(std::ffi::OsStr::from_bytes(b"/root/\xff")),
    )
    .unwrap();
    journal.commit(&a, None, &inventory(1)).unwrap();
    journal.commit(&b, None, &inventory(2)).unwrap();
    journal.commit(&byte_root, None, &inventory(3)).unwrap();
    let events = journal.pending(&a).unwrap();
    assert_eq!(events.len(), 3);
    drop(journal);
    let journal = LocalJournal::open(&directory.0).unwrap();
    let checkpoint = journal.load(&a).unwrap().unwrap();
    assert_eq!(checkpoint.generation, 1);
    assert_eq!(checkpoint.inventory, inventory(1));
    assert_eq!(journal.pending(&a).unwrap(), events);
    assert!(!journal.acknowledge(&b, events[0].id).unwrap());
    assert!(!journal.acknowledge(&other_root, events[0].id).unwrap());
    assert!(!journal.acknowledge(&other_client, events[0].id).unwrap());
    assert!(journal.acknowledge(&a, events[0].id).unwrap());
    assert!(!journal.acknowledge(&a, events[0].id).unwrap());
    assert_eq!(journal.pending(&b).unwrap().len(), 3);
    assert!(journal.load(&other_root).unwrap().is_none());
    assert!(journal.load(&other_client).unwrap().is_none());
    assert_eq!(
        journal.load(&byte_root).unwrap().unwrap().inventory,
        inventory(3)
    );
    assert_eq!(
        journal.acknowledge(&a, 0),
        Err(LocalJournalError::InvalidScope)
    );
}

#[test]
fn stale_writers_and_changed_roots_preserve_state() {
    let directory = Directory::new();
    let first = LocalJournal::open(&directory.0).unwrap();
    let second = LocalJournal::open(&directory.0).unwrap();
    let scope = scope("a", "/root");
    first.commit(&scope, None, &inventory(1)).unwrap();
    second.commit(&scope, Some(1), &inventory(2)).unwrap();
    let before = first.pending(&scope).unwrap();
    assert_eq!(
        before.last().unwrap().observation,
        LocalObservation::Reevaluate {
            path: vec![0xff],
            previous: inventory(1).entries.get(&[0xff][..]).cloned(),
            current: inventory(2).entries.get(&[0xff][..]).cloned(),
        }
    );
    assert_eq!(
        first.commit(&scope, Some(1), &inventory(3)),
        Err(LocalJournalError::Stale)
    );
    let mut replaced = inventory(3);
    replaced.entries.get_mut(&[][..]).unwrap().metadata.inode = 99;
    assert_eq!(
        first.commit(&scope, Some(2), &replaced),
        Err(LocalJournalError::RootChanged)
    );
    assert_eq!(first.load(&scope).unwrap().unwrap().inventory, inventory(2));
    assert_eq!(first.pending(&scope).unwrap(), before);
}

#[test]
fn event_insertion_failure_rolls_back_inventory_and_all_events() {
    let directory = Directory::new();
    let journal = LocalJournal::open(&directory.0).unwrap();
    let scope = scope("a", "/root");
    journal.commit(&scope, None, &inventory(1)).unwrap();
    let before = journal.pending(&scope).unwrap();
    journal
        .connect()
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER reject_event BEFORE INSERT ON events
             WHEN (SELECT count(*) FROM events) > 3
             BEGIN SELECT RAISE(ABORT, 'injected failure'); END;",
        )
        .unwrap();
    let mut changed = inventory(2);
    changed.entries.get_mut(&[][..]).unwrap().metadata.size = 1;
    assert_eq!(
        journal.commit(&scope, Some(1), &changed),
        Err(LocalJournalError::Storage)
    );
    let reopened = LocalJournal::open(&directory.0).unwrap();
    let checkpoint = reopened.load(&scope).unwrap().unwrap();
    assert_eq!(checkpoint.generation, 1);
    assert_eq!(checkpoint.inventory, inventory(1));
    assert_eq!(reopened.pending(&scope).unwrap(), before);
}

#[test]
fn incompatible_or_corrupt_data_is_preserved() {
    let directory = Directory::new();
    let journal = LocalJournal::open(&directory.0).unwrap();
    let scope = scope("a", "/root");
    journal.commit(&scope, None, &inventory(1)).unwrap();
    let event_id = journal.pending(&scope).unwrap()[0].id;
    let connection = journal.connect().unwrap();
    connection
        .execute("UPDATE events SET payload = X'00' WHERE id=?1", [event_id])
        .unwrap();
    assert_eq!(
        journal.pending(&scope),
        Err(LocalJournalError::Incompatible)
    );
    assert_eq!(
        journal.acknowledge(&scope, event_id),
        Err(LocalJournalError::Incompatible)
    );
    let count: i64 = connection
        .query_row("SELECT count(*) FROM events", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 3);
    connection
        .execute("UPDATE scopes SET inventory = X'00'", [])
        .unwrap();
    assert_eq!(journal.load(&scope), Err(LocalJournalError::Incompatible));
    assert_eq!(
        journal.commit(&scope, Some(1), &inventory(2)),
        Err(LocalJournalError::Incompatible)
    );
    connection.pragma_update(None, "user_version", 99).unwrap();
    drop(connection);
    let original = fs::read(&journal.path).unwrap();
    assert!(matches!(
        LocalJournal::open(&directory.0),
        Err(LocalJournalError::Incompatible)
    ));
    assert_eq!(fs::read(&journal.path).unwrap(), original);
}

#[test]
fn validates_scope_and_private_storage() {
    assert!(LocalScope::new("", "account", Path::new("/test")).is_err());
    assert!(LocalScope::new("client", "account", Path::new("/")).is_err());
    assert!(LocalScope::new("client", "account", Path::new("/test/../other")).is_err());
    assert!(LocalScope::new("client", "account", Path::new("/test//other")).is_err());
    let non_utf8 = Path::new(std::ffi::OsStr::from_bytes(b"/test/\xff"));
    assert!(LocalScope::new("client", "account", non_utf8).is_ok());
    let directory = Directory::new();
    let journal = LocalJournal::open(&directory.0).unwrap();
    fs::set_permissions(&journal.path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(matches!(
        LocalJournal::open(&directory.0),
        Err(LocalJournalError::Storage)
    ));
}

#[test]
fn refuses_database_and_directory_symlinks() {
    let directory = Directory::new();
    let journal = LocalJournal::open(&directory.0).unwrap();
    let other = Directory::new();
    std::os::unix::fs::symlink(&directory.0, &other.0).unwrap();
    assert!(matches!(
        LocalJournal::open(&other.0),
        Err(LocalJournalError::Storage)
    ));
    fs::remove_file(&other.0).unwrap();
    let path = journal.path.clone();
    let saved = directory.0.join("saved.sqlite3");
    fs::rename(&path, &saved).unwrap();
    std::os::unix::fs::symlink(&saved, &path).unwrap();
    assert!(matches!(
        LocalJournal::open(&directory.0),
        Err(LocalJournalError::Storage)
    ));
}

#[test]
fn concurrent_initialization_preserves_both_scopes() {
    let directory = Directory::new();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    std::thread::scope(|threads| {
        for account in ["a", "b"] {
            let barrier = barrier.clone();
            let path = &directory.0;
            threads.spawn(move || {
                barrier.wait();
                let journal = LocalJournal::open(path).unwrap();
                journal
                    .commit(&scope(account, "/root"), None, &inventory(1))
                    .unwrap();
            });
        }
    });
    let journal = LocalJournal::open(&directory.0).unwrap();
    for account in ["a", "b"] {
        assert_eq!(journal.pending(&scope(account, "/root")).unwrap().len(), 3);
    }
}

#[test]
fn killed_writer_rolls_back_uncommitted_inventory_and_events() {
    const CHILD: &str = "DRIVESYNC_TEST_LOCAL_JOURNAL_CHILD";
    if let Some(path) = std::env::var_os(CHILD) {
        let journal = LocalJournal::open(Path::new(&path)).unwrap();
        let result = journal.commit(&scope("a", "/root"), Some(1), &inventory(2));
        panic!("writer unexpectedly finished: {result:?}");
    }
    let directory = Directory::new();
    let journal = LocalJournal::open(&directory.0).unwrap();
    let scope = scope("a", "/root");
    journal.commit(&scope, None, &inventory(1)).unwrap();
    let old_events = journal.pending(&scope).unwrap();
    let reader = journal.connect().unwrap();
    reader
        .execute_batch("BEGIN; SELECT count(*) FROM scopes;")
        .unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "local_detector::journal::tests::killed_writer_rolls_back_uncommitted_inventory_and_events",
            "--nocapture",
        ])
        .env(CHILD, &directory.0)
        .spawn()
        .unwrap();
    let rollback = directory.0.join("local-detector.sqlite3-journal");
    let deadline = Instant::now() + Duration::from_secs(4);
    let mut started = false;
    while Instant::now() < deadline {
        if fs::metadata(&rollback).is_ok_and(|metadata| metadata.len() > 0) {
            started = true;
            break;
        }
        if child.try_wait().unwrap().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let killed = child.kill();
    let exit = child.wait().unwrap();
    reader.execute_batch("ROLLBACK;").unwrap();
    assert!(started, "writer did not create its rollback journal");
    killed.expect("writer exited before interruption");
    assert!(!exit.success());
    let reopened = LocalJournal::open(&directory.0).unwrap();
    let checkpoint = reopened.load(&scope).unwrap().unwrap();
    assert_eq!(checkpoint.generation, 1);
    assert_eq!(checkpoint.inventory, inventory(1));
    assert_eq!(reopened.pending(&scope).unwrap(), old_events);
    reopened.commit(&scope, Some(1), &inventory(2)).unwrap();
    assert_eq!(reopened.load(&scope).unwrap().unwrap().generation, 2);
}
