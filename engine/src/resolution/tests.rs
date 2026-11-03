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
    preview_fault: u8,
    preview_path: Option<PathBuf>,
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
            preview_fault: 0,
            preview_path: None,
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
        let mut bytes = self.objects[&version.item.id].bytes.clone();
        match self.preview_fault {
            1 => return Err(EngineError::Authentication(crate::AuthError::Network)),
            2 => bytes.push(0),
            3 => self.objects.get_mut(&version.item.id).unwrap().version += 1,
            4 => fs::write(self.preview_path.as_ref().unwrap(), b"concurrent").unwrap(),
            _ => {}
        }
        Ok(Box::new(Cursor::new(bytes)))
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
impl crate::download::Source for Provider {
    fn configured(&mut self) -> Result<(), EngineError> {
        Remote::configured(self)
    }
    fn metadata(&mut self, item: &InventoryEntry) -> Result<InventoryEntry, EngineError> {
        Ok(self.observe(&item.id)?.item)
    }
    fn media(&mut self, id: &str) -> Result<Box<dyn Read>, EngineError> {
        Ok(Box::new(Cursor::new(
            self.objects
                .get(id)
                .ok_or(EngineError::RemoteFolderUnavailable)?
                .bytes
                .clone(),
        )))
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
        Self::with_base(local, b"base")
    }
    fn with_base(local: &[u8], base_bytes: &[u8]) -> Self {
        let directory = std::env::var_os("DRIVESYNC_RESOLUTION_FIXTURE_PARENT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(format!(
                "drivesync-resolution-{}",
                oauth2::CsrfToken::new_random().secret()
            ));
        let root = directory.join("root");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("report.txt"), base_bytes).unwrap();
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
            .put_verified_operation(
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
                        size: Some(base_bytes.len() as u64),
                        md5_checksum: Some(md5(base_bytes)),
                        web_view_link: None,
                        kind: InventoryKind::Binary,
                    },
                    relative: "report.txt".into(),
                    temporary: ".drivesync-base.part".into(),
                    root: filesystem::identity(&opened.file).unwrap(),
                    parent: filesystem::identity(&opened.file).unwrap(),
                    phase: Phase::Installed {
                        identity,
                        sha256: sha(base_bytes),
                    },
                },
                &opened,
                &Cancellation::new(),
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
            conflict: Some("b".repeat(64)),
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
            merge: None,
            desired_name: target.into(),
            copy_name: (choice == ResolutionChoice::KeepBoth)
                .then(|| "report (remote copy).txt".into()),
            copy_id: (choice == ResolutionChoice::KeepBoth).then(|| "reserved-copy".into()),
        }
    }
    fn begin(&self, plan: Plan) -> Intent {
        self.begin_with_id(plan, "a".repeat(64))
    }
    fn begin_with_id(&self, plan: Plan, id: String) -> Intent {
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
            id,
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
        let (record, _) =
            preference_candidate(&fixture, &mut remote, ConflictPreference::KeepLocal);
        {
            let root = Root::open(&fixture.root).unwrap();
            let comparison =
                crate::comparison::inspect(&root, &mut remote, &record, &Cancellation::new())
                    .unwrap();
            assert_eq!(
                comparison.content,
                crate::ComparisonContent::Available {
                    local: b"local edit".to_vec(),
                    remote: b"remote edit".to_vec()
                }
            );
            assert_eq!(remote.effects, 0);
        }
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
                b"remote edit".as_slice()
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

#[test]
fn c09_fresh_classification_selects_and_executes_both_independent_combinations() {
    for local_rename in [true, false] {
        let fixture = Fixture::new(if local_rename { b"base" } else { b"local edit" });
        let mut remote = Provider::new(if local_rename {
            b"remote edit"
        } else {
            b"base"
        });
        if local_rename {
            fs::rename(
                fixture.root.join("report.txt"),
                fixture.root.join("renamed.txt"),
            )
            .unwrap();
        } else {
            remote.objects.get_mut("file").unwrap().name = "renamed.txt".into();
        }
        let inventory = RemoteInventory {
            account_id: fixture.scope.account_id.clone(),
            root_id: "root".into(),
            entries: vec![remote.observe("file").unwrap().item],
        };
        let root = Root::open(&fixture.root).unwrap();
        let cancel = Cancellation::new();
        let store = Store::open(&fixture.db).unwrap();
        let local = crate::local::scan(&root, &cancel).unwrap();
        let bases = fixture.bases();
        let prepared =
            crate::changes::prepare(&inventory, &root, &bases, &local, &mut remote, &cancel)
                .unwrap();
        let selected = planning::candidate(
            &inventory.entries[0],
            &prepared.entries[0],
            &fixture.base,
            &local,
        )
        .expect("fresh independently classified C09");
        let plan = Plan {
            conflict: None,
            origin: selected.origin,
            target: selected.target.clone(),
            local_fingerprint: selected.fingerprint,
            remote: remote.observe("file").unwrap(),
            choice: selected.choice,
            base: Some(fixture.base.clone()),
            merge: None,
            desired_name: selected
                .target
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .into(),
            copy_name: None,
            copy_id: None,
        };
        let ctx = Context {
            client: "client",
            scope: &prepared,
            root: &root,
            store: &store,
            cancel: &cancel,
        };
        let intent = execution::begin(&ctx, plan, "c".repeat(64)).unwrap();
        execution::run(&ctx, &mut remote, intent).unwrap();
        assert_eq!(
            fs::read(fixture.root.join("renamed.txt")).unwrap(),
            if local_rename {
                b"remote edit"
            } else {
                b"local edit".as_slice()
            }
        );
        assert_eq!(remote.objects.len(), 1);
        assert_eq!(remote.objects["file"].name, "renamed.txt");
        assert_eq!(crate::local::scan(&root, &cancel).unwrap().entries.len(), 1);
    }
}

/// Runs the actual collector, durable pending journal, consent store and selector.
fn preference_candidate(
    fixture: &Fixture,
    remote: &mut Provider,
    rule: ConflictPreference,
) -> (crate::ConflictRecord, planning::Candidate) {
    let root = Root::open(&fixture.root).unwrap();
    let cancel = Cancellation::new();
    let local = crate::local::scan(&root, &cancel).unwrap();
    let inventory = RemoteInventory {
        account_id: fixture.scope.account_id.clone(),
        root_id: "root".into(),
        entries: vec![remote.observe("file").unwrap().item],
    };
    let plan =
        crate::changes::prepare(&inventory, &root, &fixture.bases(), &local, remote, &cancel)
            .unwrap();
    let reconciliation =
        crate::state::Reconciliation::new(local, inventory, plan, fixture.bases(), vec![], None);
    let scope = crate::state::CheckpointScope {
        client: "client",
        account: &fixture.scope.account_id,
        remote: "root",
        local: &fixture.root,
    };

    let folder = crate::FolderConfiguration {
        account_id: fixture.scope.account_id.clone(),
        remote: crate::DriveFolder {
            id: "root".into(),
            name: "Root".into(),
        },
        local: fixture.root.clone(),
        local_display: fixture.root.clone(),
    };
    crate::configuration::identity::ensure(&folder, "client").unwrap();
    let path = crate::configuration::identity::metadata_path(
        &folder,
        "client",
        "conflict-preferences.sqlite3",
    )
    .unwrap();
    let preferences = crate::state::Preferences::open(&path, "client", &folder).unwrap();
    assert_eq!(preferences.load().unwrap(), ConflictPreference::Ask);
    preferences.save(rule).unwrap();
    drop(preferences);
    let restored = crate::state::Preferences::open(&path, "client", &folder)
        .unwrap()
        .load()
        .unwrap();
    let mut history =
        crate::state::ConflictHistory::open(&fixture.directory.join("history.sqlite3")).unwrap();
    history
        .record_pending(&scope, &reconciliation, &cancel)
        .unwrap();
    let record = history.records(&scope).unwrap().remove(0);
    let request = restored.request(&record).unwrap();
    let selected = preferences_candidate(
        &reconciliation.remote.entries[0],
        &reconciliation.plan.entries[0],
        &fixture.base,
        &reconciliation.local,
        &request,
    )
    .unwrap();
    (record, selected)
}

#[test]
fn p04_opt_in_rules_preserve_both_versions_and_replay_after_reopen() {
    for rule in [
        ConflictPreference::KeepLocal,
        ConflictPreference::KeepRemote,
        ConflictPreference::KeepBoth,
    ] {
        let fixture = Fixture::new(b"local edit");
        let mut remote = Provider::new(b"remote edit");
        let (record, selected) = preference_candidate(&fixture, &mut remote, rule);
        assert!(ConflictPreference::Ask.request(&record).is_none());
        let mut plan = fixture.plan(&mut remote, selected.choice, "report.txt", "report.txt");
        plan.conflict = Some(record.conflict_id);
        plan.local_fingerprint = selected.fingerprint;
        fixture.begin(plan);
        let report = fixture.run(&mut remote).unwrap();
        let expected = if rule == ConflictPreference::KeepRemote {
            b"remote edit".as_slice()
        } else {
            b"local edit".as_slice()
        };
        assert_eq!(fs::read(fixture.root.join("report.txt")).unwrap(), expected);
        assert_eq!(remote.objects["file"].bytes, expected);
        let preserved: Vec<_> = report
            .preserved_paths
            .iter()
            .map(|p| fs::read(fixture.root.join(p)).unwrap())
            .collect();
        assert!(preserved.contains(&b"local edit".to_vec()));
        assert!(preserved.contains(&b"remote edit".to_vec()));
        let effects = remote.effects;
        assert_eq!(fixture.run(&mut remote).unwrap(), report);
        assert_eq!(remote.effects, effects);
    }
}

#[test]
fn p04_p06_preferences_reject_unknown_deletion_and_stale_evidence() {
    let fixture = Fixture::new(b"local edit");
    let mut remote = Provider::new(b"remote edit");
    let (record, selected) =
        preference_candidate(&fixture, &mut remote, ConflictPreference::KeepLocal);
    for diagnosis in [
        crate::ChangeKind::Unknown,
        crate::ChangeKind::LocalMissingRemoteChanged,
        crate::ChangeKind::LocationConflict,
        crate::ChangeKind::Unsupported,
        crate::ChangeKind::AccessDenied,
    ] {
        let mut blocked = record.clone();
        blocked.diagnosis = diagnosis;
        assert!(ConflictPreference::KeepLocal.request(&blocked).is_none());
    }
    let mut missing = record.clone();
    missing.local_fingerprint = None;
    assert!(ConflictPreference::KeepBoth.request(&missing).is_none());
    let plan = fixture.plan(&mut remote, selected.choice, "report.txt", "report.txt");
    fixture.begin(plan);
    remote.objects.get_mut("file").unwrap().version += 1;
    assert!(fixture.run(&mut remote).is_err());
    assert_eq!(remote.effects, 0);
    assert_eq!(
        fs::read(fixture.root.join("report.txt")).unwrap(),
        b"local edit"
    );
    assert_eq!(remote.objects["file"].bytes, b"remote edit");
}

#[test]
fn p04_preference_decision_recovers_uncertain_remote_effect_without_duplicates() {
    let fixture = Fixture::new(b"local edit");
    let mut remote = Provider::new(b"remote edit");
    let (_, selected) = preference_candidate(&fixture, &mut remote, ConflictPreference::KeepBoth);
    let plan = fixture.plan(&mut remote, selected.choice, "report.txt", "report.txt");
    fixture.begin(plan);
    remote.lose_update = true;
    assert!(fixture.run(&mut remote).is_err());
    remote.lose_update = false;
    remote.now += 3600;
    let report = fixture.run(&mut remote).unwrap();
    assert_eq!(remote.effects, 1);
    assert_eq!(remote.copies, 1);
    assert_eq!(fixture.run(&mut remote).unwrap(), report);
    assert_eq!(remote.effects, 1);
}

/// The preview uses the same real root and durable conflict journal as decisions.
#[test]
fn comparison_preserves_versions_and_pending_history_across_reopen_and_repeat() {
    let fixture = Fixture::new(b"same\nlocal\nend\n");
    let mut remote = Provider::new(b"same\nremote\nend\n");
    let (record, _) = preference_candidate(&fixture, &mut remote, ConflictPreference::KeepLocal);
    let cancel = Cancellation::new();
    for _ in 0..2 {
        let root = Root::open(&fixture.root).unwrap();
        let result = crate::comparison::inspect(&root, &mut remote, &record, &cancel).unwrap();
        assert_eq!(
            result.content,
            crate::ComparisonContent::Available {
                local: b"same\nlocal\nend\n".to_vec(),
                remote: b"same\nremote\nend\n".to_vec()
            }
        );
        assert_eq!(
            fs::read(fixture.root.join("report.txt")).unwrap(),
            b"same\nlocal\nend\n"
        );
        let history =
            crate::state::ConflictHistory::open(&fixture.directory.join("history.sqlite3"))
                .unwrap();
        assert_eq!(
            history
                .records(&crate::state::CheckpointScope {
                    client: "client",
                    account: &fixture.scope.account_id,
                    remote: "root",
                    local: &fixture.root
                })
                .unwrap(),
            vec![record.clone()]
        );
        assert!(Store::open(&fixture.db)
            .unwrap()
            .resolutions("client", &fixture.scope)
            .unwrap()
            .is_empty());
    }
    assert_eq!(remote.effects, 0);
    assert_eq!(remote.copies, 0);
}

#[test]
fn comparison_rejects_offline_corrupt_media_and_concurrent_versions_without_effects() {
    for fault in 1..=4 {
        let fixture = Fixture::new(b"local");
        let mut remote = Provider::new(b"remote");
        let (record, _) =
            preference_candidate(&fixture, &mut remote, ConflictPreference::KeepLocal);
        remote.preview_fault = fault;
        remote.preview_path = Some(fixture.root.join("report.txt"));
        let root = Root::open(&fixture.root).unwrap();
        assert!(
            crate::comparison::inspect(&root, &mut remote, &record, &Cancellation::new()).is_err()
        );
        assert_eq!(
            fs::read(fixture.root.join("report.txt")).unwrap(),
            if fault == 4 {
                b"concurrent".as_slice()
            } else {
                b"local"
            }
        );
        assert_eq!(remote.effects, 0);
        assert_eq!(remote.objects["file"].bytes, b"remote");
    }
}

#[test]
fn comparison_bounds_content_and_rejects_stale_cancelled_and_symlink_inputs() {
    let fixture = Fixture::new(b"local");
    let mut remote = Provider::new(&vec![b'x'; crate::comparison::LIMIT as usize + 1]);
    let (record, _) = preference_candidate(&fixture, &mut remote, ConflictPreference::KeepLocal);
    remote.preview_fault = 1;
    let root = Root::open(&fixture.root).unwrap();
    assert_eq!(
        crate::comparison::inspect(&root, &mut remote, &record, &Cancellation::new())
            .unwrap()
            .content,
        crate::ComparisonContent::MetadataOnly
    );
    let mut stale = record.clone();
    stale.remote_version = Some(1);
    assert!(crate::comparison::inspect(&root, &mut remote, &stale, &Cancellation::new()).is_err());
    let cancel = Cancellation::new();
    cancel.cancel();
    assert!(crate::comparison::inspect(&root, &mut remote, &record, &cancel).is_err());
    fs::rename(
        fixture.root.join("report.txt"),
        fixture.root.join("original.txt"),
    )
    .unwrap();
    std::os::unix::fs::symlink("original.txt", fixture.root.join("report.txt")).unwrap();
    assert!(crate::comparison::inspect(&root, &mut remote, &record, &Cancellation::new()).is_err());
    assert_eq!(
        fs::read(fixture.root.join("original.txt")).unwrap(),
        b"local"
    );
}

/// Exercises the production comparison and candidate builder with retained B.
fn text_candidate(
    fixture: &Fixture,
    remote: &mut Provider,
) -> Result<crate::TextMergeCandidate, EngineError> {
    let root = Root::open(&fixture.root)?;
    let store = Store::open(&fixture.db)?;
    let version = remote.observe("file")?;
    let cancel = Cancellation::new();
    let record = crate::ConflictRecord {
        conflict_id: "b".repeat(64),
        remote_id: Some("file".into()),
        remote_parent_id: Some("root".into()),
        remote_name: Some("report.txt".into()),
        relative_path: Some("report.txt".into()),
        diagnosis: crate::ChangeKind::ConcurrentChanges,
        status: crate::ConflictStatus::Pending,
        local_size: None,
        local_modified_unix: None,
        remote_size: version.item.size,
        local_fingerprint: filesystem::fingerprint_root(
            &root,
            std::path::Path::new("report.txt"),
            &cancel,
        )?,
        remote_version: version.item.version,
        outcomes: vec![],
    };
    let bytes = store
        .common_base("client", &fixture.scope, "file", &fixture.base)?
        .ok_or(EngineError::ObservationUnknown)?;
    let comparison = crate::comparison::inspect(&root, remote, &record, &cancel)?;
    text::candidate(
        "client",
        &fixture.scope,
        &record,
        fixture.base.clone(),
        version,
        bytes,
        comparison,
    )
}
fn text_plan(fixture: &Fixture, remote: &mut Provider) -> Plan {
    let candidate = text_candidate(fixture, remote).unwrap();
    assert_eq!(candidate.base(), b"a\nb\n");
    assert_eq!(candidate.local(), b"A\nb\n");
    assert_eq!(candidate.remote(), b"a\nB\n");
    assert_eq!(candidate.result(), b"A\nB\n");
    let request = candidate.accept_reviewed();
    let mut plan = fixture.plan(
        remote,
        ResolutionChoice::KeepLocal,
        "report.txt",
        "report.txt",
    );
    plan.choice = ResolutionChoice::MergeText;
    plan.merge = request.merge;
    plan
}
#[test]
fn p05_reviewed_merge_preserves_three_versions_confirms_new_base_and_replays_once() {
    let fixture = Fixture::with_base(b"A\nb\n", b"a\nb\n");
    let mut remote = Provider::new(b"a\nB\n");
    let plan = text_plan(&fixture, &mut remote);
    assert_eq!(remote.effects, 0);
    assert!(fixture.intent_or_none().is_none());
    fixture.begin(plan);
    let report = fixture.run(&mut remote).unwrap();
    assert_eq!(
        fs::read(fixture.root.join("report.txt")).unwrap(),
        b"A\nB\n"
    );
    assert_eq!(remote.objects["file"].bytes, b"A\nB\n");
    let versions: Vec<_> = report
        .preserved_paths
        .iter()
        .map(|p| fs::read(fixture.root.join(p)).unwrap())
        .collect();
    for bytes in [b"a\nb\n", b"A\nb\n", b"a\nB\n", b"A\nB\n"] {
        assert!(versions.contains(&bytes.to_vec()));
    }
    use std::os::unix::fs::PermissionsExt;
    for path in report.preserved_paths.iter().filter(|p| {
        **p != std::path::Path::new("report.txt").with_file_name(fixture.intent().backup())
    }) {
        assert_eq!(
            fs::metadata(fixture.root.join(path))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    let base = fixture.bases().remove("file").unwrap();
    assert_eq!(
        Store::open(&fixture.db)
            .unwrap()
            .common_base("client", &fixture.scope, "file", &base)
            .unwrap(),
        Some(b"A\nB\n".to_vec())
    );
    assert_eq!(fixture.run(&mut remote).unwrap(), report);
    assert_eq!(remote.effects, 1);
}
impl Fixture {
    fn intent_or_none(&self) -> Option<Intent> {
        Store::open(&self.db)
            .unwrap()
            .resolutions("client", &self.scope)
            .unwrap()
            .into_iter()
            .next()
    }
}
#[test]
fn p05_lost_remote_response_reopens_and_reconciles_without_duplicate_upload() {
    let fixture = Fixture::with_base(b"A\nb\n", b"a\nb\n");
    let mut remote = Provider::new(b"a\nB\n");
    fixture.begin(text_plan(&fixture, &mut remote));
    remote.lose_update = true;
    assert!(fixture.run(&mut remote).is_err());
    assert_eq!(fixture.intent().stage, Stage::RemotePending);
    assert_eq!(remote.effects, 1);
    assert!(fixture.bases().is_empty());
    remote.now += 1000;
    fixture.run(&mut remote).unwrap();
    assert_eq!(remote.effects, 1);
    assert_eq!(fixture.intent().stage, Stage::Confirmed);
    assert_eq!(
        fs::read(fixture.root.join("report.txt")).unwrap(),
        b"A\nB\n"
    );
}
#[test]
fn p05_offline_and_snapshot_races_never_apply_the_candidate() {
    for fault in 1..=4 {
        let fixture = Fixture::with_base(b"A\nb\n", b"a\nb\n");
        let mut remote = Provider::new(b"a\nB\n");
        remote.preview_fault = fault;
        remote.preview_path = Some(fixture.root.join("report.txt"));
        assert!(text_candidate(&fixture, &mut remote).is_err());
        assert_eq!(remote.effects, 0);
        assert!(fixture.intent_or_none().is_none());
    }
}
#[test]
fn p05_local_remote_and_scope_changes_after_review_block_application() {
    for fault in 0..3 {
        let fixture = Fixture::with_base(b"A\nb\n", b"a\nb\n");
        let mut remote = Provider::new(b"a\nB\n");
        let mut plan = text_plan(&fixture, &mut remote);
        if fault == 2 {
            plan.merge.as_mut().unwrap().scope.account_id = AccountId("other".into());
        }
        fixture.begin(plan);
        if fault == 0 {
            fs::write(fixture.root.join("report.txt"), b"concurrent edit").unwrap();
        }
        if fault == 1 {
            remote.objects.get_mut("file").unwrap().version += 1;
        }
        assert!(fixture.run(&mut remote).is_err());
        assert_eq!(remote.effects, 0);
    }
}
#[test]
fn p05_missing_corrupt_base_and_unsupported_text_remain_unaccepted() {
    for fault in 0..4 {
        let fixture = Fixture::with_base(b"A\nb\n", b"a\nb\n");
        let mut remote = Provider::new(if fault == 2 { b"C\nb\n" } else { b"a\nB\n" });
        if fault == 0 {
            rusqlite::Connection::open(&fixture.db)
                .unwrap()
                .execute_batch("DELETE FROM base_content")
                .unwrap();
        }
        if fault == 1 {
            rusqlite::Connection::open(&fixture.db)
                .unwrap()
                .execute_batch("UPDATE base_content SET bytes=X'00'")
                .unwrap();
        }
        if fault == 3 {
            fs::write(fixture.root.join("report.txt"), b"A\0\nb\n").unwrap();
        }
        assert!(text_candidate(&fixture, &mut remote).is_err());
        assert_eq!(remote.effects, 0);
        assert!(fixture.intent_or_none().is_none());
    }
}

#[test]
fn p05_durable_boundaries_recover_after_sqlite_failures_without_duplicate_effects() {
    for boundary in [
        "Preserved",
        "RemotePending",
        "RemoteVerified",
        "LocalPending",
        "Verified",
        "Confirmed",
    ] {
        let fixture = Fixture::with_base(b"A\nb\n", b"a\nb\n");
        let mut remote = Provider::new(b"a\nB\n");
        fixture.begin(text_plan(&fixture, &mut remote));
        let db = rusqlite::Connection::open(&fixture.db).unwrap();
        db.execute_batch(&format!("CREATE TRIGGER fail BEFORE UPDATE ON resolutions WHEN json_extract(NEW.record,'$.stage')='{boundary}' BEGIN SELECT RAISE(ABORT,'injected'); END;")).unwrap();
        assert_eq!(
            fixture.run(&mut remote),
            Err(EngineError::StateStorage),
            "{boundary}"
        );
        assert_ne!(fixture.intent().stage, Stage::Confirmed);
        let intent = fixture.intent();
        for (kind, bytes) in [("base", b"a\nb\n"), ("merged", b"A\nB\n")] {
            assert_eq!(
                fs::read(fixture.root.join(format!(
                    "DriveSync backup conflict-{}-{kind}.bak",
                    intent.id
                )))
                .unwrap(),
                bytes
            );
        }
        db.execute_batch("DROP TRIGGER fail").unwrap();
        remote.now += 1000;
        fixture.run(&mut remote).unwrap();
        assert_eq!(remote.effects, 1, "{boundary}");
        assert_eq!(fixture.intent().stage, Stage::Confirmed);
        assert_eq!(
            fs::read(fixture.root.join("report.txt")).unwrap(),
            b"A\nB\n"
        );
    }
}
#[test]
fn p05_corrupt_preserved_ancestor_and_local_race_after_upload_block_confirmation() {
    for corrupt in [true, false] {
        let fixture = Fixture::with_base(b"A\nb\n", b"a\nb\n");
        let mut remote = Provider::new(b"a\nB\n");
        fixture.begin(text_plan(&fixture, &mut remote));
        if corrupt {
            remote.lose_update = true;
            assert!(fixture.run(&mut remote).is_err());
            fs::write(
                fixture.root.join(format!(
                    "DriveSync backup conflict-{}-base.bak",
                    fixture.intent().id
                )),
                b"corrupt",
            )
            .unwrap();
            remote.now += 1000;
        } else {
            remote.local_after_update = Some(fixture.root.join("report.txt"));
        }
        assert!(fixture.run(&mut remote).is_err());
        assert_eq!(remote.effects, 1);
        assert!(fixture.bases().is_empty());
        assert_ne!(fixture.intent().stage, Stage::Confirmed);
    }
}

/// Atomic editor saves change inode even when the path remains managed. Only
/// exact explicit consent permits binding the replacement to the existing ID.
#[test]
fn editor_replacement_choices_preserve_versions_and_replay() {
    for (choice, remote_changed) in [
        ResolutionChoice::KeepLocal,
        ResolutionChoice::KeepRemote,
        ResolutionChoice::KeepBoth,
    ]
    .into_iter()
    .flat_map(|choice| [false, true].map(|changed| (choice, changed)))
    {
        let fixture = Fixture::new(b"base");
        let old_file = File::open(fixture.root.join("report.txt")).unwrap();
        fs::write(fixture.root.join(".editor-save.tmp"), b"editor version 1").unwrap();
        fs::rename(
            fixture.root.join(".editor-save.tmp"),
            fixture.root.join("report.txt"),
        )
        .unwrap();
        let mut remote = Provider::new(b"base");
        remote.objects.get_mut("file").unwrap().version = if remote_changed { 2 } else { 1 };
        let root = Root::open(&fixture.root).unwrap();
        let cancel = Cancellation::new();
        let inventory = RemoteInventory {
            account_id: fixture.scope.account_id.clone(),
            root_id: "root".into(),
            entries: vec![remote.observe("file").unwrap().item],
        };
        let local = crate::local::scan(&root, &cancel).unwrap();
        let prepared = crate::changes::prepare(
            &inventory,
            &root,
            &fixture.bases(),
            &local,
            &mut remote,
            &cancel,
        )
        .unwrap();
        assert_eq!(
            prepared.entries[0].action,
            crate::PlanAction::Blocked(if remote_changed {
                crate::ConflictReason::ConcurrentIdentityChanges
            } else {
                crate::ConflictReason::LocalIdentityChanged
            })
        );
        let observation = crate::state::Reconciliation::new(
            local,
            inventory,
            prepared,
            fixture.bases(),
            vec![],
            None,
        );
        let scope = crate::state::CheckpointScope {
            client: "client",
            account: &fixture.scope.account_id,
            remote: "root",
            local: &fixture.root,
        };
        let mut history =
            crate::state::ConflictHistory::open(&fixture.directory.join("history.sqlite3"))
                .unwrap();
        history
            .record_pending(&scope, &observation, &cancel)
            .unwrap();
        let record = history.records(&scope).unwrap().remove(0);
        assert_eq!(
            record.diagnosis,
            if remote_changed {
                crate::ChangeKind::ConcurrentIdentityChanges
            } else {
                crate::ChangeKind::LocalIdentityChanged
            }
        );
        assert!(ConflictPreference::KeepLocal.request(&record).is_none());
        let request = ResolutionRequest::from_record(&record, choice).unwrap();
        let comparison = crate::comparison::inspect(&root, &mut remote, &record, &cancel).unwrap();
        assert_eq!(
            comparison.content,
            crate::ComparisonContent::Available {
                local: b"editor version 1".to_vec(),
                remote: b"base".to_vec()
            }
        );
        drop(root);
        let mut plan = fixture.plan(&mut remote, request.choice, "report.txt", "report.txt");
        plan.conflict = Some(record.conflict_id.clone());
        // Automatic reconciliation must retain the strict identity boundary.
        let mut automatic = plan.clone();
        automatic.conflict = None;
        assert!(!automatic.explicit_replacement());
        fixture.begin(plan);
        let report = fixture.run(&mut remote).unwrap();
        let expected = if choice == ResolutionChoice::KeepRemote {
            b"base".as_slice()
        } else {
            b"editor version 1".as_slice()
        };
        assert_eq!(fs::read(fixture.root.join("report.txt")).unwrap(), expected);
        assert_eq!(remote.objects["file"].bytes, expected);
        let preserved: Vec<_> = report
            .preserved_paths
            .iter()
            .map(|p| fs::read(fixture.root.join(p)).unwrap())
            .collect();
        assert!(preserved.contains(&b"editor version 1".to_vec()));
        assert!(preserved.contains(&b"base".to_vec()));
        let effects = remote.effects;
        assert_eq!(fixture.run(&mut remote).unwrap(), report);
        assert_eq!(remote.effects, effects);
        history
            .complete_resolution(&scope, &fixture.intent())
            .unwrap();
        assert!(history.records(&scope).unwrap().is_empty());
        drop(old_file);
    }
}

#[test]
fn editor_replacement_requires_consent_and_rejects_stale_versions() {
    for fault in ["automatic", "moved", "renamed", "local-race", "remote-race"] {
        let fixture = Fixture::new(b"base");
        let _old = File::open(fixture.root.join("report.txt")).unwrap();
        fs::write(fixture.root.join(".editor-save.tmp"), b"replacement").unwrap();
        fs::rename(
            fixture.root.join(".editor-save.tmp"),
            fixture.root.join("report.txt"),
        )
        .unwrap();
        let mut remote = Provider::new(b"remote");
        let mut plan = fixture.plan(
            &mut remote,
            ResolutionChoice::KeepLocal,
            "report.txt",
            "report.txt",
        );
        match fault {
            "automatic" => plan.conflict = None,
            "moved" => {
                plan.base
                    .as_mut()
                    .unwrap()
                    .location
                    .as_mut()
                    .unwrap()
                    .relative_path = "other/report.txt".into()
            }
            "renamed" => plan.remote.item.name = "other.txt".into(),
            _ => {}
        }
        if matches!(fault, "automatic" | "moved" | "renamed") {
            let root = Root::open(&fixture.root).unwrap();
            let store = Store::open(&fixture.db).unwrap();
            let cancel = Cancellation::new();
            let ctx = Context {
                client: "client",
                scope: &fixture.scope,
                root: &root,
                store: &store,
                cancel: &cancel,
            };
            assert!(execution::begin(&ctx, plan, "c".repeat(64)).is_err());
        } else {
            fixture.begin(plan);
            if fault == "local-race" {
                fs::write(fixture.root.join("report.txt"), b"newer local").unwrap();
            } else {
                remote.objects.get_mut("file").unwrap().version += 1;
            }
            assert!(fixture.run(&mut remote).is_err());
        }
        assert_eq!(remote.effects, 0);
        assert_eq!(remote.objects["file"].bytes, b"remote");
        assert_eq!(
            fs::read(fixture.root.join("report.txt")).unwrap(),
            if fault == "local-race" {
                b"newer local".as_slice()
            } else {
                b"replacement".as_slice()
            }
        );
    }
}

#[test]
fn editor_replacement_recovers_uncertain_effect_without_duplicate_update() {
    let fixture = Fixture::new(b"base");
    let _old = File::open(fixture.root.join("report.txt")).unwrap();
    fs::write(fixture.root.join(".save.tmp"), b"edited").unwrap();
    fs::rename(
        fixture.root.join(".save.tmp"),
        fixture.root.join("report.txt"),
    )
    .unwrap();
    let mut remote = Provider::new(b"remote");
    let plan = fixture.plan(
        &mut remote,
        ResolutionChoice::KeepBoth,
        "report.txt",
        "report.txt",
    );
    fixture.begin(plan);
    remote.lose_update = true;
    assert!(fixture.run(&mut remote).is_err());
    assert_eq!(remote.effects, 1);
    assert_eq!(remote.copies, 1);
    assert!(fixture.bases().is_empty());
    assert_eq!(fixture.run(&mut remote), Err(EngineError::UploadDeferred));
    remote.now = 2000;
    let report = fixture.run(&mut remote).unwrap();
    assert_eq!((remote.effects, remote.copies), (1, 1));
    assert_eq!(fs::read(fixture.root.join(report.path)).unwrap(), b"edited");
    assert_eq!(fixture.intent().stage, Stage::Confirmed);
}

#[test]
fn repeated_same_inode_edits_select_automatic_upload_only_against_unchanged_remote() {
    let mut fixture = Fixture::new(b"edited");
    let mut remote = Provider::new(b"base");
    remote.objects.get_mut("file").unwrap().version = 1;
    for (index, bytes) in [b"first save".as_slice(), b"second save".as_slice()]
        .into_iter()
        .enumerate()
    {
        fs::write(fixture.root.join("report.txt"), bytes).unwrap();
        let root = Root::open(&fixture.root).unwrap();
        let cancel = Cancellation::new();
        let local = crate::local::scan(&root, &cancel).unwrap();
        let inventory = RemoteInventory {
            account_id: fixture.scope.account_id.clone(),
            root_id: "root".into(),
            entries: vec![remote.observe("file").unwrap().item],
        };
        let prepared = crate::changes::prepare(
            &inventory,
            &root,
            &fixture.bases(),
            &local,
            &mut remote,
            &cancel,
        )
        .unwrap();
        let selected = planning::candidate(
            &inventory.entries[0],
            &prepared.entries[0],
            &fixture.base,
            &local,
        )
        .expect("same-object local-only edit");
        let mut changed = inventory.entries[0].clone();
        changed.version = Some(changed.version.unwrap() + 1);
        assert!(
            planning::candidate(&changed, &prepared.entries[0], &fixture.base, &local).is_none()
        );
        let mut aliases = local.clone();
        aliases.entries.insert(
            "alias.txt".into(),
            local.entries[std::path::Path::new("report.txt")].clone(),
        );
        assert!(planning::candidate(
            &inventory.entries[0],
            &prepared.entries[0],
            &fixture.base,
            &aliases
        )
        .is_none());
        drop(root);
        let mut plan = fixture.plan(&mut remote, selected.choice, "report.txt", "report.txt");
        plan.conflict = None;
        fixture.begin_with_id(plan, if index == 0 { "a" } else { "b" }.repeat(64));
        let report = fixture.run(&mut remote).unwrap();
        assert_eq!(remote.objects["file"].bytes, bytes);
        assert_eq!(fs::read(fixture.root.join(report.path)).unwrap(), bytes);
        fixture.base = fixture.bases().remove("file").unwrap();
    }
}

/// Missing advisory-file-lock capability must not disable the verified snapshot
/// protocol. Root/account exclusion, hashes, conditional writes and replay remain.
#[test]
fn unsupported_file_locks_preserve_comparison_resolution_and_recovery() {
    filesystem::with_unsupported_file_locks(|| {
        p03_all_choices_preserve_versions_confirm_b_and_replay_without_effects();
        editor_replacement_choices_preserve_versions_and_replay();
        editor_replacement_requires_consent_and_rejects_stale_versions();
        editor_replacement_recovers_uncertain_effect_without_duplicate_update();
        repeated_same_inode_edits_select_automatic_upload_only_against_unchanged_remote();
        comparison_preserves_versions_and_pending_history_across_reopen_and_repeat();
        comparison_rejects_offline_corrupt_media_and_concurrent_versions_without_effects();
        comparison_bounds_content_and_rejects_stale_cancelled_and_symlink_inputs();
        uncertain_update_recovers_after_reopen_without_duplicate_mutation();
        stale_pre_effect_evidence_aborts_and_retains_originals_and_old_b();
        concurrent_local_edit_after_remote_effect_is_preserved_and_never_confirms_b();
        displaced_inode_edits_after_cutover_are_preserved_without_b_confirmation();
        interrupted_local_publication_reopens_and_recognizes_its_exact_inode();
        destination_collisions_and_snapshot_symlinks_never_replace_other_files();
        cancelled_start_does_not_publish_authority_and_root_replacement_stops_replay();
        final_baseline_failure_rolls_back_confirmation_and_replays_local_installation();
        p05_reviewed_merge_preserves_three_versions_confirms_new_base_and_replays_once();
        p05_corrupt_preserved_ancestor_and_local_race_after_upload_block_confirmation();
    });
}

#[test]
fn supported_file_lock_contention_still_blocks_resolution_without_effects() {
    let fixture = Fixture::new(b"local");
    let mut remote = Provider::new(b"remote");
    let plan = fixture.plan(
        &mut remote,
        ResolutionChoice::KeepLocal,
        "report.txt",
        "report.txt",
    );
    fixture.begin(plan);
    let writer = File::open(fixture.root.join("report.txt")).unwrap();
    rustix::fs::flock(
        &writer,
        rustix::fs::FlockOperation::NonBlockingLockExclusive,
    )
    .unwrap();
    assert_eq!(fixture.run(&mut remote), Err(EngineError::LocalWriting));
    assert_eq!(remote.effects, 0);
    assert_eq!(fs::read(fixture.root.join("report.txt")).unwrap(), b"local");
}

/// Uses a fresh exported directory and the real FUSE backend with a simulated
/// conditional remote. No Google account, personal tree or permission changes.
#[test]
#[ignore = "requires a fresh disposable document-portal directory export"]
fn real_document_portal_supports_verified_conflict_decisions() {
    use std::os::unix::fs::OpenOptionsExt;
    let parent = PathBuf::from(
        std::env::var_os("DRIVESYNC_RESOLUTION_FIXTURE_PARENT")
            .expect("explicit document portal directory"),
    );
    assert!(parent.is_absolute());
    assert!(parent.starts_with("/run/flatpak/doc") || parent.starts_with("/run/user"));
    let probe = parent.join(format!(
        ".drivesync-capability-{}",
        oauth2::CsrfToken::new_random().secret()
    ));
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&probe)
        .unwrap();
    assert!(
        matches!(
            rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockShared),
            Err(rustix::io::Errno::NOSYS | rustix::io::Errno::OPNOTSUPP)
        ),
        "the test must exercise an actual unsupported flock backend"
    );
    drop(file);
    fs::remove_file(probe).unwrap();
    // Run without injected errors: every effect uses the real exported view.
    p03_all_choices_preserve_versions_confirm_b_and_replay_without_effects();
    editor_replacement_choices_preserve_versions_and_replay();
    uncertain_update_recovers_after_reopen_without_duplicate_mutation();
    comparison_rejects_offline_corrupt_media_and_concurrent_versions_without_effects();
    displaced_inode_edits_after_cutover_are_preserved_without_b_confirmation();
}
