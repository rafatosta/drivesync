//! Opt-in real-Google integration smoke test.
//!
//! Requires isolated test credentials, a working vault and explicit opt-in.
use super::*;

/// Prepares only newly generated fixtures and a separate metadata-only profile.
/// Credentials stay in the existing vault namespace; the configured user root
/// and its account index are never rewritten. Actual cycles use `sync-cycle`.
#[test]
#[ignore = "explicit opt-in; creates a fresh disposable real Drive download fixture"]
fn prepare_isolated_download_fixture() {
    use super::live_resolution_tests::{create_file, create_root, external_edit};
    use std::{fs, os::unix::fs::DirBuilderExt, path::PathBuf};
    assert_eq!(
        std::env::var("DRIVESYNC_LIVE_GOOGLE_TEST").as_deref(),
        Ok("1")
    );
    let email = std::env::var("DRIVESYNC_TEST_ACCOUNT").expect("exact authorized test email");
    let path = PathBuf::from(
        std::env::var_os("DRIVESYNC_DOWNLOAD_EVIDENCE").expect("new evidence directory"),
    );
    assert!(path.is_absolute());
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&path)
        .expect("exclusive evidence directory");
    let configured = DriveSyncEngine::new();
    let accounts = configured.list_accounts().expect("existing account index");
    let matches: Vec<_> = accounts
        .into_iter()
        .filter(|a| a.email.as_deref() == Some(email.as_str()))
        .collect();
    assert_eq!(matches.len(), 1, "exactly one authorized account");
    let account = matches.into_iter().next().unwrap();
    let config = auth::OAuthConfig::load().expect("existing OAuth client");
    configured
        .auth
        .verify_with_config(&config, &account.id)
        .expect("existing vault credentials and Google identity");
    let label = format!(
        "DriveSync — Ensaio controlado downloads {}",
        oauth2::CsrfToken::new_random().secret()
    );
    let binary: Vec<u8> = (0..10 * 1024 * 1024).map(|i| (i % 251) as u8).collect();
    let root = configured
        .auth_upload_access(&config, &account.id, |token| {
            let root = create_root(token, &label);
            fs::write(
                path.join("created-remote-root.json"),
                serde_json::to_vec_pretty(&serde_json::json!({"id":root,"name":label})).unwrap(),
            )
            .unwrap();
            for (name, bytes) in [
                ("simple.txt", b"DriveSync download fixture\n".as_slice()),
                ("empty.bin", b"".as_slice()),
                ("binary.dat", binary.as_slice()),
            ] {
                let file = create_file(token, &root, name);
                fs::write(
                    path.join(format!("created-{name}.json")),
                    serde_json::to_vec_pretty(&serde_json::json!({"id":file.0,"parent":root}))
                        .unwrap(),
                )
                .unwrap();
                external_edit(token, &file, bytes);
            }
            Ok(root)
        })
        .expect("create only generated remote fixtures");
    let engine = DriveSyncEngine {
        accounts: accounts::Accounts::for_test(path.join("profile/drivesync/accounts.json")),
        ..DriveSyncEngine::new()
    };
    engine
        .accounts
        .record(&config.client_id, account.clone())
        .unwrap();
    let local = path.join("files");
    fs::create_dir(&local).unwrap();
    engine
        .configure_folder(&account.id, &root, &local)
        .expect("configure only isolated pair");
    let originals = path.join("originals");
    fs::create_dir(&originals).unwrap();
    fs::write(
        originals.join("simple.txt"),
        b"DriveSync download fixture\n",
    )
    .unwrap();
    fs::write(originals.join("empty.bin"), b"").unwrap();
    fs::write(originals.join("binary.dat"), &binary).unwrap();
    fs::write(path.join("fixture.json"), serde_json::to_vec_pretty(&serde_json::json!({"remote_root":root,"local_root":local,"profile":path.join("profile"),"account":email})).unwrap()).unwrap();
    println!("Disposable fixtures and isolated profile prepared; original root unchanged.");
}

/// Changes only the authenticated simple-file child in a generated fixture pair.
#[test]
#[ignore = "explicit opt-in; updates the simple file in an isolated download fixture"]
fn update_isolated_download_fixture() {
    use super::live_resolution_tests::{external_edit, CreatedFile};
    use std::{fs, path::PathBuf};
    assert_eq!(
        std::env::var("DRIVESYNC_LIVE_GOOGLE_TEST").as_deref(),
        Ok("1")
    );
    let email = std::env::var("DRIVESYNC_TEST_ACCOUNT").expect("exact authorized email");
    let path =
        PathBuf::from(std::env::var_os("DRIVESYNC_DOWNLOAD_EVIDENCE").expect("fixture directory"));
    assert!(path.is_absolute());
    let engine = DriveSyncEngine {
        accounts: accounts::Accounts::for_test(path.join("profile/drivesync/accounts.json")),
        ..DriveSyncEngine::new()
    };
    let accounts = engine.list_accounts().unwrap();
    assert_eq!(accounts.len(), 1);
    let account = &accounts[0];
    assert_eq!(account.email.as_deref(), Some(email.as_str()));
    let folder = engine.saved_folder(&account.id).unwrap().unwrap();
    assert!(folder
        .remote
        .name
        .starts_with("DriveSync — Ensaio controlado downloads "));
    assert_eq!(folder.local, path.join("files"));
    assert_ne!(folder.remote.id, "root");
    let inventory = engine.inventory(&account.id, &Cancellation::new()).unwrap();
    let files: Vec<_> = inventory
        .entries
        .iter()
        .filter(|entry| {
            entry.name == "simple.txt"
                && entry.parent_id == folder.remote.id
                && matches!(entry.kind, crate::InventoryKind::Binary)
        })
        .collect();
    assert_eq!(files.len(), 1);
    let bytes = b"DriveSync updated remote fixture\n";
    fs::write(path.join("expected-simple-v2.txt"), bytes).unwrap();
    let config = auth::OAuthConfig::load().unwrap();
    engine
        .auth_upload_access(&config, &account.id, |token| {
            external_edit(token, &CreatedFile(files[0].id.clone()), bytes);
            Ok(())
        })
        .unwrap();
    println!("Only the generated remote simple-file fixture was updated.");
}

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
