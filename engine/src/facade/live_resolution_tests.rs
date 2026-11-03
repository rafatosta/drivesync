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

/// Runs against generated files only; the configured profile supplies credentials
/// but its association, user files and synchronization state are never changed.
#[test]
#[ignore = "explicit opt-in; creates fresh disposable real Drive editor-save fixtures"]
fn real_editor_replacement_choices_and_repeated_saves() {
    assert_eq!(
        std::env::var("DRIVESYNC_REAL_RESOLUTION_TEST").as_deref(),
        Ok("1")
    );
    let selected_case =
        std::env::var("DRIVESYNC_REAL_EDITOR_CASE").unwrap_or_else(|_| "all".into());
    assert!(matches!(
        selected_case.as_str(),
        "all" | "local" | "remote" | "both"
    ));
    let rounds = std::env::var("DRIVESYNC_REAL_EDITOR_ROUNDS")
        .map(|v| v.parse::<usize>().expect("round count"))
        .unwrap_or(2);
    assert!((1..=2).contains(&rounds));
    let output_path = std::path::PathBuf::from(
        std::env::var_os("DRIVESYNC_REAL_RESOLUTION_EVIDENCE")
            .expect("new private evidence directory"),
    );
    assert!(output_path.is_absolute());
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&output_path)
        .unwrap();
    let mut output = File::create(output_path.join("steps.jsonl")).unwrap();
    let configured = DriveSyncEngine::new();
    let mut accounts = configured.list_accounts().expect("configured test profile");
    assert_eq!(accounts.len(), 1);
    let account = accounts.remove(0);
    let config = auth::OAuthConfig::load().unwrap();
    let root_id = configured
        .auth_upload_access(&config, &account.id, |token| {
            Ok(create_root(
                token,
                "DriveSync editor replacement acceptance",
            ))
        })
        .unwrap();
    evidence(
        &mut output,
        json!({"step":"created_root","remote_root":root_id}),
    );
    let engine = DriveSyncEngine {
        accounts: crate::accounts::Accounts::for_test(output_path.join("profile/accounts.json")),
        ..DriveSyncEngine::new()
    };
    engine
        .accounts
        .record(&config.client_id, account.clone())
        .unwrap();
    let portal_root =
        std::env::var_os("DRIVESYNC_REAL_RESOLUTION_PORTAL_ROOT").map(std::path::PathBuf::from);
    let local = portal_root
        .clone()
        .unwrap_or_else(|| output_path.join("files"));
    if portal_root.is_some() {
        assert!(
            local.starts_with("/run/flatpak/doc"),
            "explicit fresh portal root required"
        );
    }
    fs::create_dir(&local).unwrap();
    let folder = engine
        .configure_folder(&account.id, &root_id, &local)
        .unwrap();
    if portal_root.is_some() {
        assert_eq!(
            folder.local, local,
            "the test must retain the portal view instead of direct access"
        );
        let probe = local.join(".flock-capability-probe");
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&probe)
            .unwrap();
        assert!(matches!(
            rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockShared),
            Err(rustix::io::Errno::NOSYS | rustix::io::Errno::OPNOTSUPP)
        ));
        drop(file);
        fs::remove_file(probe).unwrap();
    }
    let cancel = Cancellation::new();
    for (name, choice) in [
        ("local", crate::ResolutionChoice::KeepLocal),
        ("remote", crate::ResolutionChoice::KeepRemote),
        ("both", crate::ResolutionChoice::KeepBoth),
    ] {
        if selected_case != "all" && selected_case != name {
            continue;
        }
        let filename = format!("{name}.txt");
        let file = configured
            .auth_upload_access(&config, &account.id, |token| {
                Ok(create_file(token, &root_id, &filename))
            })
            .unwrap();
        evidence(
            &mut output,
            json!({"step":"created_file","file_id":file.0,"case":name}),
        );
        let mut initialized = false;
        for attempt in 0..3 {
            let initial = engine.download_once(&account.id, &cancel, |_| {}).unwrap();
            if initial.issues.is_empty() {
                initialized = true;
                break;
            }
            assert!(
                initial
                    .issues
                    .iter()
                    .all(|issue| issue.error == EngineError::RemoteChanged),
                "sanitized initial download issues: {:?}",
                initial.issues
            );
            evidence(
                &mut output,
                json!({"step":"initial_observation_changed","case":name,"attempt":attempt}),
            );
        }
        assert!(
            initialized,
            "initial download must converge within bounded attempts"
        );
        for round in 0..rounds {
            let path = local.join(&filename);
            let _old = File::open(&path).unwrap();
            let selected = format!("editor save {round}").into_bytes();
            let temporary = local.join(".editor-save.tmp");
            fs::write(&temporary, &selected).unwrap();
            fs::rename(&temporary, &path).unwrap();
            let remote_bytes: &[u8] = if round == 0 {
                b"base"
            } else {
                b"concurrent Drive edit"
            };
            if round == 1 {
                configured
                    .auth_upload_access(&config, &account.id, |token| {
                        external_edit(token, &file, remote_bytes);
                        Ok(())
                    })
                    .unwrap();
            }
            let mut inspected = None;
            for attempt in 0..3 {
                engine
                    .prepare_downloads(&account.id, &cancel)
                    .expect("fresh replacement observation");
                let record = engine
                    .saved_conflicts(&account.id)
                    .unwrap()
                    .into_iter()
                    .find(|r| r.remote_id.as_deref() == Some(file.0.as_str()))
                    .expect("replacement pending");
                assert!(matches!(
                    record.diagnosis,
                    crate::ChangeKind::LocalIdentityChanged
                        | crate::ChangeKind::ConcurrentIdentityChanges
                ));
                match engine.compare_conflict(&account.id, &record, &cancel) {
                    Ok(comparison) => {
                        inspected = Some((record, comparison));
                        break;
                    }
                    Err(EngineError::RemoteChanged) if attempt < 2 => evidence(
                        &mut output,
                        json!({"step":"comparison_changed","case":name,"round":round,"attempt":attempt}),
                    ),
                    Err(error) => {
                        panic!("sanitized comparison failure in {name}/{round}: {error:?}")
                    }
                }
            }
            let (record, comparison) =
                inspected.expect("stable comparison within bounded attempts");
            assert_eq!(
                comparison.content,
                crate::ComparisonContent::Available {
                    local: selected.clone(),
                    remote: remote_bytes.into()
                }
            );
            let request = crate::ResolutionRequest::from_record(&record, choice).unwrap();
            let report = engine
                .resolve_conflict(&account.id, &request, &cancel)
                .expect("live replacement resolution");
            let expected = if choice == crate::ResolutionChoice::KeepRemote {
                remote_bytes
            } else {
                &selected
            };
            assert_eq!(fs::read(&path).unwrap(), expected);
            let preserved: Vec<_> = report
                .preserved_paths
                .iter()
                .map(|p| fs::read(local.join(p)).unwrap())
                .collect();
            assert!(preserved.contains(&selected));
            assert!(preserved.contains(&remote_bytes.to_vec()));
            if let Some(copy) = &report.additional_path {
                assert_eq!(fs::read(local.join(copy)).unwrap(), remote_bytes);
            }
            assert_eq!(
                engine
                    .resolve_conflict(&account.id, &request, &cancel)
                    .unwrap(),
                report
            );
            assert!(!engine
                .saved_conflicts(&account.id)
                .unwrap()
                .iter()
                .any(|r| r.remote_id.as_deref() == Some(file.0.as_str())));
            evidence(
                &mut output,
                json!({"step":"verified","case":name,"round":round,"operation_id":report.operation_id,"preserved_paths":report.preserved_paths}),
            );
        }
    }
    evidence(
        &mut output,
        json!({"step":"passed","case_selection":selected_case,"rounds":rounds,"portal_view":portal_root.is_some(),"personal_files_modified":false,"installed_flatpak_validated":false}),
    );
}

