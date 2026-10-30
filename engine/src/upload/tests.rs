//! Isolated filesystem/SQLite and deterministic provider interruption oracles.
use super::{
    execution::{self, Remote},
    transport::{valid_session, Position},
    Intent,
};
use crate::{
    state::{Store, Uploads},
    AccountId, Cancellation, DownloadReport, DriveFolder, EngineError, FolderConfiguration,
    InventoryEntry, InventoryKind, RemoteInventory,
};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};
#[derive(Default)]
struct Fake {
    generated: usize,
    change_on_generate: Option<PathBuf>,
    change_after_chunk: Option<PathBuf>,
    creates: usize,
    chunks: usize,
    begins: usize,
    items: HashMap<String, InventoryEntry>,
    data: HashMap<String, Vec<u8>>,
    intents: HashMap<String, Intent>,
    fail_chunk: bool,
    fail_after_complete: bool,
    expire: bool,
    lost_folder: bool,
    cancel_after_chunk: bool,
    suspend_after_chunk: Option<crate::RootSuspension>,
    replace_root_after_chunk: Option<PathBuf>,
}
fn entry(op: &Intent) -> InventoryEntry {
    InventoryEntry {
        id: op.id.clone(),
        parent_id: op.parent.clone(),
        name: op.path.file_name().unwrap().to_str().unwrap().into(),
        mime_type: if op.directory {
            "application/vnd.google-apps.folder"
        } else {
            "application/octet-stream"
        }
        .into(),
        version: Some(1),
        size: if op.directory { None } else { Some(op.size) },
        md5_checksum: if op.directory {
            None
        } else {
            Some(op.md5.clone())
        },
        web_view_link: None,
        kind: if op.directory {
            InventoryKind::Folder
        } else {
            InventoryKind::Binary
        },
    }
}
impl Remote for Fake {
    fn configured(&mut self) -> Result<(), EngineError> {
        if self.chunks > 0 {
            if let Some(reason) = self.suspend_after_chunk {
                return Err(EngineError::RootSuspended(reason));
            }
        }
        if self.cancel_after_chunk && self.chunks > 0 {
            return Err(EngineError::Authentication(crate::AuthError::Cancelled));
        }
        Ok(())
    }
    fn generate(&mut self) -> Result<String, EngineError> {
        self.generated += 1;
        if let Some(path) = self.change_on_generate.take() {
            fs::write(path, b"changed after inventory").unwrap();
        }
        Ok(format!("generated_{}", self.generated))
    }
    fn children(&mut self, parent: &str, name: &str) -> Result<Vec<InventoryEntry>, EngineError> {
        Ok(self
            .items
            .values()
            .filter(|i| i.parent_id == parent && i.name == name)
            .cloned()
            .collect())
    }
    fn stat(&mut self, op: &Intent) -> Result<Option<InventoryEntry>, EngineError> {
        Ok(self.items.get(&op.id).cloned())
    }
    fn folder(&mut self, op: &Intent) -> Result<(), EngineError> {
        assert!(!self.items.contains_key(&op.id));
        self.creates += 1;
        self.items.insert(op.id.clone(), entry(op));
        if std::mem::take(&mut self.lost_folder) {
            Err(EngineError::Authentication(crate::AuthError::Network))
        } else {
            Ok(())
        }
    }
    fn begin(&mut self, op: &Intent) -> Result<String, EngineError> {
        self.begins += 1;
        self.intents.insert(op.id.clone(), op.clone());
        self.data.entry(op.id.clone()).or_default();
        Ok(format!(
            "https://www.googleapis.com/upload/drive/v3/files?upload_id={}",
            op.id
        ))
    }
    fn send(
        &mut self,
        op: &Intent,
        offset: Option<u64>,
        bytes: Vec<u8>,
    ) -> Result<Position, EngineError> {
        if offset.is_none() {
            if self.items.contains_key(&op.id) {
                return Ok(Position::Complete);
            }
            if std::mem::take(&mut self.expire) {
                self.data.remove(&op.id);
                return Ok(Position::Expired);
            }
            return Ok(Position::Offset(self.data[&op.id].len() as u64));
        }
        assert_eq!(offset.unwrap(), self.data[&op.id].len() as u64);
        self.data.get_mut(&op.id).unwrap().extend(bytes);
        self.chunks += 1;
        if let Some(path) = self.replace_root_after_chunk.take() {
            fs::rename(&path, path.with_file_name("detached-volume")).unwrap();
            fs::create_dir(path).unwrap();
        }
        if let Some(path) = self.change_after_chunk.take() {
            fs::write(path, b"writer resumed during upload").unwrap();
        }
        if self.data[&op.id].len() as u64 == op.size {
            self.creates += 1;
            self.items.insert(op.id.clone(), entry(op));
            if std::mem::take(&mut self.fail_after_complete) {
                return Err(EngineError::Authentication(crate::AuthError::Network));
            }
            Ok(Position::Complete)
        } else if std::mem::take(&mut self.fail_chunk) {
            Err(EngineError::Authentication(crate::AuthError::Network))
        } else {
            Ok(Position::Offset(self.data[&op.id].len() as u64))
        }
    }
    fn validate_parent(&mut self, _: &str) -> Result<(), EngineError> {
        Ok(())
    }
    fn remember(&mut self, _: &InventoryEntry) {}
}
struct Fixture {
    base: PathBuf,
    root: PathBuf,
    path: PathBuf,
    folder: FolderConfiguration,
}
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "drivesync-upload-{}",
            oauth2::CsrfToken::new_random().secret()
        ));
        let root = base.join("files");
        fs::create_dir_all(&root).unwrap();
        let path = base.join("private/state.sqlite3");
        let folder = FolderConfiguration {
            account_id: AccountId("account".into()),
            remote: DriveFolder {
                id: "root".into(),
                name: "Test".into(),
            },
            local: root.clone(),
            local_display: root.clone(),
        };
        let inv = RemoteInventory {
            account_id: folder.account_id.clone(),
            root_id: "root".into(),
            entries: vec![],
        };
        let plan = crate::planner::plan(&inv, &root, |_| crate::LocalObservation::Unknown).unwrap();
        Store::open(&path)
            .unwrap()
            .save("client", &inv, &plan, &Cancellation::new())
            .unwrap();
        let fixture = Self {
            base,
            root,
            path,
            folder,
        };
        fixture.uploads().enable(true).unwrap();
        fixture
    }
    fn uploads(&self) -> Uploads {
        Uploads::open(
            &self.path.with_file_name("uploads.sqlite3"),
            "client",
            &self.folder,
        )
        .unwrap()
    }
    fn run(
        &self,
        remote: &mut Fake,
        excluded: HashSet<PathBuf>,
    ) -> Result<DownloadReport, EngineError> {
        let uploads = self.uploads();
        let inv = RemoteInventory {
            account_id: self.folder.account_id.clone(),
            root_id: "root".into(),
            entries: remote.items.values().cloned().collect(),
        };
        // The facade has already committed the download observation root.
        let mut report = DownloadReport::default();
        execution::run(
            execution::Context {
                client: "client",
                root_path: &self.root,
                state_path: &self.path,
                inventory: &inv,
                uploads: &uploads,
                cancel: &Cancellation::new(),
                excluded,
            },
            remote,
            &mut report,
            |_| {},
        )?;
        Ok(report)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}
