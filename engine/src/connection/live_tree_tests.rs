//! Opt-in tree movement validation using only isolated roots in the authorized test account.
use super::*;
use crate::{
    inventory::{models::*, Inventory, InventoryConfig},
    models::*,
    synchronization::*,
};
use sha2::{Digest, Sha256};
use std::{os::unix::ffi::OsStrExt, path::PathBuf};
fn env(key: &str) -> String {
    std::env::var(key).expect(key)
}
async fn settled(
    inventory: &Inventory,
    query: &InventoryScopeQuery,
    paths: &[&str],
    content: &[u8],
) -> Vec<InventorySnapshot> {
    for _ in 0..120 {
        let rows = inventory.enumerate(query.clone()).unwrap();
        if rows.len() == paths.len() && paths.iter().all(|path| rows.iter().any(|r| {
            r.local_path.as_deref() == Some(std::path::Path::new(path)) && r.baseline.state == BaselineState::Synced
            && r.baseline.local.as_ref().is_some_and(|l| l.path == r.local_path)
            && r.baseline.remote.as_ref().zip(r.baseline.local.as_ref()).is_some_and(|(remote, local)| (local.name == remote.name && (local.kind == Evidence::Known(FileKind::Directory) || (local.size == remote.size && local.checksum == remote.checksum)))
                && (local.kind != Evidence::Known(FileKind::Regular) || matches!(&local.checksum,Evidence::Known(c) if c.value == format!("{:x}",Sha256::digest(content)))))
            && !r.pending_operations.iter().any(|p| matches!(p.state,OperationState::Pending|OperationState::Running|OperationState::AppliedUnconfirmed))
        })) { return rows; }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    panic!(
        "tree did not settle: {:?}",
        inventory
            .enumerate(query.clone())
            .unwrap()
            .iter()
            .map(|r| (
                &r.local_path,
                &r.baseline.state,
                r.revision,
                r.pending_operations
                    .iter()
                    .map(|p| (p.action, p.state, &p.recovery_diagnostic))
                    .collect::<Vec<_>>()
            ))
            .collect::<Vec<_>>()
    );
}
#[tokio::test]
#[ignore = "requires authorized test account and explicitly selected isolated local and remote roots"]
async fn real_tree_moves_edits_new_parent_and_restart_preserve_identities() {
    let phase = env("DRIVESYNC_STAGE7_PHASE");
    let local = PathBuf::from(env("DRIVESYNC_STAGE7_LOCAL_ROOT"));
    assert!(
        local.is_absolute()
            && local
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("DriveSync-stage7-validation-")
    );
    let directory = PathBuf::from(env("DRIVESYNC_REAL_ACCOUNTS_DIRECTORY"));
    let engine = Engine::open(EngineConfig {
        data_directory: directory.clone(),
        application_id: "com.rtosta.DriveSync.Devel".into(),
        google_oauth_file: Some(env("DRIVESYNC_REAL_OAUTH_FILE").into()),
    })
    .await
    .unwrap();
    let available = engine
        .list_accounts(ListAccountsRequest { provider: None })
        .await
        .unwrap();
    let accounts = available
        .accounts
        .iter()
        .filter(|a| a.email.as_deref() == Some("rtosta.coc@gmail.com"))
        .collect::<Vec<_>>();
    assert_eq!(accounts.len(), 1);
    let account = accounts[0].id.clone();
    let root = RemoteId(env("DRIVESYNC_STAGE7_REMOTE_ROOT"));
    let session = engine
        .inner
        .accounts
        .acquire_session(AcquireSessionRequest {
            account_id: account.clone(),
        })
        .await
        .unwrap();
    let metadata = engine
        .inner
        .client
        .files_get(
            session.session.google(),
            &FileId(root.0.clone()),
            FileGetOptions::default(),
        )
        .await
        .unwrap();
    assert!(metadata
        .name
        .unwrap()
        .starts_with("DriveSync-stage7-validation-"));
    let mut hash = Sha256::new();
    for bytes in [
        account.0.as_bytes(),
        root.0.as_bytes(),
        local.as_os_str().as_bytes(),
    ] {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    let query = InventoryScopeQuery {
        account_id: account.clone(),
        sync_root_id: SyncRootId(format!("{:x}", hash.finalize())),
    };
    let inventory = Inventory::open(InventoryConfig {
        directory: synchronization::directory(&directory),
        account_id: account.clone(),
    })
    .unwrap();
    let request = StartSynchronizationRequest {
        account_id: account.clone(),
        local_root: local.clone(),
        remote_root_id: root.clone(),
    };
    if phase != "verify" {
        let before = if phase == "sync" {
            let listing = engine
                .inner
                .client
                .files_list(
                    session.session.google(),
                    FileListOptions {
                        q: Some(format!("'{}' in parents and trashed = false", root.0)),
                        fields: Some(FieldMask("files(id),incompleteSearch,nextPageToken".into())),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
            assert!(
                listing.files.is_empty()
                    && listing.incomplete_search == Some(false)
                    && listing.next_page_token.is_none()
            );
            std::fs::create_dir_all(local.join("a/sub")).unwrap();
            std::fs::write(local.join("a/sub/file.bin"), b"tree0001").unwrap();
            engine.start_synchronization(request.clone()).await.unwrap();
            let before = settled(
                &inventory,
                &query,
                &["a", "a/sub", "a/sub/file.bin"],
                b"tree0001",
            )
            .await;
            engine.stop_synchronization(account.clone()).await.unwrap();
            std::fs::create_dir(local.join("destination")).unwrap();
            std::fs::rename(local.join("a"), local.join("destination/renamed")).unwrap();
            std::fs::rename(
                local.join("destination/renamed/sub/file.bin"),
                local.join("destination/renamed/sub/new.bin"),
            )
            .unwrap();
            std::fs::write(local.join("destination/renamed/sub/new.bin"), b"tree0002").unwrap();
            before
        } else {
            assert_eq!(phase, "resume");
            inventory.enumerate(query.clone()).unwrap()
        };
        engine.start_synchronization(request.clone()).await.unwrap();
        let moved = settled(
            &inventory,
            &query,
            &[
                "destination",
                "destination/renamed",
                "destination/renamed/sub",
                "destination/renamed/sub/new.bin",
            ],
            b"tree0002",
        )
        .await;
        for old in &before {
            assert!(moved.iter().any(|r| r.identity == old.identity));
        }
        println!("local rename+move+edit and new parent converged with original identities");
        let folder = moved
            .iter()
            .find(|r| r.local_path.as_deref() == Some(std::path::Path::new("destination/renamed")))
            .unwrap();
        let destination = moved
            .iter()
            .find(|r| r.local_path.as_deref() == Some(std::path::Path::new("destination")))
            .unwrap();
        engine
            .inner
            .client
            .files_update(
                session.session.google(),
                &FileId(folder.identity.remote_id.as_ref().unwrap().0.clone()),
                GoogleFileWrite {
                    name: GooglePatch::Set("returned".into()),
                    ..Default::default()
                },
                FileUpdateOptions {
                    add_parents: Some(vec![FileId(root.0.clone())]),
                    remove_parents: Some(vec![FileId(
                        destination.identity.remote_id.as_ref().unwrap().0.clone(),
                    )]),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let returned = settled(
            &inventory,
            &query,
            &[
                "destination",
                "returned",
                "returned/sub",
                "returned/sub/new.bin",
            ],
            b"tree0002",
        )
        .await;
        for old in &moved {
            assert!(returned.iter().any(|r| r.identity == old.identity));
        }
        assert_eq!(
            std::fs::read(local.join("returned/sub/new.bin")).unwrap(),
            b"tree0002"
        );
        println!("remote rename+move with descendants converged with original identities");
    } else {
        assert_eq!(phase, "verify");
        engine.start_synchronization(request).await.unwrap();
    }
    let rows = settled(
        &inventory,
        &query,
        &[
            "destination",
            "returned",
            "returned/sub",
            "returned/sub/new.bin",
        ],
        b"tree0002",
    )
    .await;
    tokio::time::sleep(std::time::Duration::from_secs(6)).await;
    assert_eq!(
        rows,
        inventory.enumerate(query.clone()).unwrap(),
        "own movement effects must stay quiet"
    );
    engine.stop_synchronization(account).await.unwrap();
    for row in rows {
        println!(
            "verified {:?} -> {:?}",
            row.local_path, row.identity.remote_id
        );
    }
}
