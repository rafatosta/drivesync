//! N03/N04/C25/E09 availability oracles, with real isolated filesystem and SQLite.
use super::*;
use crate::{state::Roots, AccountId, DriveFolder};
use std::{fs, path::PathBuf};
struct Fixture {
    base: PathBuf,
    folder: FolderConfiguration,
}
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "drivesync-availability-{}",
            oauth2::CsrfToken::new_random().secret()
        ));
        let local = base.join("volume");
        fs::create_dir_all(&local).unwrap();
        let folder = FolderConfiguration {
            account_id: AccountId("a".into()),
            remote: DriveFolder {
                id: "remote".into(),
                name: "Root".into(),
            },
            local: local.clone(),
            local_display: local,
        };
        identity::ensure(&folder, "client").unwrap();
        let fixture = Self { base, folder };
        fixture
            .registry()
            .bind("client", &fixture.folder, &fixture.marker())
            .unwrap();
        fixture
    }
    fn registry(&self) -> Roots {
        Roots::open(&self.base.join("app/roots.sqlite3")).unwrap()
    }
    fn marker(&self) -> String {
        identity::verify(&self.folder, "client").unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

#[test]
fn n03_empty_is_available_but_absent_or_replaced_volume_never_becomes_empty_inventory() {
    let f = Fixture::new();
    let marker = f.marker();
    let root = open("client", &f.folder, Some(&marker)).unwrap();
    assert!(crate::local::scan(&root, &crate::Cancellation::new())
        .unwrap()
        .entries
        .is_empty());
    assert!(matches!(
        open("client", &f.folder, Some(&marker)),
        Err(EngineError::SyncBusy)
    ));
    fs::write(f.folder.local.join("keep"), b"original").unwrap();
    let parked = f.base.join("detached");
    fs::rename(&f.folder.local, &parked).unwrap();
    assert_eq!(
        verify(&root, "client", &f.folder, Some(&marker)),
        Err(EngineError::RootSuspended(RootSuspension::LocalUnavailable))
    );
    drop(root);
    assert!(open("client", &f.folder, Some(&marker)).is_err());
    assert!(!f.folder.local.exists());
    fs::create_dir(&f.folder.local).unwrap();
    assert!(matches!(
        open("client", &f.folder, Some(&marker)),
        Err(EngineError::RootSuspended(
            RootSuspension::IdentityUnverified
        ))
    ));
    assert_eq!(fs::read_dir(&f.folder.local).unwrap().count(), 0);
    assert_eq!(fs::read(parked.join("keep")).unwrap(), b"original");
    fs::remove_dir(&f.folder.local).unwrap();
    fs::rename(parked, &f.folder.local).unwrap();
    assert!(open("client", &f.folder, Some(&marker)).is_ok());
}
#[test]
fn missing_corrupt_foreign_and_replaced_markers_do_not_rebind_or_write() {
    let f = Fixture::new();
    let marker = f.marker();
    let path = f.folder.local.join(".drivesync/identity.json");
    let bytes = fs::read(&path).unwrap();
    fs::remove_file(&path).unwrap();
    assert!(open("client", &f.folder, Some(&marker)).is_err());
    assert!(!path.exists());
    fs::write(&path, b"invalid").unwrap();
    assert!(open("client", &f.folder, Some(&marker)).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"invalid");
    fs::write(&path, &bytes).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["id"] = "replacement".into();
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(open("client", &f.folder, Some(&marker)).is_err());
    assert!(f
        .registry()
        .bind("client", &f.folder, "replacement")
        .is_err());
    fs::write(&path, bytes).unwrap();
    assert!(open("other-client", &f.folder, Some(&marker)).is_err());
    assert!(open("client", &f.folder, None).is_err());
    assert_eq!(
        f.registry().marker("client", &f.folder).unwrap().as_deref(),
        Some(marker.as_str())
    );
}
#[test]
fn suspension_survives_absent_volume_restart_and_repetition_without_changing_binding() {
    let f = Fixture::new();
    let marker = f.marker();
    let parked = f.base.join("parked");
    fs::rename(&f.folder.local, &parked).unwrap();
    for reason in [
        RootSuspension::LocalUnavailable,
        RootSuspension::RemoteAccessDenied,
        RootSuspension::Offline,
    ] {
        f.registry()
            .suspend("client", &f.folder, Some(reason))
            .unwrap();
        f.registry()
            .suspend("client", &f.folder, Some(reason))
            .unwrap();
        assert_eq!(
            f.registry().reason("client", &f.folder).unwrap(),
            Some(reason)
        );
        assert_eq!(
            f.registry().marker("client", &f.folder).unwrap(),
            Some(marker.clone())
        );
    }
    let mut other = f.folder.clone();
    other.account_id = AccountId("b".into());
    assert_eq!(f.registry().reason("client", &other).unwrap(), None);
    assert!(f.registry().suspend("client", &other, None).is_err());
    fs::rename(parked, &f.folder.local).unwrap();
    let root = open("client", &f.folder, Some(&marker)).unwrap();
    verify(&root, "client", &f.folder, Some(&marker)).unwrap();
    f.registry().suspend("client", &f.folder, None).unwrap();
    assert_eq!(f.registry().reason("client", &f.folder).unwrap(), None);
}
#[test]
fn n04_c25_root_causes_are_conservative_and_e09_item_errors_remain_unclassified() {
    for (error, reason) in [
        (
            EngineError::RemoteFolderUnavailable,
            RootSuspension::RemoteUnavailable,
        ),
        (
            EngineError::RemoteForbidden,
            RootSuspension::RemoteAccessDenied,
        ),
        (
            EngineError::Authentication(AuthError::Network),
            RootSuspension::Offline,
        ),
        (EngineError::RemoteRateLimited, RootSuspension::RemoteRetry),
    ] {
        assert_eq!(remote(error), EngineError::RootSuspended(reason));
    }
    // The mapper is invoked only by root probes; it never proves deletion.
    assert_eq!(remote(EngineError::StateStorage), EngineError::StateStorage);
    assert_eq!(
        remote(EngineError::DownloadConflict),
        EngineError::DownloadConflict
    );
}

#[test]
fn n03_revoked_local_access_does_not_probe_or_recreate_user_files() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let marker = f.marker();
    let before = fs::read(f.folder.local.join(".drivesync/identity.json")).unwrap();
    let root = open("client", &f.folder, Some(&marker)).unwrap();
    fs::set_permissions(&f.folder.local, fs::Permissions::from_mode(0o500)).unwrap();
    let result = verify(&root, "client", &f.folder, Some(&marker));
    fs::set_permissions(&f.folder.local, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        result,
        Err(EngineError::RootSuspended(RootSuspension::LocalUnavailable))
    );
    assert_eq!(
        fs::read(f.folder.local.join(".drivesync/identity.json")).unwrap(),
        before
    );
    assert_eq!(fs::read_dir(&f.folder.local).unwrap().count(), 1);
}