#[test]
fn opt_in_is_exactly_scoped_and_defaults_off() {
    let f = Fixture::new();
    let store = f.uploads();
    store.enable(false).unwrap();
    assert!(!store.enabled().unwrap());
    store.enable(true).unwrap();
    assert!(f.uploads().enabled().unwrap());
    let mut renamed = f.folder.clone();
    renamed.remote.name = "Another display name".into();
    renamed.local_display = f.base.join("display-only");
    assert!(Uploads::open(
        &f.path.with_file_name("uploads.sqlite3"),
        "client",
        &renamed
    )
    .unwrap()
    .enabled()
    .unwrap());

    let mut other = f.folder.clone();
    other.account_id = AccountId("other".into());
    for (client, folder) in [("other", &f.folder), ("client", &other)] {
        assert!(
            !Uploads::open(&f.path.with_file_name("uploads.sqlite3"), client, folder)
                .unwrap()
                .enabled()
                .unwrap()
        );
    }
    other = f.folder.clone();
    other.remote.id = "other".into();
    assert!(
        !Uploads::open(&f.path.with_file_name("uploads.sqlite3"), "client", &other)
            .unwrap()
            .enabled()
            .unwrap()
    );
    other = f.folder.clone();
    other.local = f.base.join("other");
    assert!(
        !Uploads::open(&f.path.with_file_name("uploads.sqlite3"), "client", &other)
            .unwrap()
            .enabled()
            .unwrap()
    );
}
#[test]
fn creates_nested_files_once_and_records_download_evidence() {
    let f = Fixture::new();
    fs::create_dir(f.root.join("nested")).unwrap();
    fs::write(f.root.join("nested/ação.txt"), b"abc").unwrap();
    fs::write(f.root.join("empty.txt"), b"").unwrap();
    let mut remote = Fake::default();
    let first = f.run(&mut remote, HashSet::new()).unwrap();
    assert!(first.issues.is_empty());
    assert_eq!(first.uploaded, 2);
    assert_eq!(first.created_remote_folders, 1);
    assert_eq!(remote.generated, 3);
    let chunks = remote.chunks;
    let second = f.run(&mut remote, HashSet::new()).unwrap();
    assert!(second.issues.is_empty());
    assert_eq!(second.uploaded, 0);
    assert_eq!(remote.chunks, chunks);
    assert_eq!(remote.generated, 3);
    let inv = RemoteInventory {
        account_id: f.folder.account_id.clone(),
        root_id: "root".into(),
        entries: remote.items.values().cloned().collect(),
    };
    let plan =
        crate::planner::plan(&inv, &f.root, |p| crate::planner::observe(&f.root, p)).unwrap();
    let mut store = Store::open(&f.path).unwrap();
    store
        .save("client", &inv, &plan, &Cancellation::new())
        .unwrap();
    struct Source;
    impl crate::download::Source for Source {
        fn cached_unchanged(&self, _: &InventoryEntry) -> bool {
            true
        }
        fn metadata(&mut self, i: &InventoryEntry) -> Result<InventoryEntry, EngineError> {
            Ok(i.clone())
        }
        fn media(&mut self, _: &str) -> Result<Box<dyn std::io::Read>, EngineError> {
            panic!("confirmed uploads must not download again")
        }
        fn configured(&mut self) -> Result<(), EngineError> {
            Ok(())
        }
    }
    let report = crate::download::run(
        crate::download::Context {
            client: "client",
            scope: &plan,
            store: &store,
            cancel: &Cancellation::new(),
        },
        &inv,
        &mut Source,
        |_| {},
    )
    .unwrap();
    assert!(report.issues.is_empty());
    assert_eq!(report.unchanged, 2);
}
#[test]
fn interrupted_chunks_lost_confirmation_and_expired_sessions_reuse_one_id() {
    for fault in 0..3 {
        let f = Fixture::new();
        fs::write(f.root.join("large.dat"), vec![7; 2 * 1024 * 1024 + 3]).unwrap();
        let mut remote = Fake {
            fail_chunk: fault != 1,
            fail_after_complete: fault == 1,
            ..Default::default()
        };
        let first = f.run(&mut remote, HashSet::new()).unwrap();
        assert_eq!(first.issues.len(), 1);
        assert_eq!(remote.generated, 1);
        if fault == 2 {
            remote.expire = true;
        }
        let second = f.run(&mut remote, HashSet::new()).unwrap();
        assert!(second.issues.is_empty());
        assert_eq!(second.uploaded, 1);
        assert_eq!(remote.generated, 1);
        assert_eq!(remote.creates, 1);
        assert_eq!(remote.data["generated_1"].len(), 2 * 1024 * 1024 + 3);
        assert!(f
            .run(&mut remote, HashSet::new())
            .unwrap()
            .issues
            .is_empty());
        assert_eq!(remote.creates, 1);
    }
}
#[test]
fn lost_folder_response_recovers_without_duplicate_folder_or_children() {
    let f = Fixture::new();
    fs::create_dir(f.root.join("nested")).unwrap();
    fs::write(f.root.join("nested/file.txt"), b"abc").unwrap();
    let mut remote = Fake {
        lost_folder: true,
        ..Default::default()
    };
    assert_eq!(f.run(&mut remote, HashSet::new()).unwrap().issues.len(), 1);
    assert!(f
        .run(&mut remote, HashSet::new())
        .unwrap()
        .issues
        .is_empty());
    assert_eq!(remote.generated, 2);
    assert_eq!(remote.creates, 2);
}
#[test]
fn symlinks_backups_and_historical_downloads_are_never_uploaded() {
    let f = Fixture::new();
    fs::write(f.base.join("private-data"), b"private").unwrap();
    std::os::unix::fs::symlink(f.base.join("private-data"), f.root.join("link")).unwrap();
    for name in [
        ".drivesync-part.part",
        "DriveSync backup test.bak",
        "old.txt",
    ] {
        fs::write(f.root.join(name), b"abc").unwrap();
    }
    let mut remote = Fake::default();
    let report = f
        .run(&mut remote, HashSet::from(["old.txt".into()]))
        .unwrap();
    assert_eq!(report.uploaded, 0);
    assert_eq!(remote.creates, 0);
    assert_eq!(report.issues.len(), 1);
    assert_eq!(report.issues[0].error, EngineError::DownloadConflict);
}
#[test]
fn generated_ids_and_session_origins_are_validated() {
    assert!(valid_session(
        "https://www.googleapis.com/upload/drive/v3/files?upload_id=test"
    ));
    for url in [
        "https://evil.invalid/upload/drive/v3/files?upload_id=test",
        "http://www.googleapis.com/upload/drive/v3/files?upload_id=test",
        "https://www.googleapis.com/drive/v3/files?upload_id=test",
        "https://user@www.googleapis.com/upload/drive/v3/files?upload_id=test",
    ] {
        assert!(!valid_session(url));
    }
    let f = Fixture::new();
    let op = Intent {
        path: "../escape".into(),
        root: crate::download::Identity {
            device: 0,
            inode: 0,
        },
        source: None,
        id: "safe-id".into(),
        parent: "root".into(),
        directory: true,
        size: 0,
        md5: String::new(),
        sha: String::new(),
        snapshot: String::new(),
        session: None,
        confirmed: false,
    };
    assert!(f.uploads().save(&op).is_err());
    assert!(!Path::new("../escape").starts_with(&f.root));
}

