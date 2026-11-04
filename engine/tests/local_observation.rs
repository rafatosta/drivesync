//! Public local-scope resolution uses persisted configuration without auth or writes.
use drivesync_engine::{AccountId, DriveSyncEngine};
use serde_json::json;
use std::fs;

#[test]
fn resolves_scoped_local_configuration_without_touching_files_or_credentials() {
    // One test in this binary isolates configuration environment overrides.
    let root = std::env::temp_dir().join(format!(
        "drivesync-observation-scope-{}",
        oauth2::CsrfToken::new_random().secret()
    ));
    fs::create_dir_all(root.join("drivesync")).unwrap();
    std::env::set_var("XDG_DATA_HOME", &root);
    let client = "local-test.apps.googleusercontent.com";
    let other = "other-test.apps.googleusercontent.com";
    std::env::set_var("DRIVESYNC_GOOGLE_CLIENT_ID", client);
    let first = root.join("first");
    let second = root.join("second");
    fs::create_dir(&first).unwrap();
    fs::create_dir(&second).unwrap();
    fs::write(first.join("user-file"), b"preserved").unwrap();
    let profile = json!([{"id":"google:a","provider_subject":"a","email":"a@example.com"}]);
    let folder = |local: &std::path::Path| json!([{"account_id":"google:a","remote":{"id":"remote","name":"Test"},"local":local,"local_display":local}]);
    let data = json!({"version":2,"clients":{client:profile.clone(),other:profile},"folders":{client:folder(&first),other:folder(&second)}});
    let index = root.join("drivesync/accounts.json");
    let original = serde_json::to_vec(&data).unwrap();
    fs::write(&index, &original).unwrap();
    let engine = DriveSyncEngine::new();
    let id = AccountId("google:a".into());
    let scope = engine.local_observation_scope(&id).unwrap().unwrap();
    assert_eq!(scope.client(), client);
    assert_eq!(scope.account(), id.0);
    assert_eq!(scope.root(), first);
    assert_eq!(
        engine
            .local_observation_scope(&AccountId("google:missing".into()))
            .unwrap(),
        None
    );
    std::env::set_var("DRIVESYNC_GOOGLE_CLIENT_ID", other);
    assert_eq!(
        engine.local_observation_scope(&id).unwrap().unwrap().root(),
        second
    );
    assert_eq!(fs::read(&index).unwrap(), original);
    assert_eq!(fs::read(first.join("user-file")).unwrap(), b"preserved");
    fs::write(&index, b"corrupt").unwrap();
    assert!(engine.local_observation_scope(&id).is_err());
    assert_eq!(fs::read(&index).unwrap(), b"corrupt");
    fs::remove_dir_all(root).unwrap();
}

/// Explicit read-only diagnosis against a saved configuration; no auth or writes.
#[test]
#[ignore = "requires explicit account label and permission to inspect its configured root"]
fn diagnose_saved_local_observer_without_modifying_files() {
    let id = AccountId(
        std::env::var("DRIVESYNC_LOCAL_DIAGNOSTIC_ACCOUNT").expect("explicit diagnostic account"),
    );
    let scope = DriveSyncEngine::new()
        .local_observation_scope(&id)
        .expect("scope resolution")
        .expect("saved scope");
    println!("resolved_root={:?}", scope.root());
    let mut watcher = drivesync_engine::LocalWatcher::start(
        scope,
        drivesync_engine::LocalWatchOptions::default(),
    )
    .expect("observer startup");
    let batch = watcher.poll();
    println!("state={:?}", batch.state);
    for notice in batch.notices {
        if let drivesync_engine::LocalActivity::Recheck(reason) = notice.activity {
            println!("coverage_gap={reason:?}");
        }
    }
}
