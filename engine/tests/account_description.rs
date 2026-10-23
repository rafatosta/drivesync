//! Public facade persistence checks using isolated metadata and no Google/vault.
use drivesync_engine::{AccountId, DriveSyncEngine, EngineError};
use serde_json::json;
use std::fs;

#[test]
fn descriptions_survive_restart_and_preserve_account_metadata() {
    // This integration binary has one test, so environment overrides are isolated.
    let root = std::env::temp_dir().join(format!(
        "drivesync-description-{}",
        oauth2::CsrfToken::new_random().secret()
    ));
    fs::create_dir_all(root.join("drivesync")).unwrap();
    std::env::set_var("XDG_DATA_HOME", &root);
    std::env::set_var(
        "DRIVESYNC_GOOGLE_CLIENT_ID",
        "test.apps.googleusercontent.com",
    );
    std::env::remove_var("DRIVESYNC_GOOGLE_CLIENT_SECRET");
    let path = root.join("drivesync/accounts.json");
    let client = "test.apps.googleusercontent.com";
    let other = "other.apps.googleusercontent.com";
    let profiles = json!([
        {"id":"google:a", "provider_subject":"a", "email":"a@example.com"},
        {"id":"google:b", "provider_subject":"b", "email":"b@example.com"}
    ]);
    let folders = json!({client: [{"account_id":"google:a", "remote":{"id":"folder-a", "name":"Projects"}, "local":"/tmp/projects", "local_display":"/tmp/projects"}]});
    fs::write(&path, serde_json::to_vec(&json!({"version":2,"clients":{client:profiles.clone(),other:profiles},"folders":folders})).unwrap()).unwrap();
    let a = AccountId("google:a".into());
    let b = AccountId("google:b".into());
    let engine = DriveSyncEngine::new();
    assert_eq!(engine.account_description(&a).unwrap(), None);
    assert_eq!(
        engine
            .set_account_description(&a, "  Trabalho  ")
            .unwrap()
            .as_deref(),
        Some("Trabalho")
    );
    engine.set_account_description(&b, "Pessoal").unwrap();
    drop(engine);
    let restarted = DriveSyncEngine::new();
    assert_eq!(
        restarted.account_description(&a).unwrap().as_deref(),
        Some("Trabalho")
    );
    assert_eq!(
        restarted.account_description(&b).unwrap().as_deref(),
        Some("Pessoal")
    );
    let snapshot: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(snapshot["folders"], folders);
    assert_eq!(snapshot["clients"][client][0]["email"], "a@example.com");
    std::env::set_var("DRIVESYNC_GOOGLE_CLIENT_ID", other);
    assert_eq!(restarted.account_description(&a).unwrap(), None);
    restarted
        .set_account_description(&a, "Outro cliente")
        .unwrap();
    std::env::set_var("DRIVESYNC_GOOGLE_CLIENT_ID", client);
    assert_eq!(
        restarted.account_description(&a).unwrap().as_deref(),
        Some("Trabalho")
    );
    let before = fs::read(&path).unwrap();
    for value in ["x".repeat(129), "invalid\nlabel".into()] {
        assert_eq!(
            restarted.set_account_description(&a, &value),
            Err(EngineError::InvalidAccountDescription)
        );
        assert_eq!(fs::read(&path).unwrap(), before);
    }
    assert!(restarted
        .set_account_description(&AccountId("google:missing".into()), "Missing")
        .is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    let unicode = "á".repeat(128);
    restarted.set_account_description(&a, &unicode).unwrap();
    assert_eq!(restarted.account_description(&a).unwrap(), Some(unicode));
    assert_eq!(restarted.set_account_description(&a, "  ").unwrap(), None);
    assert_eq!(restarted.account_description(&a).unwrap(), None);
    assert_eq!(
        restarted.account_description(&b).unwrap().as_deref(),
        Some("Pessoal")
    );
    fs::write(&path, b"invalid index").unwrap();
    assert!(restarted.set_account_description(&b, "New name").is_err());
    assert_eq!(fs::read(&path).unwrap(), b"invalid index");
    fs::remove_dir_all(root).unwrap();
}
