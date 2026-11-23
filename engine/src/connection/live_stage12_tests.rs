//! Read-only verification of the single-file event1 fixture after GUI execution.
use super::*;
use crate::StartSynchronizationRequest;
use crate::{
    inventory::{models::*, Inventory, InventoryConfig},
    models::*,
};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
#[tokio::test]
#[ignore = "requires an explicitly selected completed stage12 event1 fixture and authorized Google account"]
async fn real_event1_independent_provider_hash_version_and_baseline() {
    let env = |key: &str| std::env::var(key).expect(key);
    let local = PathBuf::from(env("DRIVESYNC_STAGE12_LOCAL_ROOT"));
    assert!(local
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("DriveSync-stage12-event1-"));
    let accounts = PathBuf::from(env("DRIVESYNC_REAL_ACCOUNTS_DIRECTORY"));
    let engine = Engine::open(EngineConfig {
        data_directory: accounts.clone(),
        application_id: "com.rtosta.DriveSync.Devel".into(),
        google_oauth_file: Some(env("DRIVESYNC_REAL_OAUTH_FILE").into()),
    })
    .await
    .unwrap();
    let rows = engine
        .list_accounts(ListAccountsRequest { provider: None })
        .await
        .unwrap();
    let account = rows
        .accounts
        .into_iter()
        .find(|a| a.email.as_deref() == Some("rtosta.coc@gmail.com"))
        .unwrap()
        .id;
    let session = engine
        .inner
        .accounts
        .acquire_session(AcquireSessionRequest {
            account_id: account.clone(),
        })
        .await
        .unwrap();
    let remote = engine
        .inner
        .client
        .files_get(
            session.session.google(),
            &FileId(env("DRIVESYNC_STAGE12_FILE_ID")),
            FileGetOptions {
                fields: Some(FieldMask(
                    "id,name,parents,size,sha256Checksum,version,trashed,mimeType".into(),
                )),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    if std::env::var("DRIVESYNC_STAGE12_PHASE").as_deref() == Ok("prepare") {
        println!("event1 preflight provider: {remote:?}");
        std::fs::write(
            PathBuf::from(env("DRIVESYNC_STAGE12_EVIDENCE")).join("remote-before.json"),
            serde_json::to_vec_pretty(&remote).unwrap(),
        )
        .unwrap();
        return;
    }
    let bytes = std::fs::read(local.join("Relatório-evento-1.bin")).unwrap();
    let hash = format!("{:x}", Sha256::digest(&bytes));
    assert_eq!(bytes.len(), 16777216);
    assert_eq!(hash, env("DRIVESYNC_STAGE12_EXPECTED_SHA256"));
    assert_eq!(remote.sha256_checksum.as_deref(), Some(hash.as_str()));
    assert_eq!(
        remote.version,
        Some(env("DRIVESYNC_STAGE12_EXPECTED_VERSION").parse().unwrap())
    );
    assert_eq!(remote.name.as_deref(), Some("Relatório-evento-1.bin"));
    assert_eq!(
        remote.parents.as_ref().unwrap(),
        &vec![FileId(env("DRIVESYNC_STAGE12_REMOTE_ROOT"))]
    );
    let request = StartSynchronizationRequest {
        account_id: account.clone(),
        local_root: local.clone(),
        remote_root_id: RemoteId(env("DRIVESYNC_STAGE12_REMOTE_ROOT")),
    };
    let query = InventoryScopeQuery {
        account_id: account.clone(),
        sync_root_id: synchronization::root_id(&request),
    };
    let inventory = Inventory::open(InventoryConfig {
        directory: synchronization::directory(&accounts),
        account_id: account,
    })
    .unwrap();
    let items = inventory.enumerate(query).unwrap();
    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!(item.baseline.state, BaselineState::Synced);
    assert!(item.baseline.confirmed_at.is_some());
    let baseline = item.baseline.remote.as_ref().unwrap();
    assert_eq!(
        baseline.version,
        Evidence::Known(remote.version.as_ref().unwrap().parse::<u64>().unwrap())
    );
    assert_eq!(
        baseline.id,
        Some(RemoteId(env("DRIVESYNC_STAGE12_FILE_ID")))
    );
    assert!(item
        .pending_operations
        .iter()
        .all(|p| p.state == OperationState::Confirmed));
    let expected_checksum = Evidence::Known(Checksum {
        algorithm: "sha256".into(),
        value: hash.clone(),
    });
    assert_eq!(baseline.checksum, expected_checksum);
    let baseline_local = item.baseline.local.as_ref().unwrap();
    assert_eq!(baseline_local.checksum, expected_checksum);
    assert_eq!(baseline_local.size, Evidence::Known(16777216));
    assert_eq!(item.pending_operations.len(), 1);
    assert_eq!(item.pending_operations[0].action, OperationAction::Download);
    let gui = PathBuf::from(env("DRIVESYNC_STAGE12_EVIDENCE")).join("gui-event1.json");
    if gui.exists() {
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(gui).unwrap()).unwrap();
        assert_eq!(
            value["operation_id"].as_str(),
            Some(item.pending_operations[0].id.0.as_str())
        );
    }
    println!("event1 independent provider: {remote:?}; local_sha256={hash}; inventory={item:?}");
    let output = PathBuf::from(env("DRIVESYNC_STAGE12_EVIDENCE"));
    std::fs::write(
        output.join("inventory.json"),
        serde_json::to_vec_pretty(&items).unwrap(),
    )
    .unwrap();
}
