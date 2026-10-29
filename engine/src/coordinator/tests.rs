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
