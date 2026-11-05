use super::*;
use crate::{LocalActivity, LocalCoverageGap, LocalItemKind, LocalNotice, LocalWatchState};
use std::os::unix::fs::symlink;
use std::time::Instant;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "drivesync-state-test-{}",
            oauth2::CsrfToken::new_random().secret()
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn store(&self) -> LocalStateStore {
        LocalStateStore::open(&self.0.join("private")).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn scope(account: &str, root: &str) -> LocalScope {
    LocalScope::new("client", account, Path::new(root)).unwrap()
}
fn snapshot(parent: Option<&ItemId>, name: &[u8], kind: LocalItemKind) -> ItemSnapshot {
    ItemSnapshot {
        parent: parent.cloned(),
        name: name.into(),
        kind,
        content_version: None,
        remote_version: None,
    }
}
fn baseline(store: &LocalStateStore, scope: &LocalScope) -> (ItemId, ItemId, i64) {
    let (root, g) = store
        .register_verified_item(
            scope,
            None,
            Some("root-remote".into()),
            snapshot(None, b"", LocalItemKind::Directory),
            true,
        )
        .unwrap();
    let (file, g) = store
        .register_verified_item(
            scope,
            Some(g),
            Some("file-remote".into()),
            snapshot(Some(&root), b"file", LocalItemKind::File),
            true,
        )
        .unwrap();
    (root, file, g)
}
fn batch(scope: &LocalScope, path: &[u8], activity: LocalActivity) -> LocalBatch {
    use std::os::unix::ffi::OsStringExt;
    LocalBatch {
        scope: scope.clone(),
        state: LocalWatchState::Observing,
        notices: vec![LocalNotice {
            path: std::ffi::OsString::from_vec(path.into()).into(),
            object_type: LocalItemKind::File,
            activity,
        }],
    }
}

#[test]
fn ids_baselines_versions_and_non_utf8_names_survive_restart() {
    let f = Fixture::new();
    let store = f.store();
    let s = scope("a", "/root");
    let (root, file, g) = baseline(&store, &s);
    let mut next = snapshot(Some(&root), b"renamed-\xff", LocalItemKind::File);
    next.content_version = Some("sha256:verified-version".into());
    next.remote_version = Some("remote-2".into());
    store
        .record_observed(
            &s,
            g,
            vec![(file.clone(), ItemState::Present(next.clone()))],
        )
        .unwrap();
    drop(store);
    let store = f.store();
    let saved = store.load(&s).unwrap().unwrap();
    let item = saved.items.iter().find(|i| i.id == file).unwrap();
    assert_eq!(item.remote_id.as_deref(), Some("file-remote"));
    assert_eq!(item.observed, ItemState::Present(next));
    assert_ne!(Some(item.observed.clone()), item.confirmed);
    assert_eq!(
        store
            .item_paths(&s, false)
            .unwrap()
            .get(&b"renamed-\xff"[..]),
        Some(&file)
    );
    assert_eq!(
        store.item_paths(&s, true).unwrap().get(&b"file"[..]),
        Some(&file)
    );
    store
        .confirm_observed(&s, saved.generation, std::slice::from_ref(&file))
        .unwrap();
    let saved = store.load(&s).unwrap().unwrap();
    let item = saved.items.iter().find(|i| i.id == file).unwrap();
    assert_eq!(item.confirmed, Some(item.observed.clone()));
}

#[test]
fn moving_a_parent_preserves_descendant_ids_and_confirmed_paths() {
    let f = Fixture::new();
    let store = f.store();
    let s = scope("a", "/root");
    let (root, g) = store
        .register_verified_item(
            &s,
            None,
            Some("r".into()),
            snapshot(None, b"", LocalItemKind::Directory),
            true,
        )
        .unwrap();
    let (folder, g) = store
        .register_verified_item(
            &s,
            Some(g),
            Some("folder".into()),
            snapshot(Some(&root), b"a", LocalItemKind::Directory),
            true,
        )
        .unwrap();
    let (file, g) = store
        .register_verified_item(
            &s,
            Some(g),
            Some("file".into()),
            snapshot(Some(&folder), b"child", LocalItemKind::File),
            true,
        )
        .unwrap();
    store
        .record_observed(
            &s,
            g,
            vec![(
                folder.clone(),
                ItemState::Present(snapshot(Some(&root), b"b", LocalItemKind::Directory)),
            )],
        )
        .unwrap();
    assert_eq!(
        store.item_paths(&s, false).unwrap().get(&b"b/child"[..]),
        Some(&file)
    );
    assert_eq!(
        store.item_paths(&s, true).unwrap().get(&b"a/child"[..]),
        Some(&file)
    );
    assert_eq!(store.load(&s).unwrap().unwrap().items.len(), 3);
}

#[test]
fn local_new_item_can_bind_a_remote_id_without_changing_identity() {
    let f = Fixture::new();
    let store = f.store();
    let s = scope("a", "/root");
    let (root, _, g) = baseline(&store, &s);
    let (new, g) = store
        .register_verified_item(
            &s,
            Some(g),
            None,
            snapshot(Some(&root), b"new", LocalItemKind::File),
            false,
        )
        .unwrap();
    let g = store
        .bind_remote_id(&s, g, &new, "uploaded-id".into())
        .unwrap();
    let saved = store.load(&s).unwrap().unwrap();
    let item = saved.items.iter().find(|i| i.id == new).unwrap();
    assert_eq!(item.remote_id.as_deref(), Some("uploaded-id"));
    assert_eq!(item.confirmed, None);
    assert_eq!(
        store.bind_remote_id(&s, g, &new, "other-id".into()),
        Err(LocalStateError::Invalid)
    );
}

#[test]
fn cycles_duplicate_paths_and_remote_ids_are_rejected_atomically() {
    let f = Fixture::new();
    let store = f.store();
    let s = scope("a", "/root");
    let (root, file, g) = baseline(&store, &s);
    let before = store.load(&s).unwrap();
    for (remote, name) in [
        (Some("file-remote".into()), b"other".as_slice()),
        (None, b"file".as_slice()),
        (None, b"../bad".as_slice()),
    ] {
        assert!(store
            .register_verified_item(
                &s,
                Some(g),
                remote,
                snapshot(Some(&root), name, LocalItemKind::File),
                false
            )
            .is_err());
        assert_eq!(store.load(&s).unwrap(), before);
    }
    assert!(store
        .record_observed(
            &s,
            g,
            vec![(
                root.clone(),
                ItemState::Present(snapshot(Some(&file), b"loop", LocalItemKind::Directory))
            )]
        )
        .is_err());
    assert_eq!(store.load(&s).unwrap(), before);
}

#[test]
fn subtree_absence_is_atomic_and_keeps_the_confirmed_tree() {
    let f = Fixture::new();
    let store = f.store();
    let s = scope("a", "/root");
    let (root, file, g) = baseline(&store, &s);
    assert!(store
        .record_observed(&s, g, vec![(root.clone(), ItemState::Absent)])
        .is_err());
    store
        .record_observed(
            &s,
            g,
            vec![(root, ItemState::Absent), (file.clone(), ItemState::Absent)],
        )
        .unwrap();
    assert!(store.item_paths(&s, false).unwrap().is_empty());
    assert_eq!(
        store.item_paths(&s, true).unwrap().get(&b"file"[..]),
        Some(&file)
    );
}

#[test]
fn hints_coalesce_preserve_candidate_only_and_require_unknown_or_offline_review() {
    let f = Fixture::new();
    let store = f.store();
    let s = scope("a", "/root");
    let (_, file, _) = baseline(&store, &s);
    store
        .ingest(
            &batch(&s, b"file", LocalActivity::Written),
            LocalConnectivity::Unknown,
        )
        .unwrap();
    let first = store.load(&s).unwrap().unwrap();
    let work = first.pending[0].clone();
    store
        .ingest(
            &batch(&s, b"file", LocalActivity::ClosedAfterWrite),
            LocalConnectivity::Online,
        )
        .unwrap();
    let saved = f.store().load(&s).unwrap().unwrap();
    assert_eq!(saved.pending.len(), 1);
    assert_eq!(saved.pending[0].id, work.id);
    assert_eq!(saved.pending[0].revision, 2);
    assert!(saved.pending[0].requires_confirmation);
    assert_eq!(saved.pending[0].candidate_item, Some(file));
    assert_eq!(saved.pending[0].reasons.len(), 2);
    assert_eq!(first.items, saved.items);
    store
        .ingest(
            &batch(&s, b"offline", LocalActivity::Created),
            LocalConnectivity::Offline,
        )
        .unwrap();
    assert!(store
        .load(&s)
        .unwrap()
        .unwrap()
        .pending
        .iter()
        .all(|p| p.requires_confirmation));
}

#[test]
fn stale_completion_and_writers_never_erase_new_hints() {
    let f = Fixture::new();
    let store = f.store();
    let s = scope("a", "/root");
    store
        .ingest(
            &batch(&s, b"file", LocalActivity::Written),
            LocalConnectivity::Online,
        )
        .unwrap();
    let old = store.load(&s).unwrap().unwrap();
    let p = &old.pending[0];
    let next = store
        .ingest(
            &batch(&s, b"file", LocalActivity::Written),
            LocalConnectivity::Online,
        )
        .unwrap();
    assert_eq!(
        store.complete_investigation(&s, old.generation, p.id, p.revision),
        Err(LocalStateError::Stale)
    );
    assert_eq!(
        store.complete_investigation(&s, next, p.id, p.revision),
        Err(LocalStateError::Stale)
    );
    let saved = store.load(&s).unwrap().unwrap();
    let p = &saved.pending[0];
    store
        .complete_investigation(&s, next, p.id, p.revision)
        .unwrap();
    assert!(store.load(&s).unwrap().unwrap().pending.is_empty());
    assert!(store
        .register_verified_item(
            &s,
            None,
            None,
            snapshot(None, b"", LocalItemKind::Directory),
            false
        )
        .is_err());
}

#[test]
fn directory_moves_and_startup_request_reconciliation_without_remapping() {
    let f = Fixture::new();
    let store = f.store();
    let s = scope("a", "/root");
    let (_, _, _) = baseline(&store, &s);
    let before = store.load(&s).unwrap().unwrap().items;
    let mut event = batch(
        &s,
        b"dest",
        LocalActivity::Moved {
            from: "file".into(),
        },
    );
    event.notices[0].object_type = LocalItemKind::Directory;
    store.ingest(&event, LocalConnectivity::Online).unwrap();
    let saved = store.load(&s).unwrap().unwrap();
    assert_eq!(saved.items, before);
    assert!(saved
        .pending
        .iter()
        .any(|p| p.path.is_empty() && p.coverage_uncertain));
    assert!(saved.pending.iter().any(|p| p.path == b"file"));
    assert!(saved.pending.iter().any(|p| p.path == b"dest"));
    let startup = batch(&s, b"", LocalActivity::Recheck(LocalCoverageGap::Startup));
    store.ingest(&startup, LocalConnectivity::Unknown).unwrap();
    assert!(f
        .store()
        .load(&s)
        .unwrap()
        .unwrap()
        .pending
        .iter()
        .any(|p| p.path.is_empty() && p.requires_confirmation));
}

#[test]
fn scope_clients_accounts_and_roots_remain_independent() {
    let f = Fixture::new();
    let store = f.store();
    let scopes = [
        scope("a", "/one"),
        scope("b", "/one"),
        scope("a", "/two"),
        LocalScope::new("other", "a", Path::new("/one")).unwrap(),
    ];
    for s in &scopes {
        store
            .ingest(
                &batch(s, b"work", LocalActivity::Written),
                LocalConnectivity::Online,
            )
            .unwrap();
    }
    let first = store.load(&scopes[0]).unwrap().unwrap();
    store
        .complete_investigation(
            &scopes[0],
            first.generation,
            first.pending[0].id,
            first.pending[0].revision,
        )
        .unwrap();
    for s in &scopes[1..] {
        assert_eq!(store.load(s).unwrap().unwrap().pending.len(), 1);
    }
}

#[test]
fn pending_capacity_becomes_root_reconciliation_instead_of_silent_loss() {
    let f = Fixture::new();
    let store = f.store();
    let s = scope("a", "/root");
    let mut events = batch(&s, b"", LocalActivity::Written);
    events.notices.clear();
    for n in 0..=MAX_PENDING {
        events.notices.push(LocalNotice {
            path: format!("f{n}").into(),
            object_type: LocalItemKind::File,
            activity: LocalActivity::Written,
        });
    }
    store.ingest(&events, LocalConnectivity::Offline).unwrap();
    let state = store.load(&s).unwrap().unwrap();
    assert_eq!(state.pending.len(), 1);
    assert!(state.pending[0].path.is_empty());
    assert!(state.pending[0].requires_confirmation);
    assert!(state.pending[0]
        .reasons
        .contains(&InvestigationReason::CapacityLimit));
}

#[test]
fn incompatible_corrupt_and_unsafe_storage_are_preserved() {
    let f = Fixture::new();
    let store = f.store();
    let s = scope("a", "/root");
    store
        .ingest(
            &batch(&s, b"x", LocalActivity::Written),
            LocalConnectivity::Online,
        )
        .unwrap();
    let c = store.connect().unwrap();
    c.execute(
        "UPDATE state_scopes SET payload=?1",
        [b"invalid".as_slice()],
    )
    .unwrap();
    assert_eq!(store.load(&s), Err(LocalStateError::Incompatible));
    assert!(store
        .ingest(
            &batch(&s, b"x", LocalActivity::Written),
            LocalConnectivity::Online
        )
        .is_err());
    assert_eq!(
        c.query_row("SELECT payload FROM state_scopes", [], |r| r
            .get::<_, Vec<u8>>(0))
            .unwrap(),
        b"invalid"
    );
    for unsupported in [1, 3] {
        c.pragma_update(None, "user_version", unsupported).unwrap();
        assert!(matches!(
            f.store_result(),
            Err(LocalStateError::Incompatible)
        ));
    }
    let alias = f.0.join("alias");
    symlink(f.0.join("private"), &alias).unwrap();
    assert!(LocalStateStore::open(&alias).is_err());
    fs::set_permissions(f.0.join("private"), fs::Permissions::from_mode(0o755)).unwrap();
    assert!(LocalStateStore::open(&f.0.join("private")).is_err());
}
impl Fixture {
    fn store_result(&self) -> Result<LocalStateStore, LocalStateError> {
        LocalStateStore::open(&self.0.join("private"))
    }
}

#[test]
fn sqlite_failure_rolls_back_all_state_and_work() {
    let f = Fixture::new();
    let store = f.store();
    let s = scope("a", "/root");
    let _ = baseline(&store, &s);
    let old = store.load(&s).unwrap();
    store.connect().unwrap().execute_batch("CREATE TRIGGER fail_update BEFORE UPDATE ON state_scopes BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert_eq!(
        store.ingest(
            &batch(&s, b"file", LocalActivity::Written),
            LocalConnectivity::Online
        ),
        Err(LocalStateError::Storage)
    );
    assert_eq!(store.load(&s).unwrap(), old);
}

#[test]
fn concurrent_producers_merge_and_initialization_does_not_replace_data() {
    let f = Fixture::new();
    let path = f.0.join("private");
    let s = scope("a", "/root");
    std::thread::scope(|threads| {
        for n in 0..4 {
            let path = &path;
            let s = &s;
            threads.spawn(move || {
                let store = LocalStateStore::open(path).unwrap();
                store
                    .ingest(
                        &batch(s, format!("f{n}").as_bytes(), LocalActivity::Written),
                        LocalConnectivity::Online,
                    )
                    .unwrap();
            });
        }
    });
    assert_eq!(f.store().load(&s).unwrap().unwrap().pending.len(), 4);
}

#[test]
fn killed_uncommitted_writer_keeps_previous_generation_and_pending_work() {
    const CHILD: &str = "DRIVESYNC_STATE_KILL_TEST";
    if let Some(path) = std::env::var_os(CHILD) {
        let store = LocalStateStore::open(Path::new(&path)).unwrap();
        let result = store.ingest(
            &batch(&scope("a", "/root"), b"new", LocalActivity::Written),
            LocalConnectivity::Online,
        );
        panic!("writer unexpectedly completed: {result:?}");
    }
    let f = Fixture::new();
    let store = f.store();
    let s = scope("a", "/root");
    store
        .ingest(
            &batch(&s, b"old", LocalActivity::Written),
            LocalConnectivity::Online,
        )
        .unwrap();
    let old = store.load(&s).unwrap();
    let reader = store.connect().unwrap();
    reader
        .execute_batch("BEGIN; SELECT count(*) FROM state_scopes;")
        .unwrap();
    let mut child=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","local_state::store::tests::killed_uncommitted_writer_keeps_previous_generation_and_pending_work","--nocapture"]).env(CHILD,f.0.join("private")).spawn().unwrap();
    let journal = f.0.join("private/local-state.sqlite3-journal");
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut started = false;
    while Instant::now() < deadline {
        if fs::metadata(&journal).is_ok_and(|m| m.len() > 0) {
            started = true;
            break;
        }
        if child.try_wait().unwrap().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let killed = child.kill();
    let status = child.wait().unwrap();
    reader.execute_batch("ROLLBACK;").unwrap();
    assert!(started);
    killed.unwrap();
    assert!(!status.success());
    assert_eq!(f.store().load(&s).unwrap(), old);
}

#[test]
fn invalid_batch_and_oversized_database_payload_do_not_advance_state() {
    let f = Fixture::new();
    let store = f.store();
    let s = scope("a", "/root");
    store
        .ingest(
            &batch(&s, b"old", LocalActivity::Written),
            LocalConnectivity::Online,
        )
        .unwrap();
    let old = store.load(&s).unwrap();
    let mut invalid = batch(&s, b"new", LocalActivity::Written);
    invalid
        .notices
        .extend(batch(&s, b"../invalid", LocalActivity::Written).notices);
    assert_eq!(
        store.ingest(&invalid, LocalConnectivity::Online),
        Err(LocalStateError::Invalid)
    );
    assert_eq!(store.load(&s).unwrap(), old);
    store
        .connect()
        .unwrap()
        .execute(
            "UPDATE state_scopes SET payload=zeroblob(?1)",
            [MAX_PAYLOAD as i64 + 1],
        )
        .unwrap();
    assert_eq!(store.load(&s), Err(LocalStateError::Incompatible));
}

#[test]
fn excessive_tree_depth_and_item_count_preserve_previous_checkpoint() {
    let f = Fixture::new();
    let store = f.store();
    let s = scope("a", "/root");
    let (_, _, g) = baseline(&store, &s);
    let old = store.load(&s).unwrap();
    let result = store.mutate(&s, Some(Some(g)), |document| {
        let mut parent = document.root.clone().unwrap();
        for _ in 0..129 {
            let id = ItemId::new();
            let state = ItemState::Present(snapshot(Some(&parent), b"d", LocalItemKind::Directory));
            document.items.push(TrackedItem {
                id: id.clone(),
                remote_id: None,
                confirmed: None,
                observed: state,
            });
            parent = id;
        }
        Ok(())
    });
    assert_eq!(result, Err(LocalStateError::Invalid));
    assert_eq!(store.load(&s).unwrap(), old);
    let result = store.mutate(&s, Some(Some(g)), |document| {
        document
            .items
            .resize(MAX_ITEMS + 1, document.items[0].clone());
        Ok(())
    });
    assert_eq!(result, Err(LocalStateError::Limit));
    assert_eq!(store.load(&s).unwrap(), old);
}

#[test]
fn database_symlinks_and_self_observation_are_rejected() {
    let f = Fixture::new();
    let store = f.store();
    let own = LocalScope::new("client", "a", &f.0).unwrap();
    assert_eq!(
        store.ingest(
            &batch(&own, b"x", LocalActivity::Written),
            LocalConnectivity::Online
        ),
        Err(LocalStateError::Invalid)
    );
    let alias = f.0.join("other");
    fs::create_dir(&alias).unwrap();
    fs::set_permissions(&alias, fs::Permissions::from_mode(0o700)).unwrap();
    symlink(&store.path, alias.join("local-state.sqlite3")).unwrap();
    assert!(LocalStateStore::open(&alias).is_err());
}

#[test]
fn storage_paths_are_canonical_for_self_observation_checks() {
    let f = Fixture::new();
    let private = f.0.join("private");
    fs::create_dir(&private).unwrap();
    fs::set_permissions(&private, fs::Permissions::from_mode(0o700)).unwrap();
    let aliased = private.join("../private");
    let store = LocalStateStore::open(&aliased).unwrap();
    assert_eq!(store.path, private.join("local-state.sqlite3"));
    let own = LocalScope::new("client", "a", &f.0).unwrap();
    assert_eq!(
        store.ingest(
            &batch(&own, b"x", LocalActivity::Written),
            LocalConnectivity::Online
        ),
        Err(LocalStateError::Invalid)
    );
}
