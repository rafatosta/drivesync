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
