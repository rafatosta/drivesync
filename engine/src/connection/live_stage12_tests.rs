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
    verify_binary_fixture("event2").await;
}

/// Verify exactly one new remote identity and its installed download receipt.
#[tokio::test]
#[ignore = "requires an explicitly isolated event3 fixture and the authorized Google test account"]
async fn real_event3_verify_created_remote_file() {
    verify_binary_fixture("event3").await;
}

/// Verify the installed upload of a file created after initial convergence.
#[tokio::test]
#[ignore = "requires an explicitly isolated event4 fixture and the authorized Google test account"]
async fn real_event4_verify_created_local_file() {
    verify_binary_fixture("event4").await;
}

async fn verify_binary_fixture(event: &str) {
    let created_remote = event == "event3";
    let created_local = event == "event4";
    let env = |key: &str| std::env::var(key).expect(key);
    let local = PathBuf::from(env("DRIVESYNC_STAGE12_LOCAL_ROOT"));
    let fixture = local.file_name().unwrap().to_str().unwrap();
    assert!(local.is_absolute() && fixture.starts_with(&format!("DriveSync-stage12-{event}-")));
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
        if created_local {
            assert_eq!(std::fs::read_dir(&local).unwrap().count(), 0);
        }
        println!("{event} preflight: empty Drive folder {remote_root}");
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
    let name = match event {
        "event2" => "existing-local.bin",
        "event3" => "created-remote.bin",
        "event4" => "created-local.bin",
        _ => unreachable!(),
    };
    if created_remote {
        assert_eq!(id.0, env("DRIVESYNC_STAGE12_FILE_ID"));
    }
    if created_local || created_remote {
        assert_eq!(std::fs::read_dir(&local).unwrap().count(), 1);
    }
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
    if created_remote {
        assert_eq!(remote.version, Evidence::Known(version));
        assert_eq!(local.path.as_ref(), Some(&PathBuf::from(name)));
        assert_eq!(local.parent, Evidence::Known(PathBuf::new()));
        assert_eq!(item.pending_operations.len(), 1);
        let download = &item.pending_operations[0];
        assert_eq!(download.action, OperationAction::Download);
        assert_eq!(download.state, OperationState::Confirmed);
        assert_eq!(download.id.0, env("DRIVESYNC_STAGE12_OPERATION_ID"));
        let effect = download.applied_effect.as_ref().unwrap();
        assert!(effect.verified);
        assert_eq!(effect.remote_id, remote_id);
        assert_eq!(effect.remote.as_ref(), Some(remote));
        assert_eq!(effect.local.as_ref(), Some(local));
        println!("{event} verified: remote_id={} parent={:?} item_id={:?} version={version} size={size} sha256={digest} operation_id={}", id.0, remote.parents, item.identity.item_id, download.id.0);
        return;
    }
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
    if created_local {
        use std::os::unix::fs::MetadataExt;

        let metadata = std::fs::metadata(request.local_root.join(name)).unwrap();
        let Evidence::Known(identity) = &local.identity else {
            panic!("confirmed baseline must retain the observed local identity");
        };
        assert_eq!(identity.device_id, metadata.dev());
        assert_eq!(identity.inode, metadata.ino());
        assert_eq!(item.identity.local_identity, local.identity);
        assert_eq!(effect.local.as_ref(), Some(local));
        assert_eq!(local.path.as_ref(), Some(&PathBuf::from(name)));
        assert_eq!(local.parent, Evidence::Known(PathBuf::new()));
        assert_eq!(item.pending_operations.len(), 1);
        assert_eq!(upload.id.0, env("DRIVESYNC_STAGE12_OPERATION_ID"));
        let creation = upload.recovery.as_ref().unwrap();
        assert_eq!(creation.command.action, OperationAction::Upload);
        assert!(creation.command.remote_id.is_none());
        assert_eq!(creation.reserved_remote_id, remote_id);
        assert_eq!(
            creation.identity,
            crate::models::FileIdentity {
                remote_id: None,
                ..item.identity.clone()
            }
        );
        assert_eq!(creation.local.identity, local.identity);
        assert_eq!(creation.local.checksum, checksum);
        assert_eq!(creation.local.path, local.path);
        assert_eq!(creation.expected_revision, Some(1));
        assert_eq!(creation.command.depends_on.len(), 1);
        assert_eq!(creation.completed_steps, creation.command.depends_on);
        assert_eq!(
            item.identity.item_id.as_ref().unwrap().0,
            env("DRIVESYNC_STAGE12_ITEM_ID")
        );
        assert_eq!(id.0, env("DRIVESYNC_STAGE12_FILE_ID"));
        println!(
            "{event} identity: item_id={:?} local={identity:?} parent={:?}",
            item.identity.item_id, remote.parents
        );
    }
    println!("{event} verified: remote_id={} revision={} provider_version={version} baseline_version={baseline_version} size={size} sha256={digest} operation_id={}", id.0, revision.0, upload.id.0);
}
