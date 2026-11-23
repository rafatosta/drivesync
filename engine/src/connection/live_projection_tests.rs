//! Opt-in mixed projection validation, restricted to an explicitly isolated real test scope.
use super::*;
use crate::{
    inventory::{models::*, Inventory, InventoryConfig},
    models::*,
    synchronization::*,
};
use sha2::{Digest, Sha256};
use std::{
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::{symlink, MetadataExt},
    },
    path::PathBuf,
};
fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("explicit fixture required: {name}"))
}
#[tokio::test]
#[ignore = "requires the authorized real test account and an explicitly isolated projection fixture"]
async fn real_workspace_projection_mixed_scope_and_restart() {
    let phase = env("DRIVESYNC_STAGE9_PHASE");
    assert!(matches!(
        phase.as_str(),
        "sync" | "resume" | "verify" | "rename" | "edit"
    ));
    let local = PathBuf::from(env("DRIVESYNC_STAGE9_LOCAL_ROOT"));
    assert!(
        local.is_absolute()
            && local
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("DriveSync-stage9-validation-")
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
    let remote = RemoteId(env("DRIVESYNC_STAGE9_REMOTE_ROOT"));
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
        .starts_with("DriveSync-stage9-validation-"));
    if phase == "sync" {
        std::fs::create_dir(&local).unwrap();
        std::fs::write(local.join("local-good.bin"), b"stage9 supported binary\n").unwrap();
        symlink("local-good.bin", local.join("ignored-link")).unwrap();
        std::fs::write(
            local.join(std::ffi::OsString::from_vec(vec![0xff])),
            b"non UTF-8 name",
        )
        .unwrap();
        assert!(std::process::Command::new("mkfifo")
            .arg(local.join("ignored-fifo"))
            .status()
            .unwrap()
            .success());
        for (name, mime) in [
            ("Report", "application/vnd.google-apps.document"),
            ("Folder", "application/vnd.google-apps.folder"),
            ("bad/name", "application/octet-stream"),
            ("duplicate.bin", "application/octet-stream"),
            ("duplicate.bin", "application/octet-stream"),
            ("Collision", "application/vnd.google-apps.document"),
            ("Collision.desktop", "application/octet-stream"),
        ] {
            let file = engine
                .inner
                .client
                .files_create(
                    session,
                    GoogleFileWrite {
                        name: GooglePatch::Set(name.into()),
                        mime_type: GooglePatch::Set(MimeType(mime.into())),
                        parents: GooglePatch::Set(vec![FileId(remote.0.clone())]),
                        ..Default::default()
                    },
                    FileCreateOptions::default(),
                )
                .await
                .unwrap();
            if name == "Folder" {
                engine
                    .inner
                    .client
                    .files_create(
                        session,
                        GoogleFileWrite {
                            name: GooglePatch::Set("Inside".into()),
                            mime_type: GooglePatch::Set(MimeType(
                                "application/vnd.google-apps.spreadsheet".into(),
                            )),
                            parents: GooglePatch::Set(vec![file.id.unwrap()]),
                            ..Default::default()
                        },
                        FileCreateOptions::default(),
                    )
                    .await
                    .unwrap();
            }
        }
    }
    let mut hash = Sha256::new();
    for bytes in [
        account.0.as_bytes(),
        remote.0.as_bytes(),
        local.as_os_str().as_bytes(),
    ] {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
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
    let doc = before.iter().find(|r| {
        r.baseline.projection == Some(ProjectionKind::GoogleWorkspaceShortcut)
            && r.local_path
                .as_ref()
                .is_some_and(|p| p.components().count() == 1)
    });
    let inode = doc.map(|d| {
        std::fs::metadata(local.join(d.local_path.as_ref().unwrap()))
            .unwrap()
            .ino()
    });
    if phase == "rename" {
        let d = doc.unwrap();
        engine
            .inner
            .client
            .files_update(
                session,
                &FileId(d.identity.remote_id.as_ref().unwrap().0.clone()),
                GoogleFileWrite {
                    name: GooglePatch::Set("Renamed".into()),
                    ..Default::default()
                },
                FileUpdateOptions::default(),
            )
            .await
            .unwrap();
    }
    if phase == "edit" {
        std::fs::write(
            local.join(doc.unwrap().local_path.as_ref().unwrap()),
            b"user edited local projection\n",
        )
        .unwrap();
    }
    engine
        .start_synchronization(StartSynchronizationRequest {
            account_id: account.clone(),
            local_root: local.clone(),
            remote_root_id: remote,
        })
        .await
        .unwrap();
    for _ in 0..60 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let rows = inventory.enumerate(query.clone()).unwrap();
        if rows.len() == 4
            && rows
                .iter()
                .all(|r| r.baseline.state == BaselineState::Synced)
            && (phase != "rename" || local.join("Renamed.desktop").exists())
        {
            break;
        }
    }
    tokio::time::sleep(std::time::Duration::from_secs(8)).await;
    let status = engine
        .synchronization_status(account.clone())
        .await
        .unwrap();
    println!(
        "mixed scope status {:?}, issue {:?}",
        status.state, status.issue
    );
    engine.stop_synchronization(account).await.unwrap();
    let rows = inventory.enumerate(query).unwrap();
    println!("rows: {rows:#?}");
    assert_eq!(rows.len(), 4);
    assert!(rows
        .iter()
        .all(|r| r.baseline.state == BaselineState::Synced));
    assert_eq!(
        rows.iter()
            .filter(|r| r.baseline.projection == Some(ProjectionKind::GoogleWorkspaceShortcut))
            .count(),
        2
    );
    assert!(!local.join("Collision.desktop").exists());
    assert!(!local.join("duplicate.bin").exists());
    assert!(local.join("ignored-link").is_symlink());
    for row in rows
        .iter()
        .filter(|r| r.baseline.projection == Some(ProjectionKind::GoogleWorkspaceShortcut))
    {
        let path = local.join(row.local_path.as_ref().unwrap());
        if phase == "edit" && path.components().count() == local.components().count() + 1 {
            assert_eq!(
                std::fs::read(path).unwrap(),
                b"user edited local projection\n"
            );
        } else {
            assert_eq!(
                std::fs::read(path).unwrap(),
                crate::projections::link_bytes(row.identity.remote_id.as_ref().unwrap())
            );
        }
        let source = engine
            .inner
            .client
            .files_get(
                session,
                &FileId(row.identity.remote_id.as_ref().unwrap().0.clone()),
                FileGetOptions::default(),
            )
            .await
            .unwrap();
        assert!(matches!(
            crate::projections::kind(&source.mime_type.unwrap().0),
            crate::projections::RemoteKind::Workspace
        ));
    }
    if phase == "verify" || phase == "edit" {
        assert_eq!(
            before, rows,
            "restart or local edits must not replay effects or rewrite baseline"
        );
    }
    if phase == "rename" {
        let old = doc.unwrap();
        let new = rows
            .iter()
            .find(|r| r.identity.item_id == old.identity.item_id)
            .unwrap();
        assert_eq!(new.identity.remote_id, old.identity.remote_id);
        assert_eq!(
            std::fs::metadata(local.join("Renamed.desktop"))
                .unwrap()
                .ino(),
            inode.unwrap()
        );
    }
    println!("{phase}: four supported items converged; incompatible names, collisions and special files isolated");
}
