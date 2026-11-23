//! Read-only provider and inventory checks for isolated Stage 12 binary fixtures.
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

/// Verify provider bytes and the persisted receipt independently of the installed GUI.
#[tokio::test]
#[ignore = "requires an explicitly isolated event2 fixture and the authorized Google test account"]
async fn real_event2_verify_uploaded_file() {
    let env = |key: &str| std::env::var(key).expect(key);
    let local = PathBuf::from(env("DRIVESYNC_STAGE12_LOCAL_ROOT"));
    let fixture = local.file_name().unwrap().to_str().unwrap();
    assert!(local.is_absolute() && fixture.starts_with("DriveSync-stage12-event2-"));
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
    assert_eq!(rows.accounts.len(), 1);
    let account = rows.accounts.into_iter().next().unwrap();
    assert_eq!(account.email.as_deref(), Some("rtosta.coc@gmail.com"));
    let account = account.id;
    let phase = env("DRIVESYNC_STAGE12_PHASE");
    let remote_root = env("DRIVESYNC_STAGE12_REMOTE_ROOT");
    assert!(remote_root
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte)));
    let folder = engine
        .remote_folder(account.clone(), remote_root.clone())
        .await
        .unwrap();
    assert_eq!(folder.name, fixture);
    let session = engine
        .inner
        .accounts
        .acquire_session(AcquireSessionRequest {
            account_id: account.clone(),
        })
        .await
        .unwrap();
    let session = session.session.google();
    let page = engine
        .inner
        .client
        .files_list(
            session,
            FileListOptions {
                q: Some(format!("'{remote_root}' in parents and trashed = false")),
                fields: Some(FieldMask("files(id),nextPageToken,incompleteSearch".into())),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(page.next_page_token.is_none());
    assert_ne!(page.incomplete_search, Some(true));
    let files = page.files;
    if phase == "prepare" {
        assert!(files.is_empty());
        println!("event2 preflight: empty Drive folder {remote_root}");
        return;
    }
    assert_eq!(phase, "verify");
    assert_eq!(files.len(), 1);
    let id = files[0].id.as_ref().unwrap();
    let file = engine.inner.client.files_get(session, id, FileGetOptions {
        fields: Some(FieldMask("id,name,parents,size,sha256Checksum,version,headRevisionId,trashed,mimeType,appProperties".into())),
        ..Default::default()
    }).await.unwrap();
    assert_eq!(file.id.as_ref(), Some(id));
    assert_eq!(file.trashed, Some(false));
    assert_eq!(
        file.parents.as_deref(),
        Some(&[FileId(remote_root.clone())][..])
    );
    let name = "existing-local.bin";
    assert_eq!(file.name.as_deref(), Some(name));
    let local_bytes = std::fs::read(local.join(name)).unwrap();
    let size = local_bytes.len() as u64;
    let digest = format!("{:x}", Sha256::digest(&local_bytes));
    assert_eq!(digest, env("DRIVESYNC_STAGE12_EXPECTED_SHA256"));
    assert_eq!(file.size.as_ref().unwrap().parse::<u64>().unwrap(), size);
    assert_eq!(file.sha256_checksum.as_deref(), Some(digest.as_str()));
    let revision = file.head_revision_id.as_ref().unwrap();
    let revisions = engine
        .inner
        .client
        .revisions_list(
            session,
            id,
            RevisionListOptions {
                fields: Some(FieldMask("revisions(id,size),nextPageToken".into())),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(revisions.next_page_token.is_none());
    assert_eq!(revisions.revisions.len(), 1);
    assert_eq!(revisions.revisions[0].id.as_ref(), Some(revision));
    assert_eq!(
        revisions.revisions[0]
            .size
            .as_ref()
            .unwrap()
            .parse::<u64>()
            .unwrap(),
        size
    );
    let mut stream = engine
        .inner
        .client
        .get_file_bytes(session, id, None)
        .await
        .unwrap();
    let mut remote_bytes = Vec::new();
    while let Some(chunk) = stream.body.next_chunk().await.unwrap() {
        remote_bytes.extend_from_slice(&chunk);
    }
    assert!(remote_bytes == local_bytes, "Drive media bytes differ");
    let request = StartSynchronizationRequest {
        account_id: account.clone(),
        local_root: local,
        remote_root_id: RemoteId(remote_root),
    };
    let inventory = Inventory::open(InventoryConfig {
        directory: synchronization::directory(&accounts),
        account_id: account.clone(),
    })
    .unwrap();
    let items = inventory
        .enumerate(InventoryScopeQuery {
            account_id: account,
            sync_root_id: synchronization::root_id(&request),
        })
        .unwrap();
    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!(item.baseline.state, BaselineState::Synced);
    assert!(item.baseline.confirmed_at.is_some());
    let remote = item.baseline.remote.as_ref().unwrap();
    let local = item.baseline.local.as_ref().unwrap();
    let checksum = Evidence::Known(Checksum {
        algorithm: "sha256".into(),
        value: digest.clone(),
    });
    let version = file.version.as_ref().unwrap().parse::<u64>().unwrap();
    let remote_id = Some(RemoteId(id.0.clone()));
    assert_eq!(item.identity.remote_id, remote_id);
    assert_eq!(remote.id, remote_id);
    assert_eq!(remote.name, Evidence::Known(name.into()));
    assert_eq!(
        remote.parents,
        Evidence::Known(vec![request.remote_root_id])
    );
    assert_eq!(remote.mime_type, Evidence::Known(file.mime_type.unwrap().0));
    assert_eq!(remote.trashed, Evidence::Known(false));
    let Evidence::Known(baseline_version) = remote.version else {
        panic!("confirmation must retain its observed provider version");
    };
    // Drive's file version includes server-side changes outside the binary revision history.
    assert!(baseline_version > 0 && version >= baseline_version);
    assert_eq!(remote.checksum, checksum);
    assert_eq!(local.checksum, checksum);
    assert_eq!(remote.size, Evidence::Known(size));
    assert_eq!(local.size, Evidence::Known(size));
    let uploads: Vec<_> = item
        .pending_operations
        .iter()
        .filter(|op| op.action == OperationAction::Upload)
        .collect();
    assert_eq!(uploads.len(), 1);
    assert!(item
        .pending_operations
        .iter()
        .all(|op| op.state == OperationState::Confirmed));
    let upload = uploads[0];
    let effect = upload.applied_effect.as_ref().unwrap();
    assert!(effect.verified);
    assert_eq!(effect.remote_id, remote_id);
    assert_eq!(effect.remote.as_ref().unwrap(), remote);
    assert_eq!(
        file.app_properties
            .as_ref()
            .unwrap()
            .get("drivesyncExecution"),
        Some(&upload.id.0)
    );
    println!("event2 verified: remote_id={} revision={} provider_version={version} baseline_version={baseline_version} size={size} sha256={digest} operation_id={}", id.0, revision.0, upload.id.0);
}