/// Resumes only a previously generated acceptance fixture after its test process
/// was interrupted. The configured personal profile is never used as a target.
#[test]
#[ignore = "explicit opt-in; resumes accepted work in an existing generated Drive fixture"]
fn real_editor_fixture_recovers_after_process_interruption() {
    assert_eq!(
        std::env::var("DRIVESYNC_REAL_RESOLUTION_TEST").as_deref(),
        Ok("1")
    );
    let path = std::path::PathBuf::from(
        std::env::var_os("DRIVESYNC_REAL_RESOLUTION_EVIDENCE")
            .expect("existing acceptance fixture"),
    );
    assert!(path.is_absolute());
    let events = fs::read_to_string(path.join("steps.jsonl")).unwrap();
    let created: Value = serde_json::from_str(events.lines().next().unwrap()).unwrap();
    assert_eq!(created["step"], "created_root");
    let engine = DriveSyncEngine {
        accounts: crate::accounts::Accounts::for_test(path.join("profile/accounts.json")),
        ..DriveSyncEngine::new()
    };
    let accounts = engine.list_accounts().unwrap();
    assert_eq!(accounts.len(), 1);
    let account = &accounts[0];
    let folder = engine.saved_folder(&account.id).unwrap().unwrap();
    let expected_local = std::env::var_os("DRIVESYNC_REAL_RESOLUTION_PORTAL_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| path.join("files"));
    assert_eq!(folder.local, expected_local);
    assert_eq!(
        Some(folder.remote.id.as_str()),
        created["remote_root"].as_str()
    );
    let before = engine.saved_resolutions(&account.id).unwrap();
    assert!(
        !before.is_empty(),
        "interrupted accepted intention required"
    );
    let reports = engine
        .resume_resolutions(&account.id, &Cancellation::new())
        .unwrap();
    assert!(!reports.is_empty());
    for report in &reports {
        for relative in &report.preserved_paths {
            assert!(folder.local.join(relative).is_file());
        }
        assert!(folder.local.join(&report.path).is_file());
    }
    assert!(engine.saved_resolutions(&account.id).unwrap().is_empty());
    assert!(engine.saved_conflicts(&account.id).unwrap().is_empty());
    let mut output = fs::OpenOptions::new()
        .append(true)
        .open(path.join("steps.jsonl"))
        .unwrap();
    evidence(
        &mut output,
        json!({"step":"recovered_after_process_interruption","verified_operations":reports.len(),"personal_files_modified":false,"power_loss_validated":false}),
    );
}
