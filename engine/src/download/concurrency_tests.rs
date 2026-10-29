//! I22-I24: deterministic observations between planning, reads and installation.
use super::baseline_tests::{base, cycle, prepare};
use super::*;
use crate::{ConflictReason, DownloadPlan};
use std::{cell::RefCell, rc::Rc};

pub(super) const V1: &[u8] = b"BASE-V1";
pub(super) const V2: &[u8] = b"BASE-V2";
pub(super) fn remote(bytes: &[u8], version: u64) -> RemoteInventory {
    let mut inv = inventory();
    let item = &mut inv.entries[0];
    item.name = "dados.db".into();
    item.version = Some(version);
    item.size = Some(bytes.len() as u64);
    item.md5_checksum = Some(format!("{:x}", md5::Md5::digest(bytes)));
    inv
}
pub(super) fn fake(inv: &RemoteInventory, bytes: &[u8]) -> Fake {
    let mut source = Fake::new(inv);
    source
        .content
        .insert(inv.entries[0].id.clone(), bytes.to_vec());
    source
}
pub(super) fn installed() -> (Fixture, RemoteInventory) {
    let f = Fixture::new();
    let inv = remote(V1, 1);
    assert_eq!(cycle(&f, &inv, &mut fake(&inv, V1)).unwrap().downloaded, 1);
    (f, inv)
}
/// Executes the supplied plan without rebuilding it, so a concurrent change
/// cannot be hidden by test preparation. Provider and progress hooks are local.
pub(super) fn execute_plan(
    f: &Fixture,
    inv: &RemoteInventory,
    plan: &DownloadPlan,
    source: &mut impl Source,
    cancel: &Cancellation,
    progress: impl FnMut(DownloadProgress),
) -> Result<DownloadReport, EngineError> {
    let store = Store::open(&f.db())?;
    run(
        Context {
            client: "client",
            scope: plan,
            store: &store,
            cancel,
        },
        inv,
        source,
        progress,
    )
}
pub(super) fn journal(f: &Fixture, inv: &RemoteInventory) -> Vec<Operation> {
    let plan = crate::planner::plan(inv, &f.root, |p| crate::planner::observe(&f.root, p)).unwrap();
    Store::open(&f.db())
        .unwrap()
        .operations("client", &plan)
        .unwrap()
}
pub(super) fn files(f: &Fixture) -> HashMap<PathBuf, (Vec<u8>, u64, u64)> {
    fs::read_dir(&f.root)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            let metadata = entry.metadata().unwrap();
            assert!(metadata.is_file());
            (
                PathBuf::from(entry.file_name()),
                (
                    fs::read(entry.path()).unwrap(),
                    metadata.dev(),
                    metadata.ino(),
                ),
            )
        })
        .collect()
}
fn replace(f: &Fixture, bytes: &[u8]) {
    let path = f.root.join("dados.db");
    let old = fs::metadata(&path).unwrap();
    fs::rename(&path, f.root.join("user-original")).unwrap();
    fs::write(&path, bytes).unwrap();
    let new = fs::metadata(&path).unwrap();
    assert_ne!((old.dev(), old.ino()), (new.dev(), new.ino()));
}

struct RemoteState {
    available: bool,
    entries: HashMap<String, InventoryEntry>,
}
/// A one-shot intervention runs after a positive media read, before its bytes
/// return to the production stream copier. Shared metadata exposes the new state.
struct DuringRead {
    reader: Box<dyn Read>,
    hook: Option<Box<dyn FnOnce()>>,
}
impl Read for DuringRead {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let n = self.reader.read(buffer)?;
        if n > 0 {
            if let Some(hook) = self.hook.take() {
                hook();
            }
        }
        Ok(n)
    }
}
struct Controlled {
    fake: Fake,
    state: Rc<RefCell<RemoteState>>,
    on_read: Option<Box<dyn FnOnce()>>,
}
impl Controlled {
    fn new(inv: &RemoteInventory, bytes: &[u8]) -> Self {
        let fake = fake(inv, bytes);
        let state = Rc::new(RefCell::new(RemoteState {
            available: true,
            entries: fake.entries.clone(),
        }));
        Self {
            fake,
            state,
            on_read: None,
        }
    }
    fn available(&self) -> Result<(), EngineError> {
        if self.state.borrow().available {
            Ok(())
        } else {
            Err(EngineError::Authentication(crate::AuthError::Network))
        }
    }
}
impl Source for Controlled {
    fn metadata(&mut self, item: &InventoryEntry) -> Result<InventoryEntry, EngineError> {
        self.available()?;
        self.fake.entries = self.state.borrow().entries.clone();
        self.fake.metadata(item)
    }
    fn media(&mut self, id: &str) -> Result<Box<dyn Read>, EngineError> {
        self.available()?;
        Ok(Box::new(DuringRead {
            reader: self.fake.media(id)?,
            hook: self.on_read.take(),
        }))
    }
    fn configured(&mut self) -> Result<(), EngineError> {
        self.available()
    }
}

