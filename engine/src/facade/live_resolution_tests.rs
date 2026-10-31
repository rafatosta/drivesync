//! Opt-in production-adapter acceptance against newly created disposable Drive data.
use super::*;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::{Read, Write},
    os::unix::fs::DirBuilderExt,
    path::Path,
    time::Duration,
};

fn client() -> Client {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .build()
        .unwrap()
}
fn response(response: reqwest::blocking::Response) -> Value {
    let status = response.status().as_u16();
    let mut bytes = vec![];
    response
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .expect("bounded test response");
    assert!(
        matches!(status, 200 | 201),
        "sanitized fixture HTTP status: {status}"
    );
    assert!(bytes.len() <= 1024 * 1024);
    serde_json::from_slice(&bytes).expect("fixture metadata JSON")
}
fn created_id(value: &Value) -> String {
    value["id"]
        .as_str()
        .filter(|id| crate::configuration::valid_id(id))
        .expect("new fixture identity")
        .into()
}
pub(super) fn create_root(token: &str, label: &str) -> String {
    created_id(&response(
        client()
            .post("https://www.googleapis.com/drive/v3/files")
            .bearer_auth(token)
            .query(&[("fields", "id")])
            .json(&json!({"name":label,"mimeType":"application/vnd.google-apps.folder"}))
            .send()
            .expect("fixture folder transport"),
    ))
}
pub(super) struct CreatedFile(pub(super) String);
pub(super) fn create_file(token: &str, parent: &str, name: &str) -> CreatedFile {
    let boundary = format!("fixture-{}", oauth2::CsrfToken::new_random().secret());
    let metadata = serde_json::to_vec(
        &json!({"name":name,"parents":[parent],"mimeType":"application/octet-stream"}),
    )
    .unwrap();
    let mut bytes = format!("--{boundary}\r\nContent-Type: application/json\r\n\r\n").into_bytes();
    bytes.extend(metadata);
    bytes.extend(format!("\r\n--{boundary}\r\nContent-Type: application/octet-stream\r\n\r\nbase\r\n--{boundary}--\r\n").as_bytes());
    CreatedFile(created_id(&response(
        client()
            .post("https://www.googleapis.com/upload/drive/v3/files")
            .bearer_auth(token)
            .query(&[("uploadType", "multipart"), ("fields", "id")])
            .header(
                reqwest::header::CONTENT_TYPE,
                format!("multipart/related; boundary={boundary}"),
            )
            .body(bytes)
            .send()
            .expect("fixture create transport"),
    )))
}
pub(super) fn external_edit(token: &str, file: &CreatedFile, bytes: &[u8]) {
    // This deliberately unconditional external writer is test-only and accepts
    // only a CreatedFile returned by this module's fresh fixture creation.
    response(
        client()
            .patch(format!(
                "https://www.googleapis.com/upload/drive/v3/files/{}",
                file.0
            ))
            .bearer_auth(token)
            .query(&[("uploadType", "media"), ("fields", "id")])
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .body(bytes.to_vec())
            .send()
            .expect("fixture external edit transport"),
    );
}
pub(super) fn external_rename(token: &str, file: &CreatedFile, name: &str) {
    response(
        client()
            .patch(format!(
                "https://www.googleapis.com/drive/v3/files/{}",
                file.0
            ))
            .bearer_auth(token)
            .query(&[("fields", "id")])
            .json(&json!({"name":name}))
            .send()
            .expect("fixture rename transport"),
    );
}
fn evidence(output: &mut File, event: Value) {
    writeln!(output, "{event}").unwrap();
    output.sync_all().unwrap();
}

