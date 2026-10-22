use super::*;
use crate::DriveSyncEngine;
use std::os::unix::fs::PermissionsExt;
const CLIENT: &str = "test.apps.googleusercontent.com";
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "drivesync-profiles-{}",
            oauth2::CsrfToken::new_random().secret()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn accounts(&self) -> Accounts {
        Accounts {
            path: Some(self.0.join("private/accounts.json")),
            lock: Mutex::new(()),
        }
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn profile(subject: &str, email: &str) -> AccountSummary {
    AccountSummary {
        id: AccountId::from_subject(subject),
        provider_subject: subject.into(),
        email: Some(email.into()),
    }
}
#[test]
fn restart_updates_without_duplicates_and_isolates_clients() {
    let dir = Directory::new();
    let accounts = dir.accounts();
    assert!(accounts.list(CLIENT).unwrap().is_empty());
    accounts
        .record(CLIENT, profile("a", "old@example.com"))
        .unwrap();
    accounts
        .record(CLIENT, profile("b", "b@example.com"))
        .unwrap();
    accounts
        .record(
            "other.apps.googleusercontent.com",
            profile("c", "c@example.com"),
        )
        .unwrap();
    drop(accounts);
    let reopened = dir.accounts();
    reopened
        .record(CLIENT, profile("a", "new@example.com"))
        .unwrap();
    let list = reopened.list(CLIENT).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].email.as_deref(), Some("new@example.com"));
    assert_eq!(
        reopened.list("other.apps.googleusercontent.com").unwrap()[0].provider_subject,
        "c"
    );
    let file = dir.0.join("private/accounts.json");
    assert_eq!(
        fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(file.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    let text = fs::read_to_string(file).unwrap();
    for forbidden in [
        "access_token",
        "refresh_token",
        "client_secret",
        "Authenticated",
    ] {
        assert!(!text.contains(forbidden));
    }
}
#[test]
fn corrupt_future_duplicate_and_foreign_identity_are_preserved() {
    let dir = Directory::new();
    let accounts = dir.accounts();
    accounts
        .record(CLIENT, profile("a", "a@example.com"))
        .unwrap();
    let path = accounts.path.as_ref().unwrap();
    let original: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let mut future = original.clone();
    future["version"] = 999.into();
    let mut mismatch = original.clone();
    mismatch["clients"][CLIENT][0]["id"] = "google:wrong".into();
    let mut duplicate = original.clone();
    duplicate["clients"][CLIENT]
        .as_array_mut()
        .unwrap()
        .push(original["clients"][CLIENT][0].clone());
    for data in [
        b"broken".to_vec(),
        serde_json::to_vec(&future).unwrap(),
        serde_json::to_vec(&mismatch).unwrap(),
        serde_json::to_vec(&duplicate).unwrap(),
    ] {
        fs::write(path, &data).unwrap();
        assert_eq!(accounts.list(CLIENT), Err(AuthError::AccountStorage));
        assert_eq!(
            accounts.record(CLIENT, profile("b", "b@example.com")),
            Err(AuthError::AccountStorage)
        );
        assert_eq!(fs::read(path).unwrap(), data);
    }
}
#[test]
fn failed_write_is_reported_without_an_in_memory_success() {
    let dir = Directory::new();
    fs::write(dir.0.join("private"), "blocked parent").unwrap();
    let accounts = dir.accounts();
    assert_eq!(
        accounts.record(CLIENT, profile("a", "a@example.com")),
        Err(AuthError::AccountStorage)
    );
    assert_eq!(
        fs::read_to_string(dir.0.join("private")).unwrap(),
        "blocked parent"
    );
}
#[test]
fn startup_failures_are_per_account_retry_updates_and_no_cached_authentication() {
    let dir = Directory::new();
    let accounts = dir.accounts();
    accounts
        .record(CLIENT, profile("a", "a@example.com"))
        .unwrap();
    accounts
        .record(CLIENT, profile("b", "old@example.com"))
        .unwrap();
    let engine = DriveSyncEngine {
        accounts: dir.accounts(),
        ..DriveSyncEngine::default()
    };
    for error in [
        AuthError::Network,
        AuthError::SecureStorage,
        AuthError::ReauthorizationRequired,
    ] {
        let results = engine
            .restore_with(CLIENT, |id| {
                if id == &AccountId::from_subject("a") {
                    Err(error)
                } else {
                    Ok(profile("b", "new@example.com"))
                }
            })
            .unwrap();
        assert_eq!(results[0].error, Some(error));
        assert!(results[1].error.is_none());
        assert_eq!(engine.accounts.list(CLIENT).unwrap().len(), 2);
    }
    let retry = engine
        .restore_with(CLIENT, |id| {
            Ok(profile(
                id.0.strip_prefix("google:").unwrap(),
                "verified@example.com",
            ))
        })
        .unwrap();
    assert!(retry.iter().all(|r| r.error.is_none()));
    let wrong = engine
        .restore_with(CLIENT, |_| Ok(profile("foreign", "foreign@example.com")))
        .unwrap();
    assert!(wrong
        .iter()
        .all(|r| r.error == Some(AuthError::IdentityMismatch)));
    assert_eq!(dir.accounts().list(CLIENT).unwrap().len(), 2);
}

#[test]
fn concurrent_updates_are_serialized_and_uncommitted_temp_is_ignored() {
    let dir = Directory::new();
    let accounts = std::sync::Arc::new(dir.accounts());
    let threads: Vec<_> = (0..8)
        .map(|i| {
            let accounts = accounts.clone();
            std::thread::spawn(move || {
                accounts
                    .record(CLIENT, profile(&i.to_string(), "test@example.com"))
                    .unwrap()
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    fs::write(dir.0.join("private/.accounts-interrupted.tmp"), b"partial").unwrap();
    assert_eq!(dir.accounts().list(CLIENT).unwrap().len(), 8);
}

#[test]
fn invalid_profile_and_oversized_index_fail_closed() {
    let dir = Directory::new();
    let accounts = dir.accounts();
    accounts
        .record(CLIENT, profile("a", "a@example.com"))
        .unwrap();
    let path = accounts.path.as_ref().unwrap();
    let old = fs::read(path).unwrap();
    assert_eq!(
        accounts.record(CLIENT, profile("", "a@example.com")),
        Err(AuthError::AccountStorage)
    );
    assert_eq!(fs::read(path).unwrap(), old);
    fs::write(path, vec![b' '; MAX_INDEX as usize + 1]).unwrap();
    assert_eq!(accounts.list(CLIENT), Err(AuthError::AccountStorage));
}

fn configured(account: &str, path: PathBuf) -> FolderConfiguration {
    FolderConfiguration {
        account_id: AccountId::from_subject(account),
        remote: crate::DriveFolder {
            id: format!("folder-{account}"),
            name: "Projects".into(),
        },
        local_display: path.clone(),
        local: path,
    }
}
#[test]
fn folder_settings_survive_restart_auth_failure_and_profile_updates() {
    let dir = Directory::new();
    let accounts = dir.accounts();
    accounts
        .record(CLIENT, profile("a", "a@example.com"))
        .unwrap();
    let local = dir.0.join("downloads");
    fs::create_dir(&local).unwrap();
    let folder = configured("a", local);
    accounts.configure(CLIENT, folder.clone()).unwrap();
    let reopened = dir.accounts();
    reopened
        .record(CLIENT, profile("a", "changed@example.com"))
        .unwrap();
    assert_eq!(reopened.folders(CLIENT).unwrap(), vec![folder.clone()]);
    assert!(reopened
        .folders("other.apps.googleusercontent.com")
        .unwrap()
        .is_empty());
    let engine = DriveSyncEngine {
        accounts: reopened,
        ..DriveSyncEngine::default()
    };
    let restored = engine
        .restore_with(CLIENT, |_| Err(AuthError::Network))
        .unwrap();
    assert_eq!(restored[0].folder, Some(folder));
    assert_eq!(restored[0].error, Some(AuthError::Network));
}
#[test]
fn legacy_v1_index_is_rejected_without_mutation() {
    let dir = Directory::new();
    let accounts = dir.accounts();
    accounts
        .record(CLIENT, profile("a", "a@example.com"))
        .unwrap();
    let path = accounts.path.as_ref().unwrap();
    let mut legacy: serde_json::Value =
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    legacy["version"] = 1.into();
    legacy.as_object_mut().unwrap().remove("folders");
    let original = serde_json::to_vec(&legacy).unwrap();
    fs::write(path, &original).unwrap();
    assert_eq!(accounts.list(CLIENT), Err(AuthError::AccountStorage));
    assert_eq!(
        accounts.record(CLIENT, profile("b", "b@example.com")),
        Err(AuthError::AccountStorage)
    );
    assert_eq!(fs::read(path).unwrap(), original);
    assert!(!path.with_extension("v1.json.bak").exists());
}
#[test]
fn local_roots_reject_exact_nested_and_symlink_aliases_without_losing_old_settings() {
    let dir = Directory::new();
    let accounts = dir.accounts();
    for id in ["a", "b"] {
        accounts
            .record(CLIENT, profile(id, "test@example.com"))
            .unwrap();
    }
    let parent = dir.0.join("downloads");
    let child = parent.join("nested");
    fs::create_dir_all(&child).unwrap();
    let alias = dir.0.join("alias");
    std::os::unix::fs::symlink(&parent, &alias).unwrap();
    accounts
        .configure(CLIENT, configured("a", parent.clone()))
        .unwrap();
    for path in [&parent, &child, &alias] {
        assert_eq!(
            accounts.configure(CLIENT, configured("b", path.clone())),
            Err(EngineError::FolderOverlap)
        );
    }
    assert_eq!(accounts.folders(CLIENT).unwrap().len(), 1);
    // A textual prefix is a distinct directory, not an ancestor.
    let other = dir.0.join("downloads-other");
    fs::create_dir(&other).unwrap();
    accounts.configure(CLIENT, configured("b", other)).unwrap();
    accounts.configure(CLIENT, configured("a", child)).unwrap();
    assert_eq!(
        accounts.configure(CLIENT, configured("b", parent)),
        Err(EngineError::FolderOverlap)
    );
    assert_eq!(accounts.folders(CLIENT).unwrap().len(), 2);
}
#[test]
fn local_validation_canonicalizes_aliases_and_rejects_non_directories() {
    let dir = Directory::new();
    let target = dir.0.join("valid");
    fs::create_dir(&target).unwrap();
    let alias = dir.0.join("alias");
    std::os::unix::fs::symlink(&target, &alias).unwrap();
    assert_eq!(crate::configuration::local_folder(&alias).unwrap(), target);
    assert!(crate::configuration::local_folder(std::path::Path::new("relative")).is_err());
    assert!(crate::configuration::local_folder(&dir.0.join("missing")).is_err());
    let file = dir.0.join("file");
    fs::write(&file, "keep").unwrap();
    assert!(crate::configuration::local_folder(&file).is_err());
    assert_eq!(fs::read_to_string(file).unwrap(), "keep");
}

#[test]
fn forgetting_account_removes_its_metadata_but_keeps_user_files() {
    let dir = Directory::new();
    let accounts = dir.accounts();
    accounts
        .record(CLIENT, profile("a", "forgotten@example.com"))
        .unwrap();
    accounts
        .record(CLIENT, profile("b", "keep@example.com"))
        .unwrap();
    accounts
        .record(
            "other.apps.googleusercontent.com",
            profile("a", "other-client@example.com"),
        )
        .unwrap();
    let local = dir.0.join("downloads");
    fs::create_dir(&local).unwrap();
    fs::write(local.join("user-file.txt"), "keep my files").unwrap();
    accounts
        .configure(CLIENT, configured("a", local.clone()))
        .unwrap();
    let path = accounts.path.as_ref().unwrap();
    accounts
        .forget(CLIENT, &AccountId::from_subject("a"))
        .unwrap();
    accounts
        .forget(CLIENT, &AccountId::from_subject("a"))
        .unwrap();
    let reopened = dir.accounts();
    assert_eq!(
        reopened.list(CLIENT).unwrap(),
        vec![profile("b", "keep@example.com")]
    );
    assert!(reopened.folders(CLIENT).unwrap().is_empty());
    assert_eq!(
        reopened
            .list("other.apps.googleusercontent.com")
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        fs::read_to_string(local.join("user-file.txt")).unwrap(),
        "keep my files"
    );
    assert!(!fs::read_to_string(path)
        .unwrap()
        .contains("forgotten@example.com"));
    assert!(!path.with_extension("v1.json.bak").exists());
}
