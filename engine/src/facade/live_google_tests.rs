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

/// Runs the isolated precondition experiment without exporting vault credentials.
///
/// Requires a dedicated profile containing exactly one test account, an explicit
/// opt-in and a new private evidence directory. Only the child pipe receives the
/// short-lived access token; arguments, environment and test output never do.
#[test]
#[ignore = "explicit opt-in; creates disposable files in a dedicated real Google test account"]
fn dedicated_account_can_probe_direct_write_preconditions() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    assert_eq!(
        std::env::var("DRIVESYNC_PRECONDITION_PROBE").as_deref(),
        Ok("1"),
        "explicit real-write opt-in required"
    );
    let evidence = std::env::var_os("DRIVESYNC_PROBE_EVIDENCE")
        .expect("new absolute private evidence directory required");
    let write_api = std::env::var("DRIVESYNC_PROBE_WRITE_API").unwrap_or_else(|_| "v3".into());
    assert!(
        matches!(write_api.as_str(), "v3" | "v2"),
        "explicit API version"
    );
    let kind = std::env::var("DRIVESYNC_PROBE_KIND").unwrap_or_else(|_| "all".into());
    assert!(
        matches!(kind.as_str(), "all" | "name" | "media" | "resumable"),
        "explicit endpoint kind"
    );
    let engine = DriveSyncEngine::new();
    let accounts = engine.list_accounts().expect("dedicated profile index");
    assert_eq!(
        accounts.len(),
        1,
        "dedicated profile must contain one test account"
    );
    let config = auth::OAuthConfig::load().expect("OAuth configuration");
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../scripts/check-drive-preconditions.py");
    let result = engine.auth_upload_access(&config, &accounts[0].id, |token| {
        let mut child = Command::new("python3")
            .arg("-B")
            .arg(&script)
            .arg("--run-real")
            .arg("--token-stdin")
            .arg("--write-api")
            .arg(&write_api)
            .arg("--kind")
            .arg(&kind)
            .arg("--evidence-dir")
            .arg(&evidence)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| EngineError::InvalidPlan)?;
        let sent = child
            .stdin
            .take()
            .ok_or(EngineError::InvalidPlan)
            .and_then(|mut input| writeln!(input, "{token}").map_err(|_| EngineError::InvalidPlan));
        if sent.is_err() {
            let _ = child.kill();
        }
        let status = child.wait().map_err(|_| EngineError::InvalidPlan)?;
        sent?;
        Ok(status.success())
    });
    assert!(
        result.expect("sanitized authentication/experiment failure"),
        "precondition experiment incomplete; inspect private evidence; production writes remain disabled"
    );
}
