//! Explicitly opted-in create-only upload acceptance on disposable portal roots.
use super::*;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::DirBuilderExt,
    path::PathBuf,
};

fn inputs() -> (PathBuf, String) {
    assert_eq!(
        std::env::var("DRIVESYNC_LIVE_GOOGLE_TEST").as_deref(),
        Ok("1")
    );
    let evidence = PathBuf::from(
        std::env::var_os("DRIVESYNC_UPLOAD_EVIDENCE").expect("exclusive test evidence"),
    );
    assert!(evidence.is_absolute());
    let email = std::env::var("DRIVESYNC_TEST_ACCOUNT").expect("authorized test email");
    (evidence, email)
}

/// Loads only the generated pair; never reconfigures a normal account profile.
fn fixture() -> (
    DriveSyncEngine,
    AccountSummary,
    FolderConfiguration,
    PathBuf,
) {
    let (evidence, email) = inputs();
    let engine = DriveSyncEngine {
        accounts: accounts::Accounts::for_test(evidence.join("profile/drivesync/accounts.json")),
        ..DriveSyncEngine::new()
    };
    let accounts = engine.list_accounts().unwrap();
    assert_eq!(accounts.len(), 1);
    let account = accounts[0].clone();
    assert_eq!(account.email.as_deref(), Some(email.as_str()));
    let folder = engine.saved_folder(&account.id).unwrap().unwrap();
    assert!(folder
        .remote
        .name
        .starts_with("DriveSync — Ensaio controlado uploads "));
    assert!(folder.local.starts_with("/run/flatpak/doc"));
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(evidence.join("fixture.json")).unwrap()).unwrap();
    assert_eq!(folder.local.to_str(), manifest["local_root"].as_str());
    assert_eq!(
        Some(folder.remote.id.as_str()),
        manifest["remote_root"].as_str()
    );
    (engine, account, folder, evidence)
}

#[test]
#[ignore = "explicit opt-in; creates a generated remote upload root and local portal fixtures"]
fn prepare_portal_upload_fixture() {
    let (evidence, email) = inputs();
    let local = PathBuf::from(
        std::env::var_os("DRIVESYNC_UPLOAD_PORTAL_ROOT").expect("empty disposable portal root"),
    );
    assert!(local.starts_with("/run/user") || local.starts_with("/run/flatpak/doc"));
    assert_eq!(fs::read_dir(&local).unwrap().count(), 0);
    fs::DirBuilder::new().mode(0o700).create(&evidence).unwrap();
    let configured = DriveSyncEngine::new();
    let accounts: Vec<_> = configured
        .list_accounts()
        .unwrap()
        .into_iter()
        .filter(|a| a.email.as_deref() == Some(email.as_str()))
        .collect();
    assert_eq!(accounts.len(), 1);
    let account = accounts[0].clone();
    let config = auth::OAuthConfig::load().unwrap();
    let label = format!(
        "DriveSync — Ensaio controlado uploads {}",
        oauth2::CsrfToken::new_random().secret()
    );
    let remote = configured
        .auth_upload_access(&config, &account.id, |token| {
            Ok(super::live_resolution_tests::create_root(token, &label))
        })
        .unwrap();
    fs::write(
        evidence.join("created-root.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"id":remote,"name":label})).unwrap(),
    )
    .unwrap();
    let engine = DriveSyncEngine {
        accounts: accounts::Accounts::for_test(evidence.join("profile/drivesync/accounts.json")),
        ..DriveSyncEngine::new()
    };
    engine.accounts.record(&config.client_id, account).unwrap();
    let id = engine.list_accounts().unwrap()[0].id.clone();
    let folder = engine.configure_folder(&id, &remote, &local).unwrap();
    assert_eq!(folder.local, configuration::access_path(&local).unwrap());
    assert!(!engine.upload_enabled(&folder.account_id).unwrap());
    fs::create_dir(evidence.join("originals")).unwrap();
    let binary: Vec<u8> = (0..3 * 1024 * 1024).map(|i| (i % 251) as u8).collect();
    for (name, bytes) in [
        ("binary.dat", binary.as_slice()),
        ("empty.txt", b"".as_slice()),
        (
            "simple.txt",
            b"DriveSync portal upload fixture\n".as_slice(),
        ),
    ] {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(folder.local.join(name))
            .unwrap();
        file.write_all(bytes).unwrap();
        file.sync_all().unwrap();
        fs::write(evidence.join("originals").join(name), bytes).unwrap();
    }
    fs::write(evidence.join("fixture.json"), serde_json::to_vec_pretty(&serde_json::json!({"remote_root":remote,"local_root":folder.local,"profile":evidence.join("profile"),"account":email})).unwrap()).unwrap();
    println!("Exclusive portal upload fixture prepared; uploads remain disabled.");
}

#[test]
#[ignore = "explicit opt-in; enables uploads only for the generated isolated test pair"]
fn enable_portal_upload_fixture() {
    let (engine, _, folder, _) = fixture();
    engine.set_upload_enabled(&folder, true).unwrap();
    assert!(engine.upload_enabled(&folder.account_id).unwrap());
    println!("Uploads enabled only in the isolated fixture.");
}

#[test]
#[ignore = "explicit opt-in; verifies remote bytes of generated upload fixtures"]
fn verify_portal_upload_fixture() {
    let (engine, account, folder, evidence) = fixture();
    let inventory = engine.inventory(&account.id, &Cancellation::new()).unwrap();
    let originals: Vec<_> = fs::read_dir(evidence.join("originals"))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(inventory.entries.len(), originals.len());
    let config = auth::OAuthConfig::load().unwrap();
    let mut verified = Vec::new();
    for original in originals {
        let name = original.file_name().into_string().unwrap();
        let bytes = fs::read(original.path()).unwrap();
        assert_eq!(fs::read(folder.local.join(&name)).unwrap(), bytes);
        let matches: Vec<_> = inventory
            .entries
            .iter()
            .filter(|item| {
                item.name == name
                    && item.parent_id == folder.remote.id
                    && item.kind == crate::InventoryKind::Binary
            })
            .collect();
        assert_eq!(matches.len(), 1, "one remote ID per generated filename");
        let item = matches[0];
        let received = engine
            .auth
            .with_access_config(&config, &account.id, |token| {
                let input = remote::file_media(token, &item.id)?;
                let mut received = Vec::new();
                input
                    .take(bytes.len() as u64 + 1)
                    .read_to_end(&mut received)
                    .map_err(|_| EngineError::DownloadIo)?;
                Ok(received)
            })
            .unwrap();
        assert_eq!(received.len(), bytes.len());
        let sha = format!("{:x}", Sha256::digest(&bytes));
        assert_eq!(format!("{:x}", Sha256::digest(&received)), sha);
        verified
            .push(serde_json::json!({"name":name,"id":item.id,"sha256":sha,"size":bytes.len()}));
    }
    fs::write(
        evidence.join("remote-verification.json"),
        serde_json::to_vec_pretty(&verified).unwrap(),
    )
    .unwrap();
    println!("Remote fixture bytes, hashes and unique identities verified.");
}
