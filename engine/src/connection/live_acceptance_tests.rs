//! Installed-sandbox acceptance through the public Engine, independently checked against fresh Drive reads.
use super::*;
use crate::{
    inventory::{models::*, Inventory, InventoryConfig},
    models::*,
    synchronization::*,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::PathBuf,
};
fn env(k: &str) -> String {
    std::env::var(k).unwrap_or_else(|_| panic!("explicit acceptance fixture required: {k}"))
}
async fn complete(engine: &Engine, account: &AccountId) -> SynchronizationAudit {
    for _ in 0..120 {
        if let Ok(a) = engine.synchronization_audit(account.clone()).await {
            if a.initial_complete {
                return a;
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    panic!(
        "scope did not attest convergence: {:?} {:?}",
        engine
            .synchronization_status(account.clone())
            .await
            .unwrap(),
        engine.synchronization_audit(account.clone()).await
    );
}
#[tokio::test]
#[ignore = "requires the explicitly isolated rtosta.coc@gmail.com acceptance fixture and an installed Devel sandbox"]
async fn installed_scope_public_audit_matches_provider_tree_hashes_and_reopens_without_replay() {
    let phase = env("DRIVESYNC_STAGE11_PHASE");
    assert!(matches!(phase.as_str(), "verify" | "mixed" | "changes"));
    let local = PathBuf::from(env("DRIVESYNC_STAGE11_LOCAL_ROOT"));
    assert!(
        local.is_absolute()
            && local
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("DriveSync-stage11-validation-")
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
    assert_eq!(available.accounts.len(), 1);
    assert_eq!(
        available.accounts[0].email.as_deref(),
        Some("rtosta.coc@gmail.com")
    );
    let account = available.accounts[0].id.clone();
    let remote = RemoteId(env("DRIVESYNC_STAGE11_REMOTE_ROOT"));
    let folder = engine
        .remote_folder(account.clone(), remote.0.clone())
        .await
        .unwrap();
    assert!(folder.name.starts_with("DriveSync-stage11-validation-"));
    let acquired = engine
        .inner
        .accounts
        .acquire_session(AcquireSessionRequest {
            account_id: account.clone(),
        })
        .await
        .unwrap();
    let session = acquired.session.google();
    if phase == "mixed" {
        engine
            .inner
            .client
            .files_create(
                session,
                GoogleFileWrite {
                    name: GooglePatch::Set("Workspace document".into()),
                    mime_type: GooglePatch::Set(MimeType(
                        "application/vnd.google-apps.document".into(),
                    )),
                    parents: GooglePatch::Set(vec![FileId(remote.0.clone())]),
                    ..Default::default()
                },
                FileCreateOptions::default(),
            )
            .await
            .unwrap();
    }
    let request = StartSynchronizationRequest {
        account_id: account.clone(),
        local_root: local.clone(),
        remote_root_id: remote.clone(),
    };
    let mut hash = Sha256::new();
    for b in [
        account.0.as_bytes(),
        remote.0.as_bytes(),
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
    engine.start_synchronization(request.clone()).await.unwrap();
    let initial = complete(&engine, &account).await;
    println!("initial independent audit: {initial:?}");
    if phase == "changes" {
        let original = inventory.enumerate(query.clone()).unwrap();
        let note = original
            .iter()
            .find(|r| r.local_path == Some("root-note.txt".into()))
            .unwrap();
        let inode = std::fs::metadata(local.join("root-note.txt"))
            .unwrap()
            .ino();
        std::fs::write(
            local.join("root-note.txt"),
            b"stage11 installed content update\n",
        )
        .unwrap();
        // Do not accept the previous capture's still-valid horizon as proof of the new edit.
        for _ in 0..90 {
            let rows = inventory.enumerate(query.clone()).unwrap();
            if rows.iter().find(|r|r.identity.item_id==note.identity.item_id).unwrap().baseline.local.as_ref().is_some_and(|l|matches!(&l.checksum,Evidence::Known(c) if c.value==format!("{:x}",Sha256::digest(b"stage11 installed content update\n")))){break;}
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        complete(&engine, &account).await;
        engine
            .inner
            .client
            .files_update(
                session,
                &FileId(note.identity.remote_id.as_ref().unwrap().0.clone()),
                GoogleFileWrite {
                    name: GooglePatch::Set("renamed-note.txt".into()),
                    ..Default::default()
                },
                FileUpdateOptions::default(),
            )
            .await
            .unwrap();
        for _ in 0..60 {
            if local.join("renamed-note.txt").exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        assert_eq!(
            std::fs::metadata(local.join("renamed-note.txt"))
                .unwrap()
                .ino(),
            inode
        );
        std::fs::rename(local.join("nested/large.bin"), local.join("moved.bin")).unwrap();
        for _ in 0..90 {
            let rows = inventory.enumerate(query.clone()).unwrap();
            if rows.iter().any(|r| {
                r.local_path == Some("moved.bin".into())
                    && r.baseline
                        .local
                        .as_ref()
                        .is_some_and(|l| l.name == Evidence::Known("moved.bin".into()))
                    && r.baseline.remote.as_ref().is_some_and(|r| {
                        r.name == Evidence::Known("moved.bin".into())
                            && r.parents == Evidence::Known(vec![remote.clone()])
                    })
            }) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        complete(&engine, &account).await;
    }
    let audit = complete(&engine, &account).await;
    assert_eq!(audit.roots, request);
    assert!(audit.coverage_complete);
    assert_eq!(audit.unconfirmed_operations, 0);
    assert_eq!(audit.unresolved_conflicts, 0);
    let rows = inventory.enumerate(query.clone()).unwrap();
    let mut queue = vec![(remote.clone(), PathBuf::new())];
    let mut provider = BTreeMap::new();
    let mut pages = 0;
    while let Some((parent, path)) = queue.pop() {
        let mut token = None;
        loop {
            let page=engine.inner.client.files_list(session,FileListOptions{q:Some(format!("'{}' in parents and trashed = false",parent.0)),fields:Some(FieldMask("files(id,name,parents,mimeType,size,sha256Checksum),nextPageToken,incompleteSearch".into())),page_token:token,page_size:Some(100),..Default::default()}).await.unwrap();
            pages += 1;
            assert_eq!(page.incomplete_search, Some(false));
            for file in page.files {
                let id = file.id.as_ref().unwrap().0.clone();
                let name = file.name.as_ref().unwrap();
                let mime = &file.mime_type.as_ref().unwrap().0;
                let dest = path.join(crate::projections::local_name(name, mime).unwrap());
                if mime == "application/vnd.google-apps.folder" {
                    queue.push((RemoteId(id.clone()), dest.clone()));
                    assert!(local.join(&dest).is_dir());
                } else if mime == "application/vnd.google-apps.document" {
                    assert_eq!(
                        std::fs::read(local.join(&dest)).unwrap(),
                        crate::projections::link_bytes(&RemoteId(id.clone())),
                    );
                } else {
                    let bytes = std::fs::read(local.join(&dest)).unwrap();
                    assert_eq!(
                        Some(format!("{:x}", Sha256::digest(&bytes))),
                        file.sha256_checksum
                    );
                    assert_eq!(Some(bytes.len().to_string()), file.size);
                }
                assert!(provider.insert(id, dest).is_none());
            }
            token = page.next_page_token;
            if token.is_none() {
                break;
            }
        }
    }
    assert_eq!(rows.len(), provider.len());
    assert_eq!(audit.converged_items, provider.len());
    assert_eq!(
        rows.iter()
            .map(|r| r.identity.item_id.clone())
            .collect::<BTreeSet<_>>()
            .len(),
        rows.len()
    );
    for row in &rows {
        assert_eq!(
            provider.get(&row.identity.remote_id.as_ref().unwrap().0),
            row.local_path.as_ref()
        );
        assert_eq!(row.baseline.state, BaselineState::Synced);
        assert!(row
            .pending_operations
            .iter()
            .all(|p| matches!(p.state, OperationState::Confirmed | OperationState::Failed)));
    }
    if phase == "verify" {
        for after in &rows {
            let prior = before
                .iter()
                .find(|r| r.identity.item_id == after.identity.item_id)
                .expect("restart must not invent an item");
            assert_eq!(prior.identity, after.identity);
            assert_eq!(prior.local_path, after.local_path);
            assert_eq!(
                prior.pending_operations, after.pending_operations,
                "restart must not replay material effects"
            );
            assert_eq!(prior.baseline.projection, after.baseline.projection);
            let mut current_local = after.baseline.local.clone().unwrap();
            let old_local = prior.baseline.local.as_ref().unwrap();
            current_local.modified_at = old_local.modified_at.clone();
            current_local.changed_at = old_local.changed_at.clone();
            assert!(crate::conflicts::local_matches(&current_local, old_local));
            let current_remote = after.baseline.remote.as_ref().unwrap();
            let mut old_remote = prior.baseline.remote.clone().unwrap();
            assert!(
                matches!((&current_remote.version,&old_remote.version),(Evidence::Known(a),Evidence::Known(b)) if a>=b)
            );
            old_remote.version = current_remote.version.clone();
            assert!(
                crate::conflicts::remote_matches(&old_remote, current_remote),
                "only a verified metadata-only baseline refresh may change during restart"
            );
        }
    }
    println!("{phase}: {pages} fresh provider pages, {} unique IDs/paths, byte SHA-256 checks and Inventory baselines agree; {audit:?}",rows.len());
    engine.stop_synchronization(account).await.unwrap();
}

#[tokio::test]
#[ignore = "requires a separate explicitly isolated installed-sandbox trash fixture and real test account"]
async fn installed_default_trash_confirms_recoverable_effect_or_refuses_without_loss() {
    let local = PathBuf::from(env("DRIVESYNC_STAGE11_TRASH_LOCAL_ROOT"));
    assert!(
        local.is_absolute()
            && local
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("DriveSync-stage11-validation-trash-")
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
    assert_eq!(available.accounts.len(), 1);
    assert_eq!(
        available.accounts[0].email.as_deref(),
        Some("rtosta.coc@gmail.com")
    );
    let account = available.accounts[0].id.clone();
    let remote = RemoteId(env("DRIVESYNC_STAGE11_TRASH_REMOTE_ROOT"));
    assert!(engine
        .remote_folder(account.clone(), remote.0.clone())
        .await
        .unwrap()
        .name
        .starts_with("DriveSync-stage11-validation-trash-"));
    std::fs::create_dir(&local).unwrap();
    let bytes = b"stage11 installed default trash must preserve these bytes\n";
    std::fs::write(local.join("retained.bin"), bytes).unwrap();
    let request = StartSynchronizationRequest {
        account_id: account.clone(),
        local_root: local.clone(),
        remote_root_id: remote.clone(),
    };
    engine.start_synchronization(request.clone()).await.unwrap();
    complete(&engine, &account).await;
    let mut hash = Sha256::new();
    for b in [
        account.0.as_bytes(),
        remote.0.as_bytes(),
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
    assert_eq!(before.len(), 1);
    engine.stop_synchronization(account.clone()).await.unwrap();
    let acquired = engine
        .inner
        .accounts
        .acquire_session(AcquireSessionRequest {
            account_id: account.clone(),
        })
        .await
        .unwrap();
    let session = acquired.session.google();
    engine
        .inner
        .client
        .files_update(
            session,
            &FileId(before[0].identity.remote_id.as_ref().unwrap().0.clone()),
            GoogleFileWrite {
                trashed: GooglePatch::Set(true),
                ..Default::default()
            },
            FileUpdateOptions::default(),
        )
        .await
        .unwrap();
    engine
        .start_synchronization_with_trash_propagation(request)
        .await
        .unwrap();
    for _ in 0..60 {
        let rows = inventory.enumerate(query.clone()).unwrap();
        if rows[0].pending_operations.iter().any(|p| {
            p.action == OperationAction::RemoveLocal
                && matches!(p.state, OperationState::Confirmed | OperationState::Failed)
        }) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    engine.stop_synchronization(account).await.unwrap();
    let after = inventory.enumerate(query).unwrap();
    let row = &after[0];
    assert_eq!(row.identity, before[0].identity);
    if row.baseline.local.as_ref().unwrap().state == FileState::Absent {
        let receipt = row
            .pending_operations
            .iter()
            .find(|p| {
                p.action == OperationAction::RemoveLocal && p.state == OperationState::Confirmed
            })
            .unwrap()
            .recovery
            .as_ref()
            .unwrap()
            .local_trash
            .as_ref()
            .unwrap();
        assert_eq!(std::fs::read(&receipt.destination).unwrap(), bytes);
        assert!(!local.join("retained.bin").exists());
        println!("INSTALLED DEFAULT TRASH: verified retained bytes and absent/trashed baseline");
    } else {
        assert_eq!(row.baseline, before[0].baseline);
        assert_eq!(std::fs::read(local.join("retained.bin")).unwrap(), bytes);
        assert!(
            row.pending_operations
                .iter()
                .any(|p| p.action == OperationAction::RemoveLocal
                    && p.state == OperationState::Failed)
        );
        assert!(!row.pending_operations.iter().any(|p| matches!(
            p.state,
            OperationState::Running | OperationState::AppliedUnconfirmed
        )));
        println!("INSTALLED DEFAULT TRASH: safely refused; source and baseline preserved; no permanent-delete fallback");
    }
    println!("installed default trash receipt: {row:#?}");
}