#[test]
fn replacing_the_root_blocks_upload_before_any_provider_effect() {
    let f = Fixture::new();
    fs::rename(&f.root, f.base.join("old-root")).unwrap();
    fs::create_dir(&f.root).unwrap();
    fs::write(f.root.join("file.txt"), b"unapproved replacement").unwrap();
    let mut remote = Fake::default();
    assert!(matches!(
        f.run(&mut remote, HashSet::new()),
        Err(EngineError::DownloadConflict)
    ));
    assert_eq!(remote.generated, 0);
    assert_eq!(remote.creates, 0);
}
#[test]
fn pending_upload_does_not_rebind_to_a_new_identity_or_destination() {
    let f = Fixture::new();
    fs::write(f.root.join("file.txt"), b"abc").unwrap();
    let mut remote = Fake {
        fail_after_complete: true,
        ..Default::default()
    };
    assert_eq!(f.run(&mut remote, HashSet::new()).unwrap().issues.len(), 1);
    let store = f.uploads();
    let mut op = store.load(Path::new("file.txt")).unwrap().unwrap();
    op.id = "replacement-id".into();
    assert!(store.save(&op).is_err());
    fs::write(f.root.join("file.txt"), b"local edits").unwrap();
    let report = f.run(&mut remote, HashSet::new()).unwrap();
    assert_eq!(report.issues.len(), 1);
    assert_eq!(report.issues[0].error, EngineError::DownloadConflict);
    assert_eq!(remote.generated, 1);
    assert_eq!(remote.creates, 1);
    assert_eq!(fs::read(f.root.join("file.txt")).unwrap(), b"local edits");
}

