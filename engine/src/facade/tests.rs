//! Unit tests for explicit unsupported engine operations.
use super::*;

#[test]
fn unimplemented_operations_are_explicit() {
    let engine = DriveSyncEngine::new();
    assert_eq!(engine.status(), EngineStatus::NotConfigured);
    assert!(matches!(
        engine.start_sync(SyncRootId(1)),
        Err(EngineError::NotImplemented(_))
    ));
}

const CLIENT: &str = "test.apps.googleusercontent.com";
struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "drivesync-facade-{}",
            oauth2::CsrfToken::new_random().secret()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn accounts(&self) -> accounts::Accounts {
        accounts::Accounts::for_test(self.0.join("private/accounts.json"))
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
use std::fs;
fn profile(subject: &str, email: &str) -> AccountSummary {
    AccountSummary {
        id: AccountId::from_subject(subject),
        provider_subject: subject.into(),
        email: Some(email.into()),
    }
}
fn configured(subject: &str, local: std::path::PathBuf) -> FolderConfiguration {
    FolderConfiguration {
        account_id: AccountId::from_subject(subject),
        remote: crate::DriveFolder {
            id: format!("folder-{subject}"),
            name: "Projects".into(),
        },
        local_display: local.clone(),
        local,
    }
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
