//! Opt-in coordinator acceptance on an exclusive generated remote/local tree.
use super::live_resolution_tests::{create_file, create_root, external_edit, external_rename};
use super::*;
use serde_json::json;
use std::{
    fs::{self, File},
    io::Write,
    os::unix::fs::DirBuilderExt,
    path::Path,
};

fn record(output: &mut File, value: serde_json::Value) {
    writeln!(output, "{value}").unwrap();
    output.sync_all().unwrap();
}

#[test]
#[ignore = "explicit opt-in; exercises generated rename/edit combinations through real Drive synchronization"]
fn real_rename_combinations_use_the_coordinator() {
    assert_eq!(
        std::env::var("DRIVESYNC_REAL_RENAME_TEST").as_deref(),
        Ok("1")
    );
    let path =
        std::env::var_os("DRIVESYNC_REAL_RESOLUTION_EVIDENCE").expect("new evidence directory");
    let path = Path::new(&path);
    assert!(path.is_absolute());
    fs::DirBuilder::new()
        .mode(0o700)
        .create(path)
        .expect("exclusive new evidence directory");
    let mut output = File::create(path.join("steps.jsonl")).unwrap();
    let configured = DriveSyncEngine::new();
    let accounts = configured.list_accounts().unwrap();
    assert_eq!(accounts.len(), 1);
    let account = accounts[0].clone();
    let config = auth::OAuthConfig::load().unwrap();
    let label = format!(
        "DriveSync rename acceptance {}",
        oauth2::CsrfToken::new_random().secret()
    );
    let remote_root = configured
        .auth_upload_access(&config, &account.id, |token| Ok(create_root(token, &label)))
        .unwrap();
    record(
        &mut output,
        json!({"step":"created_root","remote_root":remote_root}),
    );
    let engine = DriveSyncEngine {
        accounts: crate::accounts::Accounts::for_test(path.join("profile/drivesync/accounts.json")),
        ..DriveSyncEngine::new()
    };
    engine
        .accounts
        .record(&config.client_id, account.clone())
        .unwrap();
    let local = path.join("files");
    fs::create_dir(&local).unwrap();
    let folder = engine
        .configure_folder(&account.id, &remote_root, &local)
        .unwrap();
    engine.set_upload_enabled(&folder, true).unwrap();
    for case in [
        "local-rename-remote-edit",
        "remote-rename-local-edit",
        "convergent-rename",
    ] {
        println!("Generated coordinator case: {case}");
        let original = format!("{case}.bin");
        let renamed = format!("{case}-renamed.bin");
        let file = engine
            .auth_upload_access(&config, &account.id, |token| {
                Ok(create_file(token, &remote_root, &original))
            })
            .unwrap();
        record(
            &mut output,
            json!({"step":"created_file","case":case,"file_id":file.0}),
        );
        let mut installed = false;
        for _ in 0..3 {
            engine
                .download_once(&account.id, &Cancellation::new(), |_| {})
                .unwrap();
            let bases = crate::state::Store::open(
                &configuration::identity::state_path(&folder, &config.client_id).unwrap(),
            )
            .unwrap()
            .confirmed_bases(&config.client_id, &account.id, &remote_root, &local)
            .unwrap();
            if bases.contains_key(&file.0) {
                installed = true;
                break;
            }
        }
        assert!(
            installed,
            "new generated file must have a confirmed baseline"
        );
        assert_eq!(fs::read(local.join(&original)).unwrap(), b"base");
        match case {
            "local-rename-remote-edit" => {
                fs::rename(local.join(&original), local.join(&renamed)).unwrap();
                engine
                    .auth_upload_access(&config, &account.id, |token| {
                        external_edit(token, &file, b"remote edit");
                        Ok(())
                    })
                    .unwrap();
            }
            "remote-rename-local-edit" => {
                fs::write(local.join(&original), b"local edit").unwrap();
                engine
                    .auth_upload_access(&config, &account.id, |token| {
                        external_rename(token, &file, &renamed);
                        Ok(())
                    })
                    .unwrap();
            }
            _ => {
                fs::rename(local.join(&original), local.join(&renamed)).unwrap();
                engine
                    .auth_upload_access(&config, &account.id, |token| {
                        external_rename(token, &file, &renamed);
                        Ok(())
                    })
                    .unwrap();
            }
        }
        let expected = match case {
            "local-rename-remote-edit" => b"remote edit".as_slice(),
            "remote-rename-local-edit" => b"local edit".as_slice(),
            _ => b"base".as_slice(),
        };
        let mut combined = false;
        for attempt in 0..4 {
            // A read-only full preparation exposes stable evidence independently
            // of change-feed delivery before the actual coordinator cycle.
            engine
                .prepare_downloads(&account.id, &Cancellation::new())
                .unwrap();
            let report = match engine.synchronize_once(&account.id, &Cancellation::new(), |_| {}) {
                Ok(report) => report,
                Err(EngineError::BulkReviewRequired(review)) => {
                    record(
                        &mut output,
                        json!({"step":"review_generated_batch","case":case}),
                    );
                    engine
                        .synchronize_reviewed(
                            &account.id,
                            &Cancellation::new(),
                            Some(&review),
                            |_| {},
                        )
                        .expect("reviewed generated fixture batch")
                }
                Err(error) => {
                    record(
                        &mut output,
                        json!({"step":"cycle_error","case":case,"attempt":attempt,"error":error.to_string()}),
                    );
                    continue;
                }
            };
            let bases = crate::state::Store::open(
                &configuration::identity::state_path(&folder, &config.client_id).unwrap(),
            )
            .unwrap()
            .confirmed_bases(&config.client_id, &account.id, &remote_root, &local)
            .unwrap();
            let at_target = bases
                .get(&file.0)
                .and_then(|base| base.location.as_ref())
                .is_some_and(|location| location.relative_path == std::path::Path::new(&renamed));
            if at_target
                && fs::read(local.join(&renamed)).ok().as_deref() == Some(expected)
                && !local.join(&original).exists()
            {
                record(
                    &mut output,
                    json!({"step":"combined","case":case,"updated":report.updated,"recovered":report.recovered}),
                );
                combined = true;
                break;
            }
        }
        assert!(
            combined,
            "coordinator did not combine the generated case within bounded cycles"
        );
        let inventory = engine.inventory(&account.id, &Cancellation::new()).unwrap();
        assert_eq!(
            inventory
                .entries
                .iter()
                .filter(|item| item.id == file.0)
                .count(),
            1
        );
        assert_eq!(
            inventory
                .entries
                .iter()
                .find(|item| item.id == file.0)
                .unwrap()
                .name,
            renamed
        );
        assert!(!inventory
            .entries
            .iter()
            .any(|item| item.id != file.0 && (item.name == original || item.name == renamed)));
        engine
            .synchronize_once(&account.id, &Cancellation::new(), |_| {})
            .expect("idempotent coordinator replay");
        assert_eq!(fs::read(local.join(&renamed)).unwrap(), expected);
        record(
            &mut output,
            json!({"step":"verified","case":case,"same_remote_id":true,"original_path_absent":true}),
        );
    }
    record(
        &mut output,
        json!({"step":"passed","installed_flatpak_validated":false,"personal_files_modified":false}),
    );
}