#[test]
fn cancellation_retains_the_session_and_confirmed_remote_removal_is_not_recreated() {
    let f = Fixture::new();
    fs::write(f.root.join("large.dat"), vec![1; 2 * 1024 * 1024]).unwrap();
    let mut remote = Fake {
        cancel_after_chunk: true,
        ..Default::default()
    };
    assert!(matches!(
        f.run(&mut remote, HashSet::new()),
        Err(EngineError::Authentication(crate::AuthError::Cancelled))
    ));
    assert_eq!(remote.chunks, 1);
    remote.cancel_after_chunk = false;
    assert!(f
        .run(&mut remote, HashSet::new())
        .unwrap()
        .issues
        .is_empty());
    assert_eq!(remote.generated, 1);
    assert_eq!(remote.creates, 1);
    remote.items.clear();
    assert!(f
        .run(&mut remote, HashSet::new())
        .unwrap()
        .issues
        .is_empty());
    assert_eq!(remote.creates, 1);
    assert_eq!(remote.generated, 1);
}

#[test]
fn a_replaced_source_with_identical_bytes_is_not_silently_adopted() {
    let f = Fixture::new();
    fs::write(f.root.join("file.txt"), b"abc").unwrap();
    let mut remote = Fake {
        fail_after_complete: true,
        ..Default::default()
    };
    assert_eq!(f.run(&mut remote, HashSet::new()).unwrap().issues.len(), 1);
    fs::rename(f.root.join("file.txt"), f.base.join("original-file")).unwrap();
    fs::write(f.root.join("file.txt"), b"abc").unwrap();
    let report = f.run(&mut remote, HashSet::new()).unwrap();
    assert_eq!(report.issues.len(), 1);
    assert_eq!(report.issues[0].error, EngineError::DownloadConflict);
    assert_eq!(report.uploaded, 0);
    assert_eq!(remote.creates, 1);
}

