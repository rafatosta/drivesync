//! Opt-in real-account validation of initial uploads through the public Engine lifecycle.
use super::*;
use crate::{
    inventory::{models::*, Inventory, InventoryConfig},
    models::*,
    synchronization::*,
};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("explicit isolated fixture required: {name}"))
}

#[tokio::test]
#[ignore = "requires the explicitly selected authorized real test account and isolated remote fixture"]
async fn real_local_tree_converges_and_reopens_without_duplicate_uploads() {
    let phase = env("DRIVESYNC_STAGE5_PHASE");
    assert!(matches!(phase.as_str(), "sync" | "verify"));
    let local = PathBuf::from(env("DRIVESYNC_STAGE5_LOCAL_ROOT"));
    assert!(
        local.is_absolute()
            && local
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("DriveSync-stage5-validation-")
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
    let remote = RemoteId(env("DRIVESYNC_STAGE5_REMOTE_ROOT"));
    let session = engine
        .inner
        .accounts
        .acquire_session(AcquireSessionRequest {
            account_id: account.clone(),
        })
        .await
        .unwrap();
    let root = engine
        .inner
        .client
        .files_get(
            session.session.google(),
            &FileId(remote.0.clone()),
            FileGetOptions::default(),
        )
        .await
        .unwrap();
    assert!(root
        .name
        .unwrap()
        .starts_with("DriveSync-stage5-validation-"));
    if phase == "sync" {
        let existing = engine
            .inner
            .client
            .files_list(
                session.session.google(),
                FileListOptions {
                    q: Some(format!("'{}' in parents and trashed = false", remote.0)),
                    fields: Some(FieldMask("files(id),incompleteSearch,nextPageToken".into())),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(existing.incomplete_search, Some(false));
        assert!(
            existing.next_page_token.is_none() && existing.files.is_empty(),
            "remote fixture must start empty"
        );
        std::fs::create_dir(&local).unwrap();
        std::fs::create_dir_all(local.join("a/b/c")).unwrap();
        std::fs::write(local.join("a/empty.bin"), b"").unwrap();
        std::fs::write(local.join("root.bin"), b"DriveSync stage 5 real fixture\n").unwrap();
        std::fs::write(
            local.join("a/b/c/large.bin"),
            (0..8388608).map(|n| (n % 256) as u8).collect::<Vec<_>>(),
        )
        .unwrap();
    }
    let request = StartSynchronizationRequest {
        account_id: account.clone(),
        local_root: local.clone(),
        remote_root_id: remote,
    };
    use std::os::unix::ffi::OsStrExt;
    let mut hash = Sha256::new();
    for bytes in [
        request.account_id.0.as_bytes(),
        request.remote_root_id.0.as_bytes(),
        local.as_os_str().as_bytes(),
    ] {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    let scope = SyncRootId(format!("{:x}", hash.finalize()));
    let inventory = Inventory::open(InventoryConfig {
        directory: synchronization::directory(&accounts),
        account_id: account.clone(),
    })
    .unwrap();
    let query = InventoryScopeQuery {
        account_id: account.clone(),
        sync_root_id: scope,
    };
    let before = inventory.enumerate(query.clone()).unwrap();
    engine.start_synchronization(request).await.unwrap();
    for _ in 0..90 {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let rows = inventory.enumerate(query.clone()).unwrap();
        if rows.len() == 6
            && rows
                .iter()
                .all(|r| r.baseline.state == BaselineState::Synced)
        {
            break;
        }
    }
    // Let the unchanged tree pass through more source rounds before stopping.
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    let status = engine
        .synchronization_status(account.clone())
        .await
        .unwrap();
    println!(
        "final real scope status: {:?}, issue: {:?}",
        status.state, status.issue
    );
    engine.stop_synchronization(account).await.unwrap();
    let rows = inventory.enumerate(query).unwrap();
    assert_eq!(
        rows.len(),
        6,
        "all three folders and files must be registered"
    );
    for row in &rows {
        assert_eq!(
            row.baseline.state,
            BaselineState::Synced,
            "{:?}",
            row.local_path
        );
        assert_eq!(
            row.pending_operations.len(),
            1,
            "exactly one material operation per item"
        );
        assert_eq!(row.pending_operations[0].state, OperationState::Confirmed);
    }
    assert_eq!(std::fs::read(local.join("a/empty.bin")).unwrap(), b"");
    assert_eq!(
        std::fs::read(local.join("root.bin")).unwrap(),
        b"DriveSync stage 5 real fixture\n"
    );
    let large = std::fs::read(local.join("a/b/c/large.bin")).unwrap();
    let expected: Vec<_> = (0..8388608).map(|n| (n % 256) as u8).collect();
    assert_eq!(large, expected);
    println!(
        "large file: {} bytes, SHA-256 {:x}",
        large.len(),
        Sha256::digest(&large)
    );
    if phase == "verify" {
        assert_eq!(
            rows, before,
            "reopening must preserve identities, baselines, executions and revisions"
        );
    }
    println!("{phase}: six converged items, no implicit adoption, no duplicate execution");
}