#[test]
#[ignore = "explicit opt-in; creates and modifies generated fixtures in a new real Drive test folder"]
fn real_resolution_choices_use_the_production_adapter() {
    assert_eq!(
        std::env::var("DRIVESYNC_REAL_RESOLUTION_TEST").as_deref(),
        Ok("1")
    );
    let path = std::env::var_os("DRIVESYNC_REAL_RESOLUTION_EVIDENCE")
        .expect("new private evidence directory");
    let path = Path::new(&path);
    assert!(path.is_absolute());
    fs::DirBuilder::new()
        .mode(0o700)
        .create(path)
        .expect("new evidence directory");
    let mut output = File::create(path.join("steps.jsonl")).unwrap();
    let configured = DriveSyncEngine::new();
    let mut accounts = configured.list_accounts().expect("test profile");
    assert_eq!(
        accounts.len(),
        1,
        "exactly one explicitly connected test account"
    );
    let account = accounts.remove(0);
    let config = auth::OAuthConfig::load().expect("OAuth configuration");
    let label = format!(
        "DriveSync executor acceptance {}",
        oauth2::CsrfToken::new_random().secret()
    );
    let root = configured
        .auth_upload_access(&config, &account.id, |token| Ok(create_root(token, &label)))
        .expect("test root creation");
    evidence(
        &mut output,
        json!({"step":"created_root","remote_root":root}),
    );
    let engine = DriveSyncEngine {
        accounts: crate::accounts::Accounts::for_test(path.join("profile/accounts.json")),
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
        .expect("isolated fixture association");
    let cases = [
        (
            "keep-remote",
            crate::ResolutionChoice::KeepRemote,
            b"local version".to_vec(),
        ),
        (
            "keep-local",
            crate::ResolutionChoice::KeepLocal,
            vec![b'L'; 9 * 1024 * 1024 + 3],
        ),
        (
            "keep-both",
            crate::ResolutionChoice::KeepBoth,
            b"local version".to_vec(),
        ),
        ("empty-local", crate::ResolutionChoice::KeepLocal, vec![]),
    ];
    let selection =
        std::env::var("DRIVESYNC_REAL_RESOLUTION_CASE").unwrap_or_else(|_| "all".into());
    assert!(matches!(
        selection.as_str(),
        "all" | "keep-remote" | "keep-local" | "keep-both" | "empty-local"
    ));
    for (name, choice, selected) in cases {
        if selection != "all" && selection != name {
            continue;
        }
        println!("Generated real Drive case: {name}");
        let filename = format!("{name}.bin");
        let file = engine
            .auth_upload_access(&config, &account.id, |token| {
                Ok(create_file(token, &root, &filename))
            })
            .unwrap();
        evidence(
            &mut output,
            json!({"step":"created_file","case":name,"file_id":file.0}),
        );
        let initial = engine
            .download_once(&account.id, &Cancellation::new(), |_| {})
            .expect("initial managed download");
        assert!(
            initial.issues.is_empty(),
            "initial download returned sanitized issues: {:?}",
            initial.issues
        );
        assert_eq!(fs::read(local.join(&filename)).unwrap(), b"base");
        fs::write(local.join(&filename), &selected).unwrap();
        engine
            .auth_upload_access(&config, &account.id, |token| {
                external_edit(token, &file, b"remote version");
                Ok(())
            })
            .unwrap();
        let mut result = None;
        for attempt in 0..3 {
            // Use the explicit full read-only preparation so change-feed
            // propagation delay is not mistaken for a resolution failure.
            engine
                .prepare_downloads(&account.id, &Cancellation::new())
                .expect("fresh conflict observation");
            let record = engine
                .saved_conflicts(&account.id)
                .unwrap()
                .into_iter()
                .find(|record| {
                    record.remote_id.as_deref() == Some(file.0.as_str())
                        && record.diagnosis == crate::ChangeKind::ConcurrentChanges
                });
            let Some(record) = record else {
                evidence(
                    &mut output,
                    json!({"step":"diagnosis_not_ready","case":name,"attempt":attempt}),
                );
                continue;
            };
            let request = crate::ResolutionRequest::from_record(&record, choice).unwrap();
            match engine.resolve_conflict(&account.id, &request, &Cancellation::new()) {
                Ok(report) => {
                    result = Some((request, report));
                    break;
                }
                Err(EngineError::RemoteChanged) if attempt < 2 => evidence(
                    &mut output,
                    json!({"step":"observation_changed","case":name}),
                ),
                Err(error) => {
                    panic!("sanitized production resolution failure in {name}: {error:?}")
                }
            }
        }
        let (request, report) = result.expect("stable exact proposal within bounded attempts");
        let expected = if choice == crate::ResolutionChoice::KeepRemote {
            b"remote version".as_slice()
        } else {
            selected.as_slice()
        };
        assert!(
            fs::read(local.join(&filename)).unwrap() == expected,
            "selected bytes differ in {name}"
        );
        assert_eq!(
            engine
                .resolve_conflict(&account.id, &request, &Cancellation::new())
                .unwrap(),
            report
        );
        if let Some(copy) = &report.additional_path {
            assert_eq!(fs::read(local.join(copy)).unwrap(), b"remote version");
        }
        assert!(!engine
            .saved_conflicts(&account.id)
            .unwrap()
            .iter()
            .any(|record| record.remote_id.as_deref() == Some(&file.0)));
        evidence(
            &mut output,
            json!({"step":"verified","case":name,"operation_id":report.operation_id,"local_bytes":expected.len(),"active_path":report.path,"additional_path":report.additional_path,"preserved_paths":report.preserved_paths}),
        );
    }
    evidence(
        &mut output,
        json!({"step":"passed","case_selection":selection,"personal_files_modified":false,"installed_flatpak_validated":false}),
    );
}