#[test]
fn corrupt_staging_blocks_replay_without_sending_another_chunk() {
    let f = Fixture::new();
    fs::write(f.root.join("large.dat"), vec![1; 2 * 1024 * 1024]).unwrap();
    let mut remote = Fake {
        fail_chunk: true,
        ..Default::default()
    };
    assert_eq!(f.run(&mut remote, HashSet::new()).unwrap().issues.len(), 1);
    let op = f.uploads().load(Path::new("large.dat")).unwrap().unwrap();
    fs::write(f.path.with_file_name(&op.snapshot), b"tampered").unwrap();
    let chunks = remote.chunks;
    assert!(matches!(
        f.run(&mut remote, HashSet::new()),
        Err(EngineError::StateStorage)
    ));
    assert_eq!(remote.chunks, chunks);
    assert_eq!(remote.creates, 0);
}

#[test]
fn verified_recovery_retires_only_its_own_stale_download_conflict() {
    let f = Fixture::new();
    fs::write(f.root.join("file.txt"), b"abc").unwrap();
    let mut remote = Fake {
        fail_after_complete: true,
        ..Default::default()
    };
    assert_eq!(f.run(&mut remote, HashSet::new()).unwrap().issues.len(), 1);
    let inv = RemoteInventory {
        account_id: f.folder.account_id.clone(),
        root_id: "root".into(),
        entries: remote.items.values().cloned().collect(),
    };
    let uploads = f.uploads();
    let mut report = DownloadReport {
        issues: vec![
            crate::DownloadIssue {
                relative_path: Some("file.txt".into()),
                error: EngineError::DownloadConflict,
            },
            crate::DownloadIssue {
                relative_path: Some("other.txt".into()),
                error: EngineError::DownloadConflict,
            },
        ],
        ..Default::default()
    };
    execution::run(
        execution::Context {
            client: "client",
            root_path: &f.root,
            state_path: &f.path,
            inventory: &inv,
            uploads: &uploads,
            cancel: &Cancellation::new(),
            excluded: HashSet::new(),
        },
        &mut remote,
        &mut report,
        |_| {},
    )
    .unwrap();
    assert_eq!(report.uploaded, 1);
    assert_eq!(report.issues.len(), 1);
    assert_eq!(
        report.issues[0].relative_path.as_deref(),
        Some(Path::new("other.txt"))
    );
}
#[test]
fn ambiguous_remote_names_cannot_be_adopted_after_a_lost_confirmation() {
    let f = Fixture::new();
    fs::write(f.root.join("file.txt"), b"abc").unwrap();
    let mut remote = Fake {
        fail_after_complete: true,
        ..Default::default()
    };
    f.run(&mut remote, HashSet::new()).unwrap();
    let mut other = remote.items["generated_1"].clone();
    other.id = "other-id".into();
    remote.items.insert(other.id.clone(), other);
    let report = f.run(&mut remote, HashSet::new()).unwrap();
    assert_eq!(report.uploaded, 0);
    assert_eq!(report.issues.len(), 1);
    assert_eq!(remote.creates, 1);
}

