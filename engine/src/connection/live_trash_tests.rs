//! Opt-in real recoverable trash propagation; every mutation is restricted to one explicit fixture.
use super::*;
use crate::{
    inventory::{models::*, Inventory, InventoryConfig},
    models::*,
    synchronization::*,
};
use sha2::{Digest, Sha256};
use std::{os::unix::ffi::OsStrExt, path::PathBuf};
fn env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("explicit fixture required: {key}"))
}
#[tokio::test]
#[ignore = "requires the authorized real test account, isolated scope and same-filesystem recoverable trash"]
async fn real_bidirectional_trash_scope_exit_and_restart() {
    let phase = env("DRIVESYNC_STAGE10_PHASE");
    assert!(matches!(
        phase.as_str(),
        "sync"
            | "local"
            | "remote"
            | "outside"
            | "verify"
            | "disabled"
            | "unavailable"
            | "local-outside"
            | "resume"
    ));
    let local = PathBuf::from(env("DRIVESYNC_STAGE10_LOCAL_ROOT"));
    assert!(
        local.is_absolute()
            && local
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("DriveSync-stage10-validation-")
    );
    let trash = PathBuf::from(env("XDG_DATA_HOME"));
    assert!(
        trash.is_absolute()
            && trash
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("DriveSync-stage10-trash-validation-")
    );
    let accounts = PathBuf::from(env("DRIVESYNC_REAL_ACCOUNTS_DIRECTORY"));
    let engine = Engine::open(EngineConfig {
        data_directory: accounts.clone(),
        application_id: "com.rtosta.DriveSync.Devel".into(),
        google_oauth_file: Some(env("DRIVESYNC_REAL_OAUTH_FILE").into()),
    })
    .await
    .unwrap();
    let available = engine
        .list_accounts(ListAccountsRequest { provider: None })
        .await
        .unwrap();
    let matching: Vec<_> = available
        .accounts
        .iter()
        .filter(|a| a.email.as_deref() == Some("rtosta.coc@gmail.com"))
        .collect();
    assert_eq!(matching.len(), 1);
    let account = matching[0].id.clone();
    let remote = RemoteId(env("DRIVESYNC_STAGE10_REMOTE_ROOT"));
    let acquired = engine
        .inner
        .accounts
        .acquire_session(AcquireSessionRequest {
            account_id: account.clone(),
        })
        .await
        .unwrap();
    let session = acquired.session.google();
    let root = engine
        .inner
        .client
        .files_get(
            session,
            &FileId(remote.0.clone()),
            FileGetOptions::default(),
        )
        .await
        .unwrap();
    assert!(root
        .name
        .unwrap()
        .starts_with("DriveSync-stage10-validation-"));
    if phase == "sync" {
        std::fs::create_dir(&local).unwrap();
        std::fs::create_dir(&trash).unwrap();
        for name in [
            "local-trash.bin",
            "remote-trash.bin",
            "outside.bin",
            "unavailable.bin",
        ] {
            std::fs::write(local.join(name), format!("stage10 recoverable {name}\n")).unwrap();
        }
        std::fs::create_dir(local.join("tree")).unwrap();
        std::fs::write(local.join("tree/child.bin"), b"stage10 retained child\n").unwrap();
    }
    let request = StartSynchronizationRequest {
        account_id: account.clone(),
        local_root: local.clone(),
        remote_root_id: remote,
    };
    let mut hash = Sha256::new();
    for b in [
        account.0.as_bytes(),
        request.remote_root_id.0.as_bytes(),
        local.as_os_str().as_bytes(),
    ] {
        hash.update((b.len() as u64).to_le_bytes());
        hash.update(b);
    }
    let inventory = Inventory::open(InventoryConfig {
        directory: synchronization::directory(&accounts),
        account_id: account.clone(),
    })
    .unwrap();
    let query = InventoryScopeQuery {
        account_id: account.clone(),
        sync_root_id: SyncRootId(format!("{:x}", hash.finalize())),
    };
    let before = inventory.enumerate(query.clone()).unwrap();
    let row = |name: &str| {
        before
            .iter()
            .find(|r| r.local_path.as_deref() == Some(std::path::Path::new(name)))
            .unwrap()
    };
    if phase == "local" && local.join("local-trash.bin").exists() {
        // Prepare only this isolated freedesktop fixture. GIO inside Flatpak can delegate
        // to the host portal, whose trash location is independent of this test data home.
        use std::io::Write;
        use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
        for path in [
            trash.join("Trash"),
            trash.join("Trash/files"),
            trash.join("Trash/info"),
        ] {
            if !path.exists() {
                std::fs::DirBuilder::new().mode(0o700).create(path).unwrap();
            }
        }
        let original = local.join("local-trash.bin");
        let encoded: String = original
            .as_os_str()
            .as_bytes()
            .iter()
            .map(|b| {
                if b.is_ascii_alphanumeric() || b"/-_.~".contains(b) {
                    (*b as char).to_string()
                } else {
                    format!("%{b:02X}")
                }
            })
            .collect();
        let mut receipt = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(trash.join("Trash/info/stage10-local-source.trashinfo"))
            .unwrap();
        writeln!(
            receipt,
            "[Trash Info]\nPath={encoded}\nDeletionDate=2026-11-23T00:00:00"
        )
        .unwrap();
        receipt.sync_all().unwrap();
        std::fs::rename(original, trash.join("Trash/files/stage10-local-source")).unwrap();
    }
    if matches!(phase.as_str(), "remote" | "unavailable") {
        let name = if phase == "remote" {
            "remote-trash.bin"
        } else {
            "unavailable.bin"
        };
        let id = row(name).identity.remote_id.as_ref().unwrap();
        engine
            .inner
            .client
            .files_update(
                session,
                &FileId(id.0.clone()),
                GoogleFileWrite {
                    trashed: GooglePatch::Set(true),
                    ..Default::default()
                },
                FileUpdateOptions::default(),
            )
            .await
            .unwrap();
    }
    if phase == "outside" {
        let parent = engine
            .inner
            .client
            .files_create(
                session,
                GoogleFileWrite {
                    name: GooglePatch::Set(
                        "DriveSync-stage10-outside-validation-2026-11-23".into(),
                    ),
                    mime_type: GooglePatch::Set(MimeType(
                        "application/vnd.google-apps.folder".into(),
                    )),
                    ..Default::default()
                },
                FileCreateOptions::default(),
            )
            .await
            .unwrap();
        let id = row("outside.bin").identity.remote_id.as_ref().unwrap();
        engine
            .inner
            .client
            .files_update(
                session,
                &FileId(id.0.clone()),
                GoogleFileWrite::default(),
                FileUpdateOptions {
                    add_parents: Some(vec![parent.id.unwrap()]),
                    remove_parents: Some(vec![FileId(request.remote_root_id.0.clone())]),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        // A whole remote folder trash must not remove its independently edited local child.
        std::fs::write(local.join("tree/child.bin"), b"edited child must survive\n").unwrap();
        let id = row("tree").identity.remote_id.as_ref().unwrap();
        engine
            .inner
            .client
            .files_update(
                session,
                &FileId(id.0.clone()),
                GoogleFileWrite {
                    trashed: GooglePatch::Set(true),
                    ..Default::default()
                },
                FileUpdateOptions::default(),
            )
            .await
            .unwrap();
    }
    if phase == "disabled" || phase == "local-outside" {
        std::fs::rename(local.join("outside.bin"), trash.join("moved-outside.bin")).unwrap();
    }
    if phase == "sync" || phase == "disabled" {
        engine.start_synchronization(request).await.unwrap();
    } else {
        engine
            .start_synchronization_with_trash_propagation(request)
            .await
            .unwrap();
    }
    for _ in 0..60 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let rows = inventory.enumerate(query.clone()).unwrap();
        let done = match phase.as_str() {
            "sync" => {
                rows.len() == 6
                    && rows
                        .iter()
                        .all(|r| r.baseline.state == BaselineState::Synced)
            }
            "local" | "remote" | "resume" => rows.iter().any(|r| {
                r.local_path.as_deref()
                    == Some(std::path::Path::new(if phase == "local" {
                        "local-trash.bin"
                    } else if phase == "resume" {
                        "unavailable.bin"
                    } else {
                        "remote-trash.bin"
                    }))
                    && r.baseline
                        .local
                        .as_ref()
                        .is_some_and(|s| s.state == FileState::Absent)
                    && r.baseline.remote.as_ref().is_some_and(|s| {
                        s.state == FileState::Absent && s.trashed == Evidence::Known(true)
                    })
            }),
            _ => true,
        };
        if done {
            break;
        }
    }
    tokio::time::sleep(std::time::Duration::from_secs(8)).await;
    println!(
        "phase {phase}: status {:?}",
        engine
            .synchronization_status(account.clone())
            .await
            .unwrap()
    );
    engine.stop_synchronization(account).await.unwrap();
    let rows = inventory.enumerate(query).unwrap();
    println!("rows: {rows:#?}");
    assert_eq!(rows.len(), 6);
    if phase == "sync" {
        assert!(rows
            .iter()
            .all(|r| r.baseline.state == BaselineState::Synced));
    }
    if phase == "local" || phase == "remote" || phase == "resume" {
        let name = if phase == "local" {
            "local-trash.bin"
        } else if phase == "resume" {
            "unavailable.bin"
        } else {
            "remote-trash.bin"
        };
        let after = rows
            .iter()
            .find(|r| r.identity.item_id == row(name).identity.item_id)
            .unwrap();
        assert_eq!(after.identity, row(name).identity);
        assert_eq!(
            after.baseline.local.as_ref().unwrap().state,
            FileState::Absent
        );
        assert_eq!(
            after.baseline.remote.as_ref().unwrap().state,
            FileState::Absent
        );
        assert!(after
            .pending_operations
            .iter()
            .all(|p| matches!(p.state, OperationState::Confirmed | OperationState::Failed)));
        let expected_action = if phase == "local" {
            OperationAction::TrashRemote
        } else {
            OperationAction::RemoveLocal
        };
        assert!(after
            .pending_operations
            .iter()
            .any(|p| p.action == expected_action && p.state == OperationState::Confirmed));
        assert!(!local.join(name).exists());
        let retained: Vec<_> = std::fs::read_dir(trash.join("Trash/files"))
            .unwrap()
            .map(|e| std::fs::read(e.unwrap().path()).unwrap())
            .collect();
        assert!(retained.contains(&format!("stage10 recoverable {name}\n").into_bytes()));
        let source = engine
            .inner
            .client
            .files_get(
                session,
                &FileId(after.identity.remote_id.as_ref().unwrap().0.clone()),
                FileGetOptions {
                    fields: Some(FieldMask("id,trashed".into())),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(source.trashed, Some(true));
    }
    if phase == "outside" {
        assert!(local.join("outside.bin").exists());
        assert_eq!(
            std::fs::read(local.join("tree/child.bin")).unwrap(),
            b"edited child must survive\n"
        );
        let after = rows
            .iter()
            .find(|r| r.identity.item_id == row("outside.bin").identity.item_id)
            .unwrap();
        assert_eq!(after, row("outside.bin"));
        let source = engine
            .inner
            .client
            .files_get(
                session,
                &FileId(after.identity.remote_id.as_ref().unwrap().0.clone()),
                FileGetOptions {
                    fields: Some(FieldMask("id,parents,trashed".into())),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(source.trashed, Some(false));
        assert_ne!(
            source.parents,
            Some(vec![FileId(env("DRIVESYNC_STAGE10_REMOTE_ROOT"))])
        );
    }
    if phase == "unavailable" {
        let refused = rows
            .iter()
            .find(|r| r.identity.item_id == row("unavailable.bin").identity.item_id)
            .unwrap();
        assert!(
            refused
                .pending_operations
                .iter()
                .any(|p| p.action == OperationAction::RemoveLocal
                    && p.state == OperationState::Failed),
            "unavailable trash must be exercised, not masked by unrelated deferred items"
        );
        assert_eq!(
            std::fs::read(local.join("unavailable.bin")).unwrap(),
            b"stage10 recoverable unavailable.bin\n"
        );
        for after in &rows {
            let prior = before
                .iter()
                .find(|r| r.identity.item_id == after.identity.item_id)
                .unwrap();
            assert_eq!(after.baseline, prior.baseline);
            assert!(after.pending_operations.iter().all(|p| !matches!(
                p.state,
                OperationState::Running | OperationState::AppliedUnconfirmed
            )));
        }
    }
    if phase == "verify" {
        for after in &rows {
            let prior = before
                .iter()
                .find(|r| r.identity.item_id == after.identity.item_id)
                .unwrap();
            assert_eq!(after.identity, prior.identity);
            assert_eq!(after.baseline, prior.baseline);
            assert_eq!(
                after.pending_operations, prior.pending_operations,
                "restart must not replay trash"
            );
            if after
                .baseline
                .local
                .as_ref()
                .is_some_and(|l| l.state == FileState::Absent)
            {
                assert_eq!(after, prior, "confirmed tombstones must remain untouched");
            }
        }
    }
    if phase == "disabled" || phase == "local-outside" {
        assert_eq!(rows, before);
        let source = engine
            .inner
            .client
            .files_get(
                session,
                &FileId(
                    row("outside.bin")
                        .identity
                        .remote_id
                        .as_ref()
                        .unwrap()
                        .0
                        .clone(),
                ),
                FileGetOptions {
                    fields: Some(FieldMask("id,trashed".into())),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(source.trashed, Some(false));
        std::fs::rename(trash.join("moved-outside.bin"), local.join("outside.bin")).unwrap();
    }
    println!("{phase}: verified isolated recoverable trash behavior");
}