#[test]
fn external_registry_corruption_incompatibility_and_failed_updates_preserve_evidence() {
    let f = Fixture::new();
    let marker = f.marker();
    f.registry()
        .suspend("client", &f.folder, Some(RootSuspension::Offline))
        .unwrap();
    let db = f.base.join("app/roots.sqlite3");
    let connection = rusqlite::Connection::open(&db).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_availability BEFORE UPDATE ON bindings BEGIN SELECT RAISE(ABORT,'injected failure'); END;").unwrap();
    assert_eq!(
        f.registry().suspend("client", &f.folder, None),
        Err(EngineError::StateStorage)
    );
    assert_eq!(
        f.registry().reason("client", &f.folder).unwrap(),
        Some(RootSuspension::Offline)
    );
    assert_eq!(
        f.registry().marker("client", &f.folder).unwrap(),
        Some(marker)
    );
    connection.pragma_update(None, "user_version", 99).unwrap();
    drop(connection);
    let before = fs::read(&db).unwrap();
    assert!(Roots::open(&db).is_err());
    assert_eq!(fs::read(&db).unwrap(), before);
    fs::write(&db, b"corrupt registry").unwrap();
    assert!(Roots::open(&db).is_err());
    assert_eq!(fs::read(&db).unwrap(), b"corrupt registry");
}

#[test]
fn concurrent_binding_cannot_accept_a_different_marker() {
    let f = Fixture::new();
    // Initialize the file first; workers race on a new scope, not schema creation.
    let mut other = f.folder.clone();
    other.remote.id = "second-root".into();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let mut workers = Vec::new();
    for marker in ["one", "two"] {
        let path = f.base.join("app/roots.sqlite3");
        let folder = other.clone();
        let barrier = barrier.clone();
        workers.push(std::thread::spawn(move || {
            let registry = Roots::open(&path).unwrap();
            barrier.wait();
            registry.bind("client", &folder, marker).map(|()| marker)
        }));
    }
    let outcomes: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
    assert_eq!(outcomes.iter().filter(|r| r.is_ok()).count(), 1);
    let winner = outcomes.into_iter().find_map(Result::ok).unwrap();
    assert_eq!(
        f.registry().marker("client", &other).unwrap().as_deref(),
        Some(winner)
    );
}

#[test]
fn association_lock_survives_root_replacement_and_isolates_unrelated_work() {
    let f = Fixture::new();
    let registry = f.registry();
    let lock = registry.lock("client", &f.folder).unwrap();
    fs::rename(&f.folder.local, f.base.join("detached")).unwrap();
    fs::create_dir(&f.folder.local).unwrap();
    assert!(matches!(
        f.registry().lock("client", &f.folder),
        Err(EngineError::SyncBusy)
    ));
    let mut other = f.folder.clone();
    other.account_id = AccountId("independent".into());
    assert!(f.registry().lock("client", &other).is_ok());
    drop(lock);
    assert!(f.registry().lock("client", &f.folder).is_ok());
}