#[test]
fn temporary_uploads_are_deferred_and_final_rename_uploads_once() {
    let f = Fixture::new();
    fs::write(f.root.join("report.part"), b"abc").unwrap();
    let mut remote = Fake::default();
    let report = f.run(&mut remote, HashSet::new()).unwrap();
    assert_eq!(report.issues[0].error, EngineError::LocalTemporary);
    assert_eq!(remote.generated, 0);
    assert_eq!(remote.begins, 0);
    fs::rename(f.root.join("report.part"), f.root.join("report.txt")).unwrap();
    assert_eq!(f.run(&mut remote, HashSet::new()).unwrap().uploaded, 1);
    assert_eq!(f.run(&mut remote, HashSet::new()).unwrap().uploaded, 0);
    assert_eq!(remote.creates, 1);
}

#[test]
fn local_write_after_inventory_never_starts_remote_upload() {
    let f = Fixture::new();
    let path = f.root.join("file");
    fs::write(&path, b"abc").unwrap();
    let mut remote = Fake {
        change_on_generate: Some(path.clone()),
        ..Default::default()
    };
    let report = f.run(&mut remote, HashSet::new()).unwrap();
    assert_eq!(report.issues[0].error, EngineError::LocalWriting);
    assert_eq!(remote.begins, 0);
    assert_eq!(remote.creates, 0);
    assert!(f.uploads().load(Path::new("file")).unwrap().is_none());
    assert_eq!(fs::read(path).unwrap(), b"changed after inventory");
}

#[test]
fn temporary_item_does_not_stop_independent_ready_uploads() {
    let f = Fixture::new();
    fs::write(f.root.join("a.part"), b"partial").unwrap();
    fs::write(f.root.join("z.txt"), b"abc").unwrap();
    let mut remote = Fake::default();
    let report = f.run(&mut remote, HashSet::new()).unwrap();
    assert_eq!(report.uploaded, 1);
    assert_eq!(report.issues.len(), 1);
    assert_eq!(report.issues[0].error, EngineError::LocalTemporary);
    assert_eq!(remote.generated, 1);
    assert_eq!(remote.creates, 1);
    assert_eq!(fs::read(f.root.join("a.part")).unwrap(), b"partial");
}

#[test]
fn n10_resumed_writer_stops_before_final_upload_publication_and_keeps_intent() {
    let f = Fixture::new();
    let path = f.root.join("large.bin");
    fs::write(&path, vec![b'a'; 2 * 1024 * 1024]).unwrap();
    let mut remote = Fake {
        change_after_chunk: Some(path.clone()),
        ..Default::default()
    };
    let report = f.run(&mut remote, HashSet::new()).unwrap();
    assert_eq!(report.issues[0].error, EngineError::LocalWriting);
    assert_eq!(remote.chunks, 1);
    assert_eq!(remote.creates, 0);
    assert!(remote.items.is_empty());
    let op = f.uploads().load(Path::new("large.bin")).unwrap().unwrap();
    assert!(!op.confirmed);
    assert!(f.path.with_file_name(&op.snapshot).is_file());
    assert_eq!(fs::read(&path).unwrap(), b"writer resumed during upload");
    assert_eq!(f.run(&mut remote, HashSet::new()).unwrap().uploaded, 0);
    assert_eq!(remote.generated, 1);
    assert_eq!(remote.creates, 0);
}

