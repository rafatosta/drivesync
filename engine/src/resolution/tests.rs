//! Real temporary filesystem/SQLite with a conditional provider and fault seams.
use super::{
    execution::{self, Context, Remote},
    *,
};
use crate::{
    download::{
        filesystem::{self, Root},
        Operation, Phase,
    },
    state::Store,
    AccountId, Cancellation, ConfirmedBase, ConfirmedLocation, DownloadPlan, EngineError,
    InventoryEntry, InventoryKind, RemoteInventory,
};
use md5::{Digest, Md5};
use sha2::Sha256;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Cursor, Read},
    path::PathBuf,
};

#[derive(Clone)]
struct Object {
    name: String,
    bytes: Vec<u8>,
    version: u64,
}
fn md5(bytes: &[u8]) -> String {
    format!("{:x}", Md5::digest(bytes))
}
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
struct Provider {
    objects: HashMap<String, Object>,
    now: u64,
    effects: usize,
    update_calls: usize,
    copies: usize,
    lose_update: bool,
    fail_before: bool,
    config_changed: bool,
    local_after_update: Option<PathBuf>,
}
impl Provider {
    fn new(bytes: &[u8]) -> Self {
        Self {
            objects: HashMap::from([(
                "file".into(),
                Object {
                    name: "report.txt".into(),
                    bytes: bytes.into(),
                    version: 2,
                },
            )]),
            now: 1000,
            effects: 0,
            update_calls: 0,
            copies: 0,
            lose_update: false,
            fail_before: false,
            config_changed: false,
            local_after_update: None,
        }
    }
}
impl Remote for Provider {
    fn configured(&mut self) -> Result<(), EngineError> {
        if self.config_changed {
            Err(EngineError::InvalidPlan)
        } else {
            Ok(())
        }
    }
    fn now(&self) -> Result<u64, EngineError> {
        Ok(self.now)
    }
    fn generate(&mut self) -> Result<String, EngineError> {
        Ok("reserved-copy".into())
    }
    fn observe(&mut self, id: &str) -> Result<RemoteVersion, EngineError> {
        let object = self
            .objects
            .get(id)
            .ok_or(EngineError::RemoteFolderUnavailable)?;
        Ok(RemoteVersion {
            etag: format!("\"revision-{}\"", object.version),
            item: InventoryEntry {
                id: id.into(),
                parent_id: "root".into(),
                name: object.name.clone(),
                mime_type: "text/plain".into(),
                version: Some(object.version),
                size: Some(object.bytes.len() as u64),
                md5_checksum: Some(md5(&object.bytes)),
                web_view_link: None,
                kind: InventoryKind::Binary,
            },
        })
    }
    fn media(&mut self, version: &RemoteVersion) -> Result<Box<dyn Read>, EngineError> {
        if self.observe(&version.item.id)? != *version {
            return Err(EngineError::RemoteChanged);
        }
        Ok(Box::new(Cursor::new(
            self.objects[&version.item.id].bytes.clone(),
        )))
    }
    fn copy(&mut self, plan: &Plan, mut bytes: File, blob: &Blob) -> Result<(), EngineError> {
        let id = plan.copy_id.as_ref().ok_or(EngineError::InvalidPlan)?;
        if self.objects.contains_key(id) {
            return Err(EngineError::RemoteChanged);
        }
        let mut content = vec![];
        bytes.read_to_end(&mut content).unwrap();
        assert_eq!(sha(&content), blob.sha256);
        self.objects.insert(
            id.clone(),
            Object {
                name: plan.copy_name.clone().unwrap(),
                bytes: content,
                version: 1,
            },
        );
        self.copies += 1;
        Ok(())
    }
    fn update(&mut self, plan: &Plan, bytes: Option<File>, blob: &Blob) -> Result<(), EngineError> {
        self.update_calls += 1;
        if self.fail_before {
            return Err(EngineError::Authentication(crate::AuthError::Network));
        }
        if self.observe(&plan.remote.item.id)? != plan.remote {
            return Err(EngineError::RemoteChanged);
        }
        let content = bytes.map(|mut file| {
            let mut content = vec![];
            file.read_to_end(&mut content).unwrap();
            assert_eq!(sha(&content), blob.sha256);
            content
        });
        let object = self.objects.get_mut(&plan.remote.item.id).unwrap();
        if let Some(content) = content {
            object.bytes = content;
        }
        object.name = plan.desired_name.clone();
        object.version += 1;
        self.effects += 1;
        if let Some(path) = self.local_after_update.take() {
            fs::write(path, b"new concurrent local edit").unwrap();
        }
        if std::mem::take(&mut self.lose_update) {
            return Err(EngineError::Authentication(crate::AuthError::Network));
        }
        Ok(())
    }
}
struct Fixture {
    directory: PathBuf,
    root: PathBuf,
    db: PathBuf,
    scope: DownloadPlan,
    base: ConfirmedBase,
}
impl Fixture {
    fn new(local: &[u8]) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "drivesync-resolution-{}",
            oauth2::CsrfToken::new_random().secret()
        ));
        let root = directory.join("root");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("report.txt"), b"base").unwrap();
        let opened = Root::open(&root).unwrap();
        let fingerprint = filesystem::fingerprint_root(
            &opened,
            std::path::Path::new("report.txt"),
            &Cancellation::new(),
        )
        .unwrap()
        .unwrap();
        let base = ConfirmedBase {
            remote_version: 1,
            local_fingerprint: fingerprint,
            location: Some(ConfirmedLocation {
                relative_path: "report.txt".into(),
                remote_name: "report.txt".into(),
                remote_parent: "root".into(),
            }),
        };
        let scope = DownloadPlan {
            account_id: AccountId("account".into()),
            remote_root: "root".into(),
            local_root: root.clone(),
            entries: vec![],
        };
        let db = directory.join("state.sqlite3");
        let mut store = Store::open(&db).unwrap();
        store
            .save(
                "client",
                &RemoteInventory {
                    account_id: scope.account_id.clone(),
                    root_id: "root".into(),
                    entries: vec![],
                },
                &scope,
                &Cancellation::new(),
            )
            .unwrap();
        let identity = filesystem::identity(&File::open(root.join("report.txt")).unwrap()).unwrap();
        store
            .put_operation(
                "client",
                &scope,
                &Operation {
                    id: "base".into(),
                    item: InventoryEntry {
                        id: "file".into(),
                        parent_id: "root".into(),
                        name: "report.txt".into(),
                        mime_type: "text/plain".into(),
                        version: Some(1),
                        size: Some(4),
                        md5_checksum: Some(md5(b"base")),
                        web_view_link: None,
                        kind: InventoryKind::Binary,
                    },
                    relative: "report.txt".into(),
                    temporary: ".drivesync-base.part".into(),
                    root: filesystem::identity(&opened.file).unwrap(),
                    parent: filesystem::identity(&opened.file).unwrap(),
                    phase: Phase::Installed {
                        identity,
                        sha256: sha(b"base"),
                    },
                },
            )
            .unwrap();
        fs::write(root.join("report.txt"), local).unwrap();
        drop(opened);
        Self {
            directory,
            root,
            db,
            scope,
            base,
        }
    }
    fn plan(
        &self,
        remote: &mut Provider,
        choice: ResolutionChoice,
        origin: &str,
        target: &str,
    ) -> Plan {
        let opened = Root::open(&self.root).unwrap();
        Plan {
            conflict: None,
            origin: origin.into(),
            target: target.into(),
            local_fingerprint: filesystem::fingerprint_root(
                &opened,
                std::path::Path::new(origin),
                &Cancellation::new(),
            )
            .unwrap()
            .unwrap(),
            remote: remote.observe("file").unwrap(),
            choice,
            base: Some(self.base.clone()),
            desired_name: target.into(),
            copy_name: (choice == ResolutionChoice::KeepBoth)
                .then(|| "report (remote copy).txt".into()),
            copy_id: (choice == ResolutionChoice::KeepBoth).then(|| "reserved-copy".into()),
        }
    }
    fn begin(&self, plan: Plan) -> Intent {
        let root = Root::open(&self.root).unwrap();
        let store = Store::open(&self.db).unwrap();
        let cancel = Cancellation::new();
        execution::begin(
            &Context {
                client: "client",
                scope: &self.scope,
                root: &root,
                store: &store,
                cancel: &cancel,
            },
            plan,
            "a".repeat(64),
        )
        .unwrap()
    }
    fn run(&self, remote: &mut Provider) -> Result<ResolutionReport, EngineError> {
        let root = Root::open(&self.root)?;
        let store = Store::open(&self.db)?;
        let cancel = Cancellation::new();
        let intent = store
            .resolutions("client", &self.scope)?
            .into_iter()
            .next()
            .unwrap();
        execution::run(
            &Context {
                client: "client",
                scope: &self.scope,
                root: &root,
                store: &store,
                cancel: &cancel,
            },
            remote,
            intent,
        )
    }
    fn intent(&self) -> Intent {
        Store::open(&self.db)
            .unwrap()
            .resolutions("client", &self.scope)
            .unwrap()
            .remove(0)
    }
    fn bases(&self) -> HashMap<String, ConfirmedBase> {
        Store::open(&self.db)
            .unwrap()
            .confirmed_bases("client", &self.scope.account_id, "root", &self.root)
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn p03_all_choices_preserve_versions_confirm_b_and_replay_without_effects() {
    for choice in [
        ResolutionChoice::KeepLocal,
        ResolutionChoice::KeepRemote,
        ResolutionChoice::KeepBoth,
    ] {
        let fixture = Fixture::new(b"local edit");
        let mut remote = Provider::new(b"remote edit");
        let plan = fixture.plan(&mut remote, choice, "report.txt", "report.txt");
        fixture.begin(plan);
        assert!(fixture.bases().is_empty());
        let report = fixture.run(&mut remote).unwrap();
        let expected = if choice == ResolutionChoice::KeepRemote {
            b"remote edit".as_slice()
        } else {
            b"local edit".as_slice()
        };
        assert_eq!(fs::read(fixture.root.join("report.txt")).unwrap(), expected);
        assert_eq!(remote.objects["file"].bytes, expected);
        assert_eq!(
            fs::read(fixture.root.join(report.preserved_paths[0].clone())).unwrap(),
            b"remote edit"
        );
        assert_eq!(
            fs::read(fixture.root.join(report.preserved_paths[1].clone())).unwrap(),
            b"local edit"
        );
        if choice == ResolutionChoice::KeepBoth {
            assert_eq!(remote.objects["reserved-copy"].bytes, b"remote edit");
            assert_eq!(
                fs::read(fixture.root.join(report.additional_path.clone().unwrap())).unwrap(),
                b"remote edit"
            );
            assert_eq!(fixture.bases().len(), 2);
        } else {
            assert_eq!(fixture.bases().len(), 1);
        }
        assert_eq!(fixture.intent().stage, Stage::Confirmed);
        let effects = remote.effects;
        let copies = remote.copies;
        assert_eq!(fixture.run(&mut remote).unwrap(), report);
        assert_eq!((remote.effects, remote.copies), (effects, copies));
        let root = Root::open(&fixture.root).unwrap();
        let files = crate::local::scan(&root, &Cancellation::new()).unwrap();
        assert_eq!(
            files.entries.len(),
            if choice == ResolutionChoice::KeepBoth {
                2
            } else {
                1
            }
        );
    }
}

#[test]
fn c09_both_directions_and_c07_convergence_keep_one_active_identity() {
    for (local_rename, local_edit, remote_rename) in [
        (true, false, false),
        (false, true, true),
        (true, false, true),
    ] {
        let fixture = Fixture::new(if local_edit { b"local edit" } else { b"base" });
        let mut remote = Provider::new(if local_rename && !remote_rename {
            b"remote edit"
        } else {
            b"base"
        });
        let origin = if local_rename {
            fs::rename(
                fixture.root.join("report.txt"),
                fixture.root.join("renamed.txt"),
            )
            .unwrap();
            "renamed.txt"
        } else {
            "report.txt"
        };
        if remote_rename {
            remote.objects.get_mut("file").unwrap().name = "renamed.txt".into();
        }
        let choice = if local_edit {
            ResolutionChoice::KeepLocal
        } else {
            ResolutionChoice::KeepRemote
        };
        let plan = fixture.plan(&mut remote, choice, origin, "renamed.txt");
        fixture.begin(plan);
        fixture.run(&mut remote).unwrap();
        assert_eq!(remote.objects.len(), 1);
        assert_eq!(remote.objects["file"].name, "renamed.txt");
        assert_eq!(
            fs::read(fixture.root.join("renamed.txt")).unwrap(),
            remote.objects["file"].bytes
        );
        assert!(!fixture.root.join("report.txt").exists());
        assert_eq!(
            fixture.bases()["file"]
                .location
                .as_ref()
                .unwrap()
                .relative_path,
            PathBuf::from("renamed.txt")
        );
    }
}

#[test]
fn uncertain_update_recovers_after_reopen_without_duplicate_mutation() {
    let fixture = Fixture::new(b"local edit");
    let mut remote = Provider::new(b"remote edit");
    let plan = fixture.plan(
        &mut remote,
        ResolutionChoice::KeepLocal,
        "report.txt",
        "report.txt",
    );
    fixture.begin(plan);
    remote.lose_update = true;
    assert!(fixture.run(&mut remote).is_err());
    assert_eq!(remote.effects, 1);
    assert_eq!(fixture.intent().stage, Stage::RemotePending);
    assert!(fixture.bases().is_empty());
    assert_eq!(fixture.run(&mut remote), Err(EngineError::UploadDeferred));
    remote.now = 2000;
    fixture.run(&mut remote).unwrap();
    assert_eq!(remote.effects, 1);
    assert_eq!(fixture.intent().stage, Stage::Confirmed);
}

#[test]
fn concurrent_local_edit_after_remote_effect_is_preserved_and_never_confirms_b() {
    let fixture = Fixture::new(b"local edit");
    let mut remote = Provider::new(b"remote edit");
    let plan = fixture.plan(
        &mut remote,
        ResolutionChoice::KeepLocal,
        "report.txt",
        "report.txt",
    );
    fixture.begin(plan);
    remote.local_after_update = Some(fixture.root.join("report.txt"));
    assert_eq!(fixture.run(&mut remote), Err(EngineError::DownloadConflict));
    assert_eq!(
        fs::read(fixture.root.join("report.txt")).unwrap(),
        b"new concurrent local edit"
    );
    assert_eq!(remote.objects["file"].bytes, b"local edit");
    assert!(fixture.bases().is_empty());
    assert_eq!(
        fs::read(fixture.root.join(local::remote_name(&fixture.intent()))).unwrap(),
        b"remote edit"
    );
}

#[test]
fn stale_pre_effect_evidence_aborts_and_retains_originals_and_old_b() {
    let fixture = Fixture::new(b"local edit");
    let mut remote = Provider::new(b"remote edit");
    let plan = fixture.plan(
        &mut remote,
        ResolutionChoice::KeepLocal,
        "report.txt",
        "report.txt",
    );
    fixture.begin(plan);
    remote.objects.get_mut("file").unwrap().version += 1;
    assert_eq!(fixture.run(&mut remote), Err(EngineError::RemoteChanged));
    assert_eq!(fixture.intent().stage, Stage::Aborted);
    assert_eq!(remote.effects, 0);
    assert_eq!(fixture.bases()["file"], fixture.base);
    assert_eq!(
        fs::read(fixture.root.join("report.txt")).unwrap(),
        b"local edit"
    );
}

#[test]
fn cas_scope_isolation_and_sqlite_failure_prevent_effects() {
    let fixture = Fixture::new(b"local edit");
    let mut remote = Provider::new(b"remote edit");
    let plan = fixture.plan(
        &mut remote,
        ResolutionChoice::KeepLocal,
        "report.txt",
        "report.txt",
    );
    let intent = fixture.begin(plan);
    let store = Store::open(&fixture.db).unwrap();
    let mut next = intent.clone();
    next.retry.attempts = 1;
    store
        .save_resolution("client", &fixture.scope, Some(&intent), &next)
        .unwrap();
    assert_eq!(
        store.save_resolution("client", &fixture.scope, Some(&intent), &next),
        Err(EngineError::StateStorage)
    );
    assert!(store
        .resolutions("another-client", &fixture.scope)
        .unwrap()
        .is_empty());
    let connection = rusqlite::Connection::open(&fixture.db).unwrap();
    connection.execute_batch("CREATE TRIGGER fail BEFORE UPDATE ON resolutions BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert_eq!(fixture.run(&mut remote), Err(EngineError::StateStorage));
    assert_eq!(remote.effects, 0);
}

#[test]
fn final_baseline_failure_rolls_back_confirmation_and_replays_local_installation() {
    let fixture = Fixture::new(b"local edit");
    let mut remote = Provider::new(b"remote edit");
    let plan = fixture.plan(
        &mut remote,
        ResolutionChoice::KeepRemote,
        "report.txt",
        "report.txt",
    );
    fixture.begin(plan);
    let connection = rusqlite::Connection::open(&fixture.db).unwrap();
    connection.execute_batch("CREATE TRIGGER fail BEFORE INSERT ON downloads WHEN NEW.operation LIKE 'resolution-%' BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert_eq!(fixture.run(&mut remote), Err(EngineError::StateStorage));
    assert_eq!(fixture.intent().stage, Stage::Verified);
    assert!(fixture.bases().is_empty());
    assert_eq!(
        fs::read(fixture.root.join("report.txt")).unwrap(),
        b"remote edit"
    );
    connection.execute_batch("DROP TRIGGER fail").unwrap();
    fixture.run(&mut remote).unwrap();
    assert_eq!(fixture.intent().stage, Stage::Confirmed);
    assert_eq!(remote.effects, 0);
}

#[test]
fn retry_budget_survives_reopen_and_never_sends_a_sixth_update() {
    let fixture = Fixture::new(b"local edit");
    let mut remote = Provider::new(b"remote edit");
    let plan = fixture.plan(
        &mut remote,
        ResolutionChoice::KeepLocal,
        "report.txt",
        "report.txt",
    );
    fixture.begin(plan);
    remote.fail_before = true;
    for attempt in 1..=5 {
        assert_eq!(
            fixture.run(&mut remote),
            Err(EngineError::Authentication(crate::AuthError::Network))
        );
        assert_eq!(fixture.intent().retry.attempts, attempt);
        assert_eq!(fixture.run(&mut remote), Err(EngineError::UploadDeferred));
        remote.now += 2000;
    }
    assert_eq!(
        fixture.run(&mut remote),
        Err(EngineError::UploadRetryExhausted)
    );
    assert_eq!(remote.update_calls, 5);
    assert_eq!(remote.effects, 0);
    assert_eq!(
        fs::read(fixture.root.join("report.txt")).unwrap(),
        b"local edit"
    );
    assert!(fixture.bases().is_empty());
}

#[test]
fn interrupted_local_publication_reopens_and_recognizes_its_exact_inode() {
    let fixture = Fixture::new(b"local edit");
    let mut remote = Provider::new(b"remote edit");
    let plan = fixture.plan(
        &mut remote,
        ResolutionChoice::KeepRemote,
        "report.txt",
        "report.txt",
    );
    fixture.begin(plan);
    let connection = rusqlite::Connection::open(&fixture.db).unwrap();
    connection.execute_batch("CREATE TRIGGER fail BEFORE UPDATE ON resolutions WHEN json_extract(NEW.record,'$.stage')='Verified' BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert_eq!(fixture.run(&mut remote), Err(EngineError::StateStorage));
    assert_eq!(fixture.intent().stage, Stage::LocalPending);
    let before =
        filesystem::identity(&File::open(fixture.root.join("report.txt")).unwrap()).unwrap();
    connection.execute_batch("DROP TRIGGER fail").unwrap();
    fixture.run(&mut remote).unwrap();
    assert_eq!(
        filesystem::identity(&File::open(fixture.root.join("report.txt")).unwrap()).unwrap(),
        before
    );
    assert_eq!(fixture.intent().stage, Stage::Confirmed);
    assert_eq!(remote.effects, 0);
}

#[test]
fn displaced_inode_edits_after_cutover_are_preserved_without_b_confirmation() {
    let fixture = Fixture::new(b"local edit");
    let mut remote = Provider::new(b"remote edit");
    let plan = fixture.plan(
        &mut remote,
        ResolutionChoice::KeepRemote,
        "report.txt",
        "report.txt",
    );
    fixture.begin(plan);
    let connection = rusqlite::Connection::open(&fixture.db).unwrap();
    connection.execute_batch("CREATE TRIGGER fail BEFORE UPDATE ON resolutions WHEN json_extract(NEW.record,'$.stage')='Verified' BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert_eq!(fixture.run(&mut remote), Err(EngineError::StateStorage));
    let backup = fixture.root.join(fixture.intent().backup());
    fs::write(&backup, b"editor retained the displaced inode").unwrap();
    connection.execute_batch("DROP TRIGGER fail").unwrap();
    assert_eq!(fixture.run(&mut remote), Err(EngineError::DownloadConflict));
    assert_eq!(
        fs::read(backup).unwrap(),
        b"editor retained the displaced inode"
    );
    assert_eq!(
        fs::read(fixture.root.join("report.txt")).unwrap(),
        b"remote edit"
    );
    assert!(fixture.bases().is_empty());
}

#[test]
fn destination_collisions_and_snapshot_symlinks_never_replace_other_files() {
    let fixture = Fixture::new(b"local edit");
    let mut remote = Provider::new(b"remote edit");
    let plan = fixture.plan(
        &mut remote,
        ResolutionChoice::KeepLocal,
        "report.txt",
        "renamed.txt",
    );
    let intent = fixture.begin(plan);
    let outsider = fixture.directory.join("outside");
    fs::write(&outsider, b"outside bytes").unwrap();
    std::os::unix::fs::symlink(&outsider, fixture.root.join(intent.temporary("local"))).unwrap();
    assert_eq!(fixture.run(&mut remote), Err(EngineError::DownloadConflict));
    assert_eq!(fs::read(outsider).unwrap(), b"outside bytes");
    assert_eq!(remote.effects, 0);
    let fixture = Fixture::new(b"local edit");
    let mut remote = Provider::new(b"remote edit");
    let plan = fixture.plan(
        &mut remote,
        ResolutionChoice::KeepLocal,
        "report.txt",
        "renamed.txt",
    );
    fixture.begin(plan);
    fs::write(fixture.root.join("renamed.txt"), b"concurrent destination").unwrap();
    assert_eq!(fixture.run(&mut remote), Err(EngineError::DownloadConflict));
    assert_eq!(remote.effects, 0);
    assert_eq!(
        fs::read(fixture.root.join("renamed.txt")).unwrap(),
        b"concurrent destination"
    );
    assert_eq!(
        fs::read(fixture.root.join("report.txt")).unwrap(),
        b"local edit"
    );
}

#[test]
fn cancelled_start_does_not_publish_authority_and_root_replacement_stops_replay() {
    let fixture = Fixture::new(b"local edit");
    let mut remote = Provider::new(b"remote edit");
    let plan = fixture.plan(
        &mut remote,
        ResolutionChoice::KeepLocal,
        "report.txt",
        "report.txt",
    );
    let root = Root::open(&fixture.root).unwrap();
    let store = Store::open(&fixture.db).unwrap();
    let cancel = Cancellation::new();
    cancel.cancel();
    assert!(matches!(
        execution::begin(
            &Context {
                client: "client",
                scope: &fixture.scope,
                root: &root,
                store: &store,
                cancel: &cancel
            },
            plan.clone(),
            "b".repeat(64)
        ),
        Err(EngineError::Authentication(crate::AuthError::Cancelled))
    ));
    assert!(store
        .resolutions("client", &fixture.scope)
        .unwrap()
        .is_empty());
    drop(root);
    fixture.begin(plan);
    fs::rename(&fixture.root, fixture.directory.join("disconnected")).unwrap();
    fs::create_dir(&fixture.root).unwrap();
    fs::write(fixture.root.join("report.txt"), b"replacement volume").unwrap();
    assert_eq!(fixture.run(&mut remote), Err(EngineError::DownloadConflict));
    assert_eq!(remote.effects, 0);
    assert_eq!(
        fs::read(fixture.root.join("report.txt")).unwrap(),
        b"replacement volume"
    );
}

#[test]
fn lost_operation_database_cannot_hide_an_uncertain_remote_effect() {
    let fixture = Fixture::new(b"local edit");
    let mut remote = Provider::new(b"remote edit");
    let plan = fixture.plan(
        &mut remote,
        ResolutionChoice::KeepLocal,
        "report.txt",
        "report.txt",
    );
    fixture.begin(plan);
    remote.lose_update = true;
    assert!(fixture.run(&mut remote).is_err());
    assert_eq!(remote.effects, 1);
    fs::rename(
        &fixture.db,
        fixture.directory.join("preserved-state.sqlite3"),
    )
    .unwrap();
    let mut fresh = Store::open(&fixture.db).unwrap();
    fresh
        .save(
            "client",
            &RemoteInventory {
                account_id: fixture.scope.account_id.clone(),
                root_id: "root".into(),
                entries: vec![],
            },
            &fixture.scope,
            &Cancellation::new(),
        )
        .unwrap();
    let root = Root::open(&fixture.root).unwrap();
    let cancel = Cancellation::new();
    let ctx = Context {
        client: "client",
        scope: &fixture.scope,
        root: &root,
        store: &fresh,
        cancel: &cancel,
    };
    assert_eq!(guard::check(&ctx), Err(EngineError::StateStorage));
    assert!(fresh
        .resolutions("client", &fixture.scope)
        .unwrap()
        .is_empty());
    assert_eq!(remote.effects, 1);
    assert_eq!(
        fs::read(fixture.root.join("report.txt")).unwrap(),
        b"local edit"
    );
    assert!(fixture.directory.join("preserved-state.sqlite3").exists());
}

#[test]
fn corrupt_independent_marker_is_preserved_and_blocks_replay() {
    let fixture = Fixture::new(b"local edit");
    let mut remote = Provider::new(b"remote edit");
    let plan = fixture.plan(
        &mut remote,
        ResolutionChoice::KeepLocal,
        "report.txt",
        "report.txt",
    );
    let intent = fixture.begin(plan);
    remote.lose_update = true;
    assert!(fixture.run(&mut remote).is_err());
    let marker = fixture
        .root
        .join(format!(".drivesync-resolution-{}.guard", intent.id));
    fs::write(&marker, b"corrupt but preserved marker").unwrap();
    remote.now = 2000;
    assert_eq!(fixture.run(&mut remote), Err(EngineError::StateStorage));
    assert_eq!(remote.effects, 1);
    assert_eq!(fs::read(marker).unwrap(), b"corrupt but preserved marker");
}

#[test]
fn completed_decision_is_audited_once_without_acknowledging_newer_observations() {
    let fixture = Fixture::new(b"local edit");
    let mut remote = Provider::new(b"remote edit");
    let mut plan = fixture.plan(
        &mut remote,
        ResolutionChoice::KeepLocal,
        "report.txt",
        "report.txt",
    );
    let mut scope = fixture.scope.clone();
    scope.entries.push(crate::PlanEntry {
        remote_id: "file".into(),
        relative_path: Some("report.txt".into()),
        expected_remote_version: Some(2),
        expected_local: crate::LocalObservation::Verified(plan.local_fingerprint.clone()),
        action: crate::PlanAction::Blocked(crate::ConflictReason::ConcurrentChanges),
    });
    let root = Root::open(&fixture.root).unwrap();
    let local = crate::local::scan(&root, &Cancellation::new()).unwrap();
    drop(root);
    let mut observation = crate::state::Reconciliation::new(
        local,
        RemoteInventory {
            account_id: scope.account_id.clone(),
            root_id: "root".into(),
            entries: vec![plan.remote.item.clone()],
        },
        scope.clone(),
        HashMap::from([("file".into(), fixture.base.clone())]),
        vec![],
        None,
    );
    let history_scope = crate::state::CheckpointScope {
        client: "client",
        account: &scope.account_id,
        remote: "root",
        local: &fixture.root,
    };
    let mut history =
        crate::state::ConflictHistory::open(&fixture.directory.join("history.sqlite3")).unwrap();
    history
        .record_pending(&history_scope, &observation, &Cancellation::new())
        .unwrap();
    let record = history.records(&history_scope).unwrap().remove(0);
    plan.conflict = Some(record.conflict_id);
    fixture.begin(plan);
    fixture.run(&mut remote).unwrap();
    let intent = fixture.intent();
    history
        .complete_resolution(&history_scope, &intent)
        .unwrap();
    assert!(history.records(&history_scope).unwrap().is_empty());
    let count = history.events(&history_scope).unwrap().len();
    history
        .complete_resolution(&history_scope, &intent)
        .unwrap();
    assert_eq!(history.events(&history_scope).unwrap().len(), count);
    observation.remote.entries[0].version = Some(99);
    history
        .record_pending(&history_scope, &observation, &Cancellation::new())
        .unwrap();
    history
        .complete_resolution(&history_scope, &intent)
        .unwrap();
    let pending = history.records(&history_scope).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].remote_version, Some(99));
    assert_eq!(pending[0].status, crate::ConflictStatus::Pending);
}
