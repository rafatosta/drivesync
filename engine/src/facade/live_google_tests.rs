//! Opt-in real-Google integration smoke test.
//!
//! Requires isolated test credentials, a working vault and explicit opt-in.
use super::*;

#[test]
#[ignore = "explicit opt-in; accesses the real Google account and desktop vault"]
fn saved_account_can_browse_drive_folders() {
    assert_eq!(
        std::env::var("DRIVESYNC_LIVE_GOOGLE_TEST").as_deref(),
        Ok("1")
    );
    let engine = DriveSyncEngine::new();
    let accounts = engine.list_accounts().expect("saved profile index");
    let id = &accounts.first().expect("at least one saved account").id;
    let page = engine
        .drive_folders(id, "root", None)
        .unwrap_or_else(|e| panic!("Sanitized Drive diagnostic: {e:?}"));
    let config = auth::OAuthConfig::load()
        .map_err(|_| "OAuth configuration")
        .unwrap();
    engine
        .auth
        .with_access_config(&config, id, |token| remote::get(token, "root"))
        .unwrap_or_else(|e| panic!("Sanitized root metadata diagnostic: {e:?}"));
    if let Some(folder) = page.folders.first() {
        engine
            .drive_folders(id, &folder.id, None)
            .unwrap_or_else(|e| panic!("Sanitized child folder diagnostic: {e:?}"));
    }
    println!("Real Google folder listing and metadata retrieval succeeded. No account or folder identifiers printed.");
}
