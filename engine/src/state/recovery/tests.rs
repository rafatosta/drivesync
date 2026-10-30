//! Real SQLite and filesystem recovery oracles, without network or credentials.
use super::*;

struct Fixture {
    folder: FolderConfiguration,
}
impl Fixture {
    fn new() -> Self {
        let local = std::env::temp_dir().join(format!(
            "drivesync-rebuild-{}",
            oauth2::CsrfToken::new_random().secret()
        ));
        std::fs::create_dir(&local).unwrap();
        let folder = FolderConfiguration {
            account_id: crate::AccountId("account".into()),
            remote: crate::DriveFolder {
                id: "remote".into(),
                name: "Remote".into(),
            },
            local_display: local.clone(),
            local,
        };
        crate::configuration::identity::ensure(&folder, "client").unwrap();
        let f = Self { folder };
        for name in NAMES {
            validate(&f.parent().join(name), name, &f.folder, "client").unwrap();
        }
        f
    }
    fn parent(&self) -> PathBuf {
        self.folder.local.join(".drivesync")
    }
    fn run(&self) -> StateRecovery {
        rebuild(&self.folder, "client", &Cancellation::new()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.folder.local);
    }
}

#[test]
fn healthy_stores_are_retained_and_recovery_is_idempotent() {
    let f = Fixture::new();
    let before = NAMES.map(|name| std::fs::read(f.parent().join(name)).unwrap());
    assert_eq!(
        f.run(),
        StateRecovery {
            rebuilt_stores: 0,
            uploads_blocked: false
        }
    );
    assert!(!f.parent().join(SELECTOR).exists());
    for (i, name) in NAMES.iter().enumerate() {
        assert_eq!(std::fs::read(f.parent().join(name)).unwrap(), before[i]);
    }
}

#[test]
fn corrupt_journal_rebuild_preserves_files_sidecars_and_healthy_observations() {
    let f = Fixture::new();
    let scope = CheckpointScope {
        client: "client",
        account: &f.folder.account_id,
        remote: "remote",
        local: &f.folder.local,
    };
    let checkpoint = crate::changes::Checkpoint {
        cursor: "cursor".into(),
        inventory: crate::RemoteInventory {
            account_id: f.folder.account_id.clone(),
            root_id: "remote".into(),
            entries: vec![],
        },
        notices: vec![],
    };
    Checkpoints::open(&f.parent().join("changes.sqlite3"))
        .unwrap()
        .save(&scope, None, &checkpoint, &Cancellation::new())
        .unwrap();
    std::fs::write(f.folder.local.join("user.txt"), b"user bytes").unwrap();
    std::fs::write(f.parent().join("state.sqlite3"), b"corrupt journal").unwrap();
    std::fs::write(f.parent().join("state.sqlite3-journal"), b"evidence").unwrap();
    assert_eq!(f.run().rebuilt_stores, 1);
    assert_eq!(
        std::fs::read(f.parent().join("state.sqlite3")).unwrap(),
        b"corrupt journal"
    );
    assert_eq!(
        std::fs::read(f.parent().join("state.sqlite3-journal")).unwrap(),
        b"evidence"
    );
    assert_eq!(
        std::fs::read(f.folder.local.join("user.txt")).unwrap(),
        b"user bytes"
    );
    assert_eq!(
        Checkpoints::open(&resolve(&f.parent(), "changes.sqlite3").unwrap())
            .unwrap()
            .load(&scope)
            .unwrap()
            .unwrap()
            .1
            .cursor,
        "cursor"
    );
    assert!(Store::open(&resolve(&f.parent(), "state.sqlite3").unwrap())
        .unwrap()
        .confirmed_bases("client", &f.folder.account_id, "remote", &f.folder.local)
        .unwrap()
        .is_empty());
    assert_eq!(f.run().rebuilt_stores, 0);
}

#[test]
fn semantic_corruption_and_incompatible_formats_are_preserved() {
    let f = Fixture::new();
    let path = f.parent().join("changes.sqlite3");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute(
        "INSERT INTO checkpoints VALUES('client','account','remote',?1,'revision','invalid json')",
        [f.folder.local.to_str().unwrap()],
    )
    .unwrap();
    drop(db);
    let original = std::fs::read(&path).unwrap();
    let incompatible = f.parent().join("reconciliation.sqlite3");
    let db = rusqlite::Connection::open(&incompatible).unwrap();
    db.pragma_update(None, "user_version", 999).unwrap();
    drop(db);
    assert_eq!(f.run().rebuilt_stores, 2);
    assert_eq!(std::fs::read(path).unwrap(), original);
    assert_eq!(
        rusqlite::Connection::open(incompatible)
            .unwrap()
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        999
    );
}

#[test]
fn missing_upload_intents_remain_blocked_across_restart_and_repetition() {
    let f = Fixture::new();
    std::fs::remove_file(f.parent().join("uploads.sqlite3")).unwrap();
    assert!(f.run().uploads_blocked);
    assert!(uploads_blocked(&f.parent()).unwrap());
    let outcome = f.run();
    assert_eq!(outcome.rebuilt_stores, 0);
    assert!(outcome.uploads_blocked);
}

