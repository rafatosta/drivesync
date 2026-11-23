//! Opt-in real continuous edits through the public Engine and the isolated test account.
use super::*;
use crate::{
    inventory::{models::*, Inventory, InventoryConfig},
    models::*,
    synchronization::*,
};
use sha2::{Digest, Sha256};
use std::{
    os::unix::{ffi::OsStrExt, fs::PermissionsExt},
    path::PathBuf,
};
fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("explicit isolated fixture required: {name}"))
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn rows(inventory: &Inventory, query: &InventoryScopeQuery) -> Vec<InventorySnapshot> {
    inventory.enumerate(query.clone()).unwrap()
}
async fn converged(
    inventory: &Inventory,
    query: &InventoryScopeQuery,
    bytes: &[u8],
) -> InventorySnapshot {
    for _ in 0..60 {
        let current = rows(inventory, query);
        if current.len() == 1
            && current[0].baseline.state == BaselineState::Synced
            && current[0]
                .baseline
                .local
                .as_ref()
                .is_some_and(|l| matches!(&l.checksum,Evidence::Known(c) if c.value==digest(bytes)))
            && current[0]
                .baseline
                .remote
                .as_ref()
                .is_some_and(|r| matches!(&r.checksum,Evidence::Known(c) if c.value==digest(bytes)))
        {
            return current[0].clone();
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    let current = rows(inventory, query);
    panic!(
        "scope did not converge: {:?}",
        current
            .iter()
            .map(|r| (
                &r.baseline.state,
                r.revision,
                r.pending_operations
                    .iter()
                    .map(|o| (o.action, o.state, &o.recovery_diagnostic))
                    .collect::<Vec<_>>()
            ))
            .collect::<Vec<_>>()
    );
}
struct RestorePermissions(PathBuf);
impl Drop for RestorePermissions {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o600));
    }
}
async fn remote_edit(engine: &Engine, account: &AccountId, id: &RemoteId, bytes: &[u8]) {
    let acquired = engine
        .inner
        .accounts
        .acquire_session(AcquireSessionRequest {
            account_id: account.clone(),
        })
        .await
        .unwrap();
    let session = acquired.session.google();
    let before = engine
        .inner
        .client
        .files_get_snapshot(session, &FileId(id.0.clone()), FileGetOptions::default())
        .await
        .unwrap();
    let upload = engine
        .inner
        .client
        .start_resumable_update_conditional(
            session,
            &FileId(id.0.clone()),
            GoogleFileWrite::default(),
            FileUpdateOptions::default(),
            Some(bytes.len() as u64),
            before.etag.as_deref().unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        engine
            .inner
            .client
            .upload_chunk_conditional(
                session,
                &upload,
                GoogleUploadChunk {
                    start: 0,
                    bytes: bytes.to_vec(),
                    total_bytes: Some(bytes.len() as u64)
                },
                before.etag.as_deref().unwrap()
            )
            .await
            .unwrap(),
        GoogleUploadProgress::Complete { .. }
    ));
}
async fn quiet(inventory: &Inventory, query: &InventoryScopeQuery) {
    let before = rows(inventory, query);
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    assert_eq!(
        before,
        rows(inventory, query),
        "own effects must not produce transfer loops or baseline rewrites"
    );
}
#[tokio::test]
#[ignore = "requires the authorized real test account and explicitly selected isolated roots"]
async fn real_edits_atomic_replacement_permission_return_and_restart_converge() {
    let phase = env("DRIVESYNC_STAGE6_PHASE");
    assert!(matches!(
        phase.as_str(),
        "edits" | "restart" | "verify" | "retry" | "finish"
    ));
    let local = PathBuf::from(env("DRIVESYNC_STAGE6_LOCAL_ROOT"));
    assert!(
        local.is_absolute()
            && local
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("DriveSync-stage6-validation-")
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
    let matching: Vec<_> = available
        .accounts
        .iter()
        .filter(|a| a.email.as_deref() == Some("rtosta.coc@gmail.com"))
        .collect();
    assert_eq!(matching.len(), 1);
    let account = matching[0].id.clone();
    let root = RemoteId(env("DRIVESYNC_STAGE6_REMOTE_ROOT"));
    let acquired = engine
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
            acquired.session.google(),
            &FileId(root.0.clone()),
            FileGetOptions::default(),
        )
        .await
        .unwrap();
    assert!(metadata
        .name
        .unwrap()
        .starts_with("DriveSync-stage6-validation-"));
    if phase == "edits" {
        let listing = engine
            .inner
            .client
            .files_list(
                acquired.session.google(),
                FileListOptions {
                    q: Some(format!("'{}' in parents and trashed = false", root.0)),
                    fields: Some(FieldMask("files(id),nextPageToken,incompleteSearch".into())),
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
        std::fs::create_dir(&local).unwrap();
        std::fs::write(local.join("file.bin"), b"initial1").unwrap();
    }
    let mut hash = Sha256::new();
    for part in [
        account.0.as_bytes(),
        root.0.as_bytes(),
        local.as_os_str().as_bytes(),
    ] {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part);
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
    let before = rows(&inventory, &query);
    engine
        .start_synchronization(StartSynchronizationRequest {
            account_id: account.clone(),
            local_root: local.clone(),
            remote_root_id: root,
        })
        .await
        .unwrap();
    let file = local.join("file.bin");
    if phase == "retry" {
        let id = before[0].identity.remote_id.as_ref().unwrap();
        let current = engine
            .inner
            .client
            .files_get(
                acquired.session.google(),
                &FileId(id.0.clone()),
                FileGetOptions {
                    fields: Some(FieldMask(
                        "id,name,mimeType,parents,size,sha256Checksum,version,trashed".into(),
                    )),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        println!(
            "current remote version {:?}, hash {:?}; baseline version {:?}",
            current.version,
            current.sha256_checksum,
            before[0].baseline.remote.as_ref().unwrap().version
        );

        let current = converged(&inventory, &query, b"atomic03").await;
        engine.stop_synchronization(account).await.unwrap();
        println!(
            "existing atomic edit recovered at revision {}",
            current.revision
        );
        return;
    }
    if phase == "edits" || phase == "finish" {
        let temporary = local.join(".drivesync-transfer-editor");
        let changed = if phase == "edits" {
            let initial = converged(&inventory, &query, b"initial1").await;
            quiet(&inventory, &query).await;
            let mtime = std::fs::metadata(&file).unwrap().modified().unwrap();
            std::fs::write(&file, b"local002").unwrap();
            std::fs::File::open(&file)
                .unwrap()
                .set_modified(mtime)
                .unwrap();
            converged(&inventory, &query, b"local002").await;
            println!("local edit with identical size/mtime converged");
            std::fs::write(&temporary, b"atomic03").unwrap();
            std::fs::rename(&temporary, &file).unwrap();
            let changed = converged(&inventory, &query, b"atomic03").await;
            assert_eq!(changed.identity.item_id, initial.identity.item_id);
            assert_eq!(changed.identity.remote_id, initial.identity.remote_id);
            assert_ne!(
                changed.identity.local_identity,
                initial.identity.local_identity
            );
            quiet(&inventory, &query).await;
            changed
        } else {
            converged(&inventory, &query, b"atomic03").await
        };
        // A replacement with equal bytes needs only confirmed identity refresh, never another upload.
        std::fs::write(&temporary, b"atomic03").unwrap();
        std::fs::rename(&temporary, &file).unwrap();
        let mut refreshed = false;
        for _ in 0..40 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let r = rows(&inventory, &query);
            if r[0].identity.local_identity != changed.identity.local_identity
                && r[0].baseline.state == BaselineState::Synced
            {
                assert_eq!(
                    r[0].pending_operations.len(),
                    changed.pending_operations.len()
                );
                refreshed = true;
                break;
            }
        }
        assert!(
            refreshed,
            "equal-byte atomic replacement must refresh the confirmed identity"
        );
        println!("atomic replacements retained ItemId/RemoteId; equal bytes avoided transfer");
        let remote = changed.identity.remote_id.unwrap();
        remote_edit(&engine, &account, &remote, b"remote04").await;
        converged(&inventory, &query, b"remote04").await;
        assert_eq!(std::fs::read(&file).unwrap(), b"remote04");
        quiet(&inventory, &query).await;
        let _permissions = RestorePermissions(file.clone());
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o0)).unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
        remote_edit(&engine, &account, &remote, b"remote05").await;
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        converged(&inventory, &query, b"remote05").await;
        assert_eq!(std::fs::read(&file).unwrap(), b"remote05");
        quiet(&inventory, &query).await;
        engine.stop_synchronization(account).await.unwrap();
        std::fs::write(&file, b"restart6").unwrap();
        println!("remote edits and permission return converged without another edit; left stopped-scope edit for next process");
        return;
    }
    converged(&inventory, &query, b"restart6").await;
    quiet(&inventory, &query).await;
    engine.stop_synchronization(account).await.unwrap();
    let after = rows(&inventory, &query);
    assert_eq!(after.len(), 1);
    assert_eq!(
        after[0]
            .pending_operations
            .iter()
            .filter(|o| o.state == OperationState::Confirmed)
            .count(),
        6
    );
    assert!(after[0]
        .pending_operations
        .iter()
        .all(|o| matches!(o.state, OperationState::Confirmed | OperationState::Failed)));
    if phase == "verify" {
        assert_eq!(before, after);
    }
    println!("{phase}: one identity, six confirmed material operations, no own-effect loop");
}