#[test]
fn association_lock_refuses_symlinks_without_touching_the_target() {
    let f = Fixture::new();
    let registry = f.registry();
    drop(registry.lock("client", &f.folder).unwrap());
    let path = fs::read_dir(f.base.join("app"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "lock"))
        .unwrap();
    let target = f.base.join("user-file");
    fs::write(&target, b"preserve").unwrap();
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(&target, &path).unwrap();
    assert!(matches!(
        registry.lock("client", &f.folder),
        Err(EngineError::StateStorage)
    ));
    assert_eq!(fs::read(target).unwrap(), b"preserve");
}

#[test]
fn transfer_transport_failure_cannot_clear_suspension_or_authorize_upload() {
    for error in [
        EngineError::Authentication(AuthError::Network),
        EngineError::RemoteRateLimited,
        EngineError::RootSuspended(RootSuspension::LocalUnavailable),
    ] {
        let report = crate::DownloadReport {
            issues: vec![crate::DownloadIssue {
                relative_path: Some("item".into()),
                error,
            }],
            ..Default::default()
        };
        assert!(matches!(
            super::report(&report),
            Err(EngineError::RootSuspended(_))
        ));
    }
    let report = crate::DownloadReport {
        issues: vec![crate::DownloadIssue {
            relative_path: Some("item".into()),
            error: EngineError::RemoteFolderUnavailable,
        }],
        ..Default::default()
    };
    assert!(super::report(&report).is_ok());
}

#[test]
fn n09_scope_lock_covers_changed_local_and_remote_roots() {
    let f = Fixture::new();
    let registry = f.registry();
    let active = registry.lock("client", &f.folder).unwrap();
    let mut changed = f.folder.clone();
    changed.local = f.base.join("new-root");
    changed.remote.id = "new-remote".into();
    assert!(matches!(
        f.registry().lock("client", &changed),
        Err(EngineError::SyncBusy)
    ));
    assert!(f.registry().lock("other-client", &changed).is_ok());
    drop(active);
    let reopened = f.registry();
    drop(reopened.lock("client", &changed).unwrap());
    assert!(reopened.lock("client", &changed).is_ok());
}

#[test]
fn n09_process_lock_holder() {
    let Some(base) = std::env::var_os("DRIVESYNC_SCOPE_LOCK_TEST") else {
        return;
    };
    let base = PathBuf::from(base);
    let registry = Roots::open(&base.join("app/roots.sqlite3")).unwrap();
    let folder: FolderConfiguration =
        serde_json::from_slice(&fs::read(base.join("scope.json")).unwrap()).unwrap();
    let _active = registry.lock("client", &folder).unwrap();
    fs::write(base.join("ready"), b"locked").unwrap();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
}

#[test]
fn n09_process_exit_releases_scope_without_deleting_lock_evidence() {
    let f = Fixture::new();
    fs::write(
        f.base.join("scope.json"),
        serde_json::to_vec(&f.folder).unwrap(),
    )
    .unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "coordinator::tests::n09_process_lock_holder",
            "--nocapture",
        ])
        .env("DRIVESYNC_SCOPE_LOCK_TEST", &f.base)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !f.base.join("ready").exists() && std::time::Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    if !f.base.join("ready").exists() {
        let _ = child.kill();
        let _ = child.wait();
        panic!("child did not acquire scope lock");
    }
    assert!(matches!(
        f.registry().lock("client", &f.folder),
        Err(EngineError::SyncBusy)
    ));
    child.kill().unwrap();
    child.wait().unwrap();
    let registry = f.registry();
    drop(registry.lock("client", &f.folder).unwrap());
    assert!(registry.lock("client", &f.folder).is_ok());
}

#[test]
fn e02_e07_capacity_and_credentials_persist_until_explicit_revalidation() {
    let f = Fixture::new();
    fs::write(f.folder.local.join("keep"), b"original").unwrap();
    for (error, reason) in [
        (
            EngineError::LocalStorageFull,
            RootSuspension::LocalStorageFull,
        ),
        (
            EngineError::RemoteStorageFull,
            RootSuspension::RemoteStorageFull,
        ),
        (
            EngineError::RemoteQuotaExceeded,
            RootSuspension::RemoteQuotaExceeded,
        ),
        (
            EngineError::Authentication(AuthError::ReauthorizationRequired),
            RootSuspension::AuthenticationRequired,
        ),
        (
            EngineError::Authentication(AuthError::MissingPermissions),
            RootSuspension::AuthenticationRequired,
        ),
    ] {
        assert_eq!(connection(error), EngineError::RootSuspended(reason));
        assert!(!reason.retryable());
        for _ in 0..2 {
            f.registry()
                .suspend("client", &f.folder, Some(reason))
                .unwrap();
            assert_eq!(
                f.registry().reason("client", &f.folder).unwrap(),
                Some(reason)
            );
            assert_eq!(fs::read(f.folder.local.join("keep")).unwrap(), b"original");
        }
        let root = open("client", &f.folder, Some(&f.marker())).unwrap();
        verify(&root, "client", &f.folder, Some(&f.marker())).unwrap();
        f.registry().suspend("client", &f.folder, None).unwrap();
        assert_eq!(f.registry().reason("client", &f.folder).unwrap(), None);
    }
}