#[test]
fn i22_equal_and_different_byte_swaps_invalidate_plans_and_installation() {
    for window in 0..3 {
        for bytes in [V1, &b"USER-DATA"[..]] {
            let (f, old) = installed();
            let confirmed = base(&f, &old).unwrap();
            let next = remote(V2, 2);
            let plan = prepare(&f, &next).unwrap();
            assert_eq!(plan.entries[0].action, PlanAction::Download);
            let mut source = fake(&next, V2);
            let mut intervened = false;
            if window == 0 {
                replace(&f, bytes);
                intervened = true;
            }
            let report = execute_plan(&f, &next, &plan, &mut source, &Cancellation::new(), |p| {
                if !intervened
                    && p.current.is_some_and(|p| {
                        (window == 1 && p.stage == DownloadStage::Downloading && p.received > 0)
                            || (window == 2 && p.stage == DownloadStage::Installing)
                    })
                {
                    replace(&f, bytes);
                    intervened = true;
                }
            })
            .unwrap();
            assert!(intervened);
            assert_eq!(report.issues[0].error, EngineError::DownloadConflict);
            assert_eq!((report.updated, report.downloaded), (0, 0));
            assert!(report.preserved_copies.is_empty());
            assert_eq!(fs::read(f.root.join("dados.db")).unwrap(), bytes);
            assert_eq!(fs::read(f.root.join("user-original")).unwrap(), V1);
            assert_eq!(source.media_ids.len(), usize::from(window != 0));
            if window == 0 {
                assert_eq!(base(&f, &next), Some(confirmed));
            } else {
                assert!(base(&f, &next).is_none());
                let op = journal(&f, &next).pop().unwrap();
                let previous = match op.phase {
                    Phase::Updating { previous } | Phase::UpdateReady { previous, .. } => previous,
                    _ => panic!("expected pending update"),
                };
                let original = fs::metadata(f.root.join("user-original")).unwrap();
                assert_eq!(
                    (previous.identity.device, previous.identity.inode),
                    (original.dev(), original.ino())
                );
            }
            let retained = files(&f);
            let pending = journal(&f, &next);
            for _ in 0..2 {
                let expected = if window == 0 {
                    ConflictReason::ConcurrentIdentityChanges
                } else {
                    ConflictReason::ExistingLocalData
                };
                assert_eq!(
                    prepare(&f, &next).unwrap().entries[0].action,
                    PlanAction::Blocked(expected)
                );
                assert_eq!(
                    cycle(&f, &next, &mut source).unwrap().issues[0].error,
                    EngineError::DownloadConflict
                );
                assert_eq!(files(&f), retained);
                assert_eq!(
                    serde_json::to_value(journal(&f, &next)).unwrap(),
                    serde_json::to_value(&pending).unwrap()
                );
                assert_eq!(source.media_ids.len(), usize::from(window != 0));
            }
        }
    }
}

#[test]
fn i23_revision_removal_and_new_homonym_during_media_cannot_change_planned_identity() {
    for update in [false, true] {
        for mutation in 0..3 {
            let (f, old) = if update {
                installed()
            } else {
                (Fixture::new(), remote(V1, 1))
            };
            let next = remote(V2, 2);
            let plan = prepare(&f, &next).unwrap();
            let mut source = Controlled::new(&next, V2);
            let state = source.state.clone();
            source.on_read = Some(Box::new(move || {
                let mut state = state.borrow_mut();
                match mutation {
                    0 => {
                        state.entries.get_mut("file").unwrap().version = Some(3);
                    }
                    1 => {
                        state.entries.clear();
                    }
                    _ => {
                        let mut substitute = state.entries.remove("file").unwrap();
                        substitute.id = "other-id".into();
                        state.entries.insert(substitute.id.clone(), substitute);
                    }
                }
            }));
            let report =
                execute_plan(&f, &next, &plan, &mut source, &Cancellation::new(), |_| {}).unwrap();
            assert!(source.on_read.is_none());
            assert_eq!(report.issues[0].error, EngineError::RemoteChanged);
            assert_eq!((report.downloaded, report.updated), (0, 0));
            assert!(base(&f, &next).is_none());
            let operations = journal(&f, &next);
            assert_eq!(operations.len(), 1 + usize::from(update));
            let pending = operations.last().unwrap();
            assert_eq!(pending.item.id, "file");
            assert_eq!(pending.item.version, Some(2));
            assert!(matches!(
                pending.phase,
                Phase::Started | Phase::Updating { .. }
            ));
            if update {
                assert_eq!(fs::read(f.root.join("dados.db")).unwrap(), V1);
            } else {
                assert!(!f.root.join("dados.db").exists());
            }
            let retained = files(&f);
            for _ in 0..2 {
                let plan = prepare(&f, &next).unwrap();
                assert_eq!(
                    execute_plan(&f, &next, &plan, &mut source, &Cancellation::new(), |_| {})
                        .unwrap()
                        .issues[0]
                        .error,
                    EngineError::RemoteChanged
                );
                assert_eq!(files(&f), retained);
                assert_eq!(source.fake.media_ids, ["file"]);
                assert!(source.fake.metadata_ids.iter().all(|id| id == "file"));
                assert!(base(&f, &old).is_none());
            }
        }
    }
}