#[test]
fn cancellation_unsafe_files_and_invalid_selectors_fail_closed() {
    let f = Fixture::new();
    let cancel = Cancellation::new();
    cancel.cancel();
    assert_eq!(
        rebuild(&f.folder, "client", &cancel),
        Err(EngineError::Authentication(AuthError::Cancelled))
    );
    assert!(!f.parent().join(SELECTOR).exists());
    let path = f.parent().join("state.sqlite3");
    std::fs::remove_file(&path).unwrap();
    let external = f.folder.local.join("external");
    std::fs::write(&external, b"private").unwrap();
    std::os::unix::fs::symlink(&external, path).unwrap();
    assert!(rebuild(&f.folder, "client", &Cancellation::new()).is_err());
    assert_eq!(std::fs::read(external).unwrap(), b"private");
    std::fs::write(
        f.parent().join(SELECTOR),
        br#"{"format":1,"stores":{"state.sqlite3":"../../outside"},"uploads_blocked":false}"#,
    )
    .unwrap();
    assert!(resolve(&f.parent(), "state.sqlite3").is_err());
    assert!(rebuild(&f.folder, "client", &Cancellation::new()).is_err());
}

#[test]
fn interruption_before_publication_keeps_old_selector_and_candidates() {
    let f = Fixture::new();
    let dir = directory(&f.parent()).unwrap();
    let candidate = "recovered-orphan-state.sqlite3";
    validate(
        &f.parent().join(candidate),
        "state.sqlite3",
        &f.folder,
        "client",
    )
    .unwrap();
    let mut value = selector(&dir).unwrap();
    value
        .stores
        .insert("state.sqlite3".into(), candidate.into());
    let cancel = Cancellation::new();
    cancel.cancel();
    assert!(publish(&dir, &value, &cancel).is_err());
    assert_eq!(
        resolve(&f.parent(), "state.sqlite3").unwrap(),
        f.parent().join("state.sqlite3")
    );
    assert!(f.parent().join(candidate).exists());
    assert_eq!(f.run().rebuilt_stores, 0);
}

/// Runs only in a subprocess so exit skips SQLite rollback and Rust destructors.
#[test]
fn crash_writer_child() {
    let Some(path) = std::env::var_os("DRIVESYNC_CRASH_CHECKPOINT") else {
        return;
    };
    let connection = rusqlite::Connection::open(path).unwrap();
    connection
        .execute_batch("PRAGMA synchronous=FULL; PRAGMA cache_size=1; BEGIN IMMEDIATE;")
        .unwrap();
    connection
        .execute("UPDATE checkpoints SET record=?1", ["x".repeat(512 * 1024)])
        .unwrap();
    std::process::exit(77);
}

#[test]
fn interrupted_sqlite_transaction_rolls_back_without_losing_confirmed_cursor() {
    let f = Fixture::new();
    let path = f.parent().join("changes.sqlite3");
    let scope = CheckpointScope {
        client: "client",
        account: &f.folder.account_id,
        remote: "remote",
        local: &f.folder.local,
    };
    let checkpoint = crate::changes::Checkpoint {
        cursor: "confirmed-cursor".into(),
        inventory: crate::RemoteInventory {
            account_id: f.folder.account_id.clone(),
            root_id: "remote".into(),
            entries: vec![],
        },
        notices: vec![],
    };
    Checkpoints::open(&path)
        .unwrap()
        .save(&scope, None, &checkpoint, &Cancellation::new())
        .unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "state::recovery::tests::crash_writer_child",
            "--nocapture",
        ])
        .env("DRIVESYNC_CRASH_CHECKPOINT", &path)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(77));
    let sidecar = f.parent().join("changes.sqlite3-journal");
    assert!(sidecar.exists());
    let bytes = std::fs::read(&sidecar).unwrap();
    assert_eq!(f.run().rebuilt_stores, 0);
    assert_eq!(std::fs::read(&sidecar).unwrap(), bytes);
    assert_eq!(
        Checkpoints::open(&path)
            .unwrap()
            .load(&scope)
            .unwrap()
            .unwrap()
            .1
            .cursor,
        "confirmed-cursor"
    );
}

#[test]
fn lost_selected_database_fails_closed_until_explicit_recovery() {
    let f = Fixture::new();
    std::fs::write(f.parent().join("state.sqlite3"), b"broken").unwrap();
    f.run();
    let selected = resolve(&f.parent(), "state.sqlite3").unwrap();
    std::fs::remove_file(&selected).unwrap();
    assert!(resolve(&f.parent(), "state.sqlite3").is_err());
    assert_eq!(f.run().rebuilt_stores, 1);
    assert_ne!(resolve(&f.parent(), "state.sqlite3").unwrap(), selected);
}

#[test]
fn lost_or_corrupt_selector_never_revives_stale_original_baselines() {
    for corrupt in [false, true] {
        let f = Fixture::new();
        std::fs::write(f.parent().join("changes.sqlite3"), b"invalid").unwrap();
        assert_eq!(f.run().rebuilt_stores, 1);
        let selected = resolve(&f.parent(), "changes.sqlite3").unwrap();
        if corrupt {
            std::fs::write(f.parent().join(SELECTOR), b"broken selector").unwrap();
        } else {
            std::fs::remove_file(f.parent().join(SELECTOR)).unwrap();
        }
        assert!(resolve(&f.parent(), "state.sqlite3").is_err());
        let recovered = f.run();
        assert_eq!(recovered.rebuilt_stores, 5);
        assert!(recovered.uploads_blocked);
        assert!(selected.exists());
        assert_ne!(
            resolve(&f.parent(), "state.sqlite3").unwrap(),
            f.parent().join("state.sqlite3")
        );
        assert_eq!(f.run().rebuilt_stores, 0);
    }
}

#[test]
fn interruption_after_no_fallback_marker_is_explicitly_recoverable() {
    let f = Fixture::new();
    std::fs::write(f.parent().join(REQUIRED), b"").unwrap();
    assert!(resolve(&f.parent(), "state.sqlite3").is_err());
    assert_eq!(f.run().rebuilt_stores, 5);
    assert!(f.run().uploads_blocked);
}