#[test]
fn source_growth_is_rejected_before_reading_beyond_the_snapshot_size() {
    let f = Fixture::new();
    let path = f.root.join("file");
    fs::write(&path, b"abc").unwrap();
    f.run(&mut Fake::default(), HashSet::new()).unwrap();
    let op = f.uploads().load(Path::new("file")).unwrap().unwrap();
    fs::write(&path, vec![b'x'; 2 * 1024 * 1024]).unwrap();
    let mut file = fs::File::open(&path).unwrap();
    assert_eq!(
        super::local::verify_file(&mut file, &op, &Cancellation::new()),
        Err(EngineError::StateStorage)
    );
    assert_eq!(std::io::Seek::stream_position(&mut file).unwrap(), 0);
}

#[test]
fn n04_c25_upload_revocation_keeps_intent_and_resumes_same_remote_id_once() {
    let f = Fixture::new();
    let bytes = vec![42; 2 * 1024 * 1024];
    fs::write(f.root.join("large.bin"), &bytes).unwrap();
    let mut remote = Fake {
        suspend_after_chunk: Some(crate::RootSuspension::RemoteAccessDenied),
        ..Default::default()
    };
    for _ in 0..2 {
        assert!(matches!(
            f.run(&mut remote, HashSet::new()),
            Err(EngineError::RootSuspended(
                crate::RootSuspension::RemoteAccessDenied
            ))
        ));
        assert_eq!(remote.chunks, 1);
        assert_eq!(remote.creates, 0);
        assert_eq!(fs::read(f.root.join("large.bin")).unwrap(), bytes);
    }
    let op = f.uploads().load(Path::new("large.bin")).unwrap().unwrap();
    assert!(!op.confirmed);
    let id = op.id;
    remote.suspend_after_chunk = None;
    assert_eq!(f.run(&mut remote, HashSet::new()).unwrap().uploaded, 1);
    assert_eq!(remote.generated, 1);
    assert_eq!(remote.creates, 1);
    assert_eq!(remote.data[&id], bytes);
    assert_eq!(f.run(&mut remote, HashSet::new()).unwrap().uploaded, 0);
    assert_eq!(remote.creates, 1);
}

#[test]
fn n03_upload_root_replacement_stops_before_final_remote_publication() {
    let f = Fixture::new();
    let bytes = vec![17; 2 * 1024 * 1024];
    fs::write(f.root.join("large.bin"), &bytes).unwrap();
    let mut remote = Fake {
        replace_root_after_chunk: Some(f.root.clone()),
        ..Default::default()
    };
    assert!(matches!(
        f.run(&mut remote, HashSet::new()),
        Err(EngineError::RootSuspended(
            crate::RootSuspension::LocalUnavailable
        ))
    ));
    assert_eq!(remote.chunks, 1);
    assert_eq!(remote.creates, 0);
    assert_eq!(fs::read_dir(&f.root).unwrap().count(), 0);
    assert_eq!(
        fs::read(f.root.with_file_name("detached-volume").join("large.bin")).unwrap(),
        bytes
    );
    fs::remove_dir(&f.root).unwrap();
    fs::rename(f.root.with_file_name("detached-volume"), &f.root).unwrap();
    assert_eq!(f.run(&mut remote, HashSet::new()).unwrap().uploaded, 1);
    assert_eq!(remote.generated, 1);
}

#[test]
fn relocated_managed_identity_is_not_uploaded_as_a_new_file() {
    let f = Fixture::new();
    fs::write(f.root.join("file.txt"), b"abc").unwrap();
    let mut remote = Fake::default();
    assert_eq!(f.run(&mut remote, HashSet::new()).unwrap().uploaded, 1);
    let creates = remote.creates;
    fs::rename(f.root.join("file.txt"), f.root.join("renamed.txt")).unwrap();
    for _ in 0..2 {
        let report = f.run(&mut remote, HashSet::new()).unwrap();
        assert_eq!(report.uploaded, 0);
        assert_eq!(remote.creates, creates);
        assert_eq!(fs::read(f.root.join("renamed.txt")).unwrap(), b"abc");
        assert!(f
            .uploads()
            .load(Path::new("renamed.txt"))
            .unwrap()
            .is_none());
    }
}