#[test]
fn i24_unavailability_then_local_or_remote_substitution_revalidates_the_old_base() {
    for local_swap in [false, true] {
        for remote_swap in [false, true] {
            let (f, old) = installed();
            let confirmed = base(&f, &old).unwrap();
            let before = files(&f);
            let mut next = remote(V2, 2);
            if remote_swap {
                next.entries[0].id = "other-id".into();
            }
            let mut source = Controlled::new(&next, V2);
            source.state.borrow_mut().available = false;
            let plan = prepare(&f, &old).unwrap();
            assert_eq!(
                execute_plan(&f, &old, &plan, &mut source, &Cancellation::new(), |_| {})
                    .unwrap_err(),
                EngineError::Authentication(crate::AuthError::Network)
            );
            assert_eq!(files(&f), before);
            assert_eq!(base(&f, &old), Some(confirmed.clone()));
            assert!(source.fake.media_ids.is_empty());
            if local_swap {
                replace(&f, V1);
            }
            source.state.borrow_mut().available = true;
            let retained = files(&f);
            for iteration in 0..2 {
                let plan = prepare(&f, &next).unwrap();
                let report =
                    execute_plan(&f, &next, &plan, &mut source, &Cancellation::new(), |_| {})
                        .unwrap();
                if local_swap || remote_swap {
                    let reason = if remote_swap {
                        ConflictReason::ExistingLocalData
                    } else {
                        ConflictReason::ConcurrentIdentityChanges
                    };
                    assert_eq!(plan.entries[0].action, PlanAction::Blocked(reason));
                    assert_eq!(report.issues[0].error, EngineError::DownloadConflict);
                    assert_eq!(files(&f), retained);
                    assert_eq!(base(&f, &old), Some(confirmed.clone()));
                    assert!(source.fake.media_ids.is_empty());
                    if remote_swap {
                        assert!(base(&f, &next).is_none());
                    }
                } else {
                    assert!(report.issues.is_empty());
                    assert_eq!(report.updated, usize::from(iteration == 0));
                    assert_eq!(fs::read(f.root.join("dados.db")).unwrap(), V2);
                    assert_eq!(base(&f, &next).unwrap().remote_version, 2);
                    assert_eq!(source.fake.media_ids, ["file"]);
                }
            }
        }
    }
}

#[test]
fn n03_root_replacement_during_media_stops_before_staging_write_or_installation() {
    let (f, _old) = installed();
    let next = remote(V2, 2);
    let plan = prepare(&f, &next).unwrap();
    let mut source = Controlled::new(&next, V2);
    let path = f.root.clone();
    let detached = f.base.join("detached-volume");
    let parked = detached.clone();
    source.on_read = Some(Box::new(move || {
        fs::rename(&path, &parked).unwrap();
        fs::create_dir(&path).unwrap();
    }));
    let result = execute_plan(&f, &next, &plan, &mut source, &Cancellation::new(), |_| {});
    assert!(matches!(
        result,
        Err(EngineError::RootSuspended(
            crate::RootSuspension::LocalUnavailable
        ))
    ));
    assert_eq!(fs::read_dir(&f.root).unwrap().count(), 0);
    assert_eq!(fs::read(detached.join("dados.db")).unwrap(), V1);
    // The detached staging descriptor received no bytes after root replacement.
    for entry in fs::read_dir(&detached).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() != "dados.db" {
            assert_eq!(entry.metadata().unwrap().len(), 0);
        }
    }
    fs::remove_dir(&f.root).unwrap();
    fs::rename(detached, &f.root).unwrap();
    let operations = journal(&f, &next);
    assert!(!matches!(
        operations.last().unwrap().phase,
        Phase::Installed { .. } | Phase::Updated { .. }
    ));
    // Rebuild the plan and reconcile the pending operation after restoration.
    let mut recovered = fake(&next, V2);
    let plan = prepare(&f, &next).unwrap();
    let report = execute_plan(
        &f,
        &next,
        &plan,
        &mut recovered,
        &Cancellation::new(),
        |_| {},
    )
    .unwrap();
    assert!(report.issues.is_empty());
    assert_eq!(fs::read(f.root.join("dados.db")).unwrap(), V2);
    let count = journal(&f, &next).len();
    let plan = prepare(&f, &next).unwrap();
    assert!(execute_plan(
        &f,
        &next,
        &plan,
        &mut recovered,
        &Cancellation::new(),
        |_| {}
    )
    .unwrap()
    .issues
    .is_empty());
    assert_eq!(journal(&f, &next).len(), count);
}

