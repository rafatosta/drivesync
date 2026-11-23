//! Explicit real-account fixture: stop, edit both replicas, persist conflicts and exercise choices.
use super::*;
use crate::{
    conflicts::*,
    inventory::{models::*, Inventory, InventoryConfig},
    models::*,
    synchronization::*,
};
use sha2::{Digest, Sha256};
use std::{os::unix::ffi::OsStrExt, path::PathBuf};
fn env(k: &str) -> String {
    std::env::var(k).expect(k)
}
async fn wait_conflict(engine: &Engine, account: &AccountId) -> ConflictSummary {
    for _ in 0..90 {
        let rows = engine.list_conflicts(account.clone()).await.unwrap();
        if rows.len() == 1 {
            return rows[0].clone();
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    panic!("real conflict not persisted");
}
async fn synced(
    inventory: &Inventory,
    query: &InventoryScopeQuery,
    bytes: &[u8],
) -> InventorySnapshot {
    for _ in 0..90 {
        let rows = inventory.enumerate(query.clone()).unwrap();
        if rows.len()==1 && rows[0].baseline.state==BaselineState::Synced
            && rows[0].baseline.local.as_ref().zip(rows[0].baseline.remote.as_ref()).is_some_and(|(l,r)|l.checksum==r.checksum && matches!(&l.checksum,Evidence::Known(h) if h.value==format!("{:x}",Sha256::digest(bytes))))
            && rows[0].prior_conflict.as_ref().is_none_or(|c|c.resolved){return rows[0].clone();}
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    panic!(
        "choice did not converge: {:?}",
        inventory
            .enumerate(query.clone())
            .unwrap()
            .iter()
            .map(|s| (
                &s.local_path,
                s.revision,
                s.pending_operations
                    .iter()
                    .map(|p| (p.action, p.state, &p.recovery_diagnostic))
                    .collect::<Vec<_>>()
            ))
            .collect::<Vec<_>>()
    );
}
#[tokio::test]
#[ignore = "requires authorized account and explicit isolated stage8 fixture"]
async fn real_conflicts_survive_restart_and_selected_versions_preserve_both_copies() {
    let phase = env("DRIVESYNC_STAGE8_PHASE");
    let local = PathBuf::from(env("DRIVESYNC_STAGE8_LOCAL_ROOT"));
    assert!(
        local.is_absolute()
            && local
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("DriveSync-stage8-validation-")
    );
    let directory = PathBuf::from(env("DRIVESYNC_REAL_ACCOUNTS_DIRECTORY"));
    let engine = Engine::open(EngineConfig {
        data_directory: directory.clone(),
        application_id: "com.rtosta.DriveSync.Devel".into(),
        google_oauth_file: Some(env("DRIVESYNC_REAL_OAUTH_FILE").into()),
    })
    .await
    .unwrap();
    let accounts = engine
        .list_accounts(ListAccountsRequest { provider: None })
        .await
        .unwrap();
    let matching = accounts
        .accounts
        .iter()
        .filter(|a| a.email.as_deref() == Some("rtosta.coc@gmail.com"))
        .collect::<Vec<_>>();
    assert_eq!(matching.len(), 1);
    let account = matching[0].id.clone();
    let root = RemoteId(env("DRIVESYNC_STAGE8_REMOTE_ROOT"));
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
        .starts_with("DriveSync-stage8-validation-"));
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
    let request = StartSynchronizationRequest {
        account_id: account.clone(),
        local_root: local.clone(),
        remote_root_id: root,
    };
    if phase == "prepare" {
        assert!(!local.exists());
        std::fs::create_dir(&local).unwrap();
        std::fs::write(local.join("file.bin"), b"initial8").unwrap();
        engine.start_synchronization(request.clone()).await.unwrap();
        let row = synced(&inventory, &query, b"initial8").await;
        engine.stop_synchronization(account.clone()).await.unwrap();
        std::fs::write(local.join("file.bin"), b"local008").unwrap();
        super::live_continuous_tests::remote_edit(
            &engine,
            &account,
            row.identity.remote_id.as_ref().unwrap(),
            b"remote08",
        )
        .await;
        engine.start_synchronization(request).await.unwrap();
        let c = wait_conflict(&engine, &account).await;
        assert!(c.can_resolve);
        assert!(!c.resolution_pending);
        engine.stop_synchronization(account).await.unwrap();
        println!("durable conflict {} at revision {}", c.id.0, c.revision);
        return;
    }
    engine.start_synchronization(request.clone()).await.unwrap();
    if phase == "verify" {
        let row = synced(&inventory, &query, b"local008").await;
        assert!(row.prior_conflict.as_ref().unwrap().resolved);
        let before = inventory.enumerate(query.clone()).unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(6)).await;
        assert_eq!(before, inventory.enumerate(query).unwrap());
        engine.stop_synchronization(account).await.unwrap();
        return;
    }
    tokio::time::sleep(std::time::Duration::from_secs(6)).await;
    let c = wait_conflict(&engine, &account).await;
    assert!(engine
        .resolve_conflict(ResolveConflictRequest {
            account_id: AccountId("unrelated-account".into()),
            conflict_id: c.id.clone(),
            expected_revision: c.revision,
            choice: ConflictChoice::KeepLocal
        })
        .await
        .is_err());
    assert_eq!(
        engine
            .resolve_conflict(ResolveConflictRequest {
                account_id: account.clone(),
                conflict_id: c.id.clone(),
                expected_revision: c.revision - 1,
                choice: ConflictChoice::KeepLocal
            })
            .await,
        Err(ConflictError::Stale)
    );
    let choice = if phase == "local" {
        ConflictChoice::KeepLocal
    } else {
        assert_eq!(phase, "remote");
        ConflictChoice::KeepRemote
    };
    engine
        .resolve_conflict(ResolveConflictRequest {
            account_id: account.clone(),
            conflict_id: c.id.clone(),
            expected_revision: c.revision,
            choice,
        })
        .await
        .unwrap();
    let bytes = if choice == ConflictChoice::KeepLocal {
        b"local008"
    } else {
        b"remote08"
    };
    synced(&inventory, &query, bytes).await;
    let archive = local.join(".drivesync/conflicts").join(c.id.0);
    assert_eq!(
        std::fs::read(archive.join("local.bin")).unwrap(),
        b"local008"
    );
    assert_eq!(
        std::fs::read(archive.join("remote.bin")).unwrap(),
        b"remote08"
    );
    assert!(engine
        .list_conflicts(account.clone())
        .await
        .unwrap()
        .is_empty());
    engine.stop_synchronization(account).await.unwrap();
    println!("choice converged and both divergent versions remain archived");
}