#[test]
fn n04_c25_revoked_root_during_media_stops_without_confirming_or_touching_other_items() {
    struct Gate(Controlled);
    impl Source for Gate {
        fn metadata(&mut self, item: &InventoryEntry) -> Result<InventoryEntry, EngineError> {
            self.0.metadata(item)
        }
        fn media(&mut self, id: &str) -> Result<Box<dyn Read>, EngineError> {
            self.0.media(id)
        }
        fn configured(&mut self) -> Result<(), EngineError> {
            if self.0.state.borrow().available {
                Ok(())
            } else {
                Err(EngineError::RootSuspended(
                    crate::RootSuspension::RemoteAccessDenied,
                ))
            }
        }
    }
    let (f, old) = installed();
    let next = remote(V2, 2);
    let plan = prepare(&f, &next).unwrap();
    let mut source = Gate(Controlled::new(&next, V2));
    let state = source.0.state.clone();
    source.0.on_read = Some(Box::new(move || state.borrow_mut().available = false));
    assert!(matches!(
        execute_plan(&f, &next, &plan, &mut source, &Cancellation::new(), |_| {}),
        Err(EngineError::RootSuspended(
            crate::RootSuspension::RemoteAccessDenied
        ))
    ));
    assert_eq!(fs::read(f.root.join("dados.db")).unwrap(), V1);
    assert!(!matches!(
        journal(&f, &next).last().unwrap().phase,
        Phase::Installed { .. } | Phase::Updated { .. }
    ));
    let count = journal(&f, &next).len();
    for _ in 0..2 {
        let plan = prepare(&f, &next).unwrap();
        assert!(execute_plan(&f, &next, &plan, &mut source, &Cancellation::new(), |_| {}).is_err());
        assert_eq!(journal(&f, &next).len(), count);
    }
    source.0.state.borrow_mut().available = true;
    let plan = prepare(&f, &next).unwrap();
    assert!(
        execute_plan(&f, &next, &plan, &mut source, &Cancellation::new(), |_| {})
            .unwrap()
            .issues
            .is_empty()
    );
    assert_eq!(fs::read(f.root.join("dados.db")).unwrap(), V2);
    assert!(base(&f, &old).is_some());
}

#[test]
fn e09_missing_remote_item_is_not_root_deletion_and_independent_download_continues() {
    let f = Fixture::new();
    let mut inv = remote(V1, 1);
    let mut second = inv.entries[0].clone();
    second.id = "independent".into();
    second.name = "independent.db".into();
    inv.entries.push(second);
    let plan = prepare(&f, &inv).unwrap();
    let mut source = fake(&inv, V1);
    source.content.insert("independent".into(), V1.to_vec());
    source.entries.remove("file");
    let report = execute_plan(&f, &inv, &plan, &mut source, &Cancellation::new(), |_| {}).unwrap();
    assert_eq!(report.downloaded, 1);
    assert_eq!(report.issues.len(), 1);
    assert_eq!(report.issues[0].error, EngineError::RemoteChanged);
    assert!(!f.root.join("dados.db").exists());
    assert_eq!(fs::read(f.root.join("independent.db")).unwrap(), V1);
}

#[test]
fn n03_write_permission_revoked_during_read_stops_an_already_open_staging_descriptor() {
    use std::os::unix::fs::PermissionsExt;
    let (f, _) = installed();
    let next = remote(V2, 2);
    let plan = prepare(&f, &next).unwrap();
    let mut source = Controlled::new(&next, V2);
    let path = f.root.clone();
    source.on_read = Some(Box::new(move || {
        fs::set_permissions(path, fs::Permissions::from_mode(0o500)).unwrap()
    }));
    let result = execute_plan(&f, &next, &plan, &mut source, &Cancellation::new(), |_| {});
    fs::set_permissions(&f.root, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(matches!(
        result,
        Err(EngineError::RootSuspended(
            crate::RootSuspension::LocalUnavailable
        ))
    ));
    assert_eq!(fs::read(f.root.join("dados.db")).unwrap(), V1);
    let operations = journal(&f, &next);
    let pending = operations.last().unwrap();
    assert_eq!(
        fs::metadata(f.root.join(&pending.temporary)).unwrap().len(),
        0
    );
}
