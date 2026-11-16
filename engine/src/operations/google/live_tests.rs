//! Explicit opt-in acceptance on newly created resources in the declared test
//! account. Never touches a preexisting remote file or permanently removes data.
use super::*;
use crate::inventory::*;
use std::{
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
fn required(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("missing {key}"))
}
fn context(id: &str, item: &ItemResult) -> ExecutionContext {
    ExecutionContext {
        id: ExecutionId::new(id.into()).unwrap(),
        source: item.metadata.clone().map(|m| match m {
            FileData::Local(metadata) => SourceEvidence::Local {
                metadata: Box::new(metadata),
                sha256: None,
            },
            FileData::Remote(metadata) => SourceEvidence::Remote {
                metadata: Box::new(metadata),
                revision: None,
            },
        }),
    }
}
fn empty(id: &str) -> ExecutionContext {
    ExecutionContext {
        id: ExecutionId::new(id.into()).unwrap(),
        source: None,
    }
}
fn directory(item: &ItemResult) -> DirectoryRef {
    match &item.resource {
        ResourceRef::Local {
            relative_path,
            local_id,
        } => DirectoryRef::Local {
            relative_path: relative_path.clone(),
            local_id: local_id.clone(),
        },
        ResourceRef::Remote {
            provider_id,
            reference,
        } => DirectoryRef::Remote {
            provider_id: provider_id.clone(),
            reference: reference.clone(),
        },
    }
}
fn model(account: &AccountId, f: &g::GoogleFile) -> RemoteFile {
    let value = |v: Option<String>| {
        v.map(MetadataValue::Known)
            .unwrap_or(MetadataValue::NotCollected)
    };
    RemoteFile {
        reference: RemoteFileRef {
            account_id: account.clone(),
            file_id: RemoteFileId(f.id.as_ref().unwrap().0.clone()),
        },
        provider: ProviderId("google".into()),
        name: value(f.name.clone()),
        kind: MetadataValue::Known(if f.mime_type.as_ref().is_some_and(|m| m.0 == FOLDER) {
            RemoteEntryKind::Directory
        } else {
            RemoteEntryKind::File
        }),
        mime_type: value(f.mime_type.as_ref().map(|m| m.0.clone())),
        parent_ids: f
            .parents
            .as_ref()
            .map(|p| MetadataValue::Known(p.iter().map(|id| RemoteFileId(id.0.clone())).collect()))
            .unwrap_or(MetadataValue::NotCollected),
        trashed: f
            .trashed
            .map(MetadataValue::Known)
            .unwrap_or(MetadataValue::NotCollected),
        version: value(f.version.clone()),
        size: f
            .size
            .as_ref()
            .map(|s| MetadataValue::Known(s.parse().unwrap()))
            .unwrap_or(MetadataValue::NotCollected),
        created_time: value(f.created_time.clone()),
        modified_time: value(f.modified_time.clone()),
        md5_checksum: value(f.md5_checksum.clone()),
        sha256_checksum: value(f.sha256_checksum.clone()),
        provider_data: MetadataValue::NotCollected,
    }
}
async fn fresh(access: &OperatorAccess, item: &ItemResult) -> ItemResult {
    let ResourceRef::Remote { reference, .. } = &item.resource else {
        return item.clone();
    };
    let mut previous = None;
    for _ in 0..12 {
        let session = access.session().await.unwrap();
        let file = access
            .client
            .files_get(
                &session,
                &g::FileId(reference.file_id.0.clone()),
                g::FileGetOptions {
                    fields: Some(g::FieldMask(FIELDS.into())),
                    supports_all_drives: Some(true),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        if previous.as_ref() == Some(&file.version) {
            return ItemResult {
                resource: item.resource.clone(),
                metadata: Some(FileData::Remote(model(&reference.account_id, &file))),
                verification: VerificationStatus::Verified,
            };
        }
        previous = Some(file.version);
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    panic!("test resource version did not settle")
}
#[tokio::test]
#[ignore = "requires explicit test-account opt-in and a private temporary root"]
async fn live_complete_operator_on_new_resources() {
    assert_eq!(required("DRIVESYNC_OPERATOR_LIVE"), "1");
    let root = PathBuf::from(required("DRIVESYNC_OPERATOR_ROOT"));
    assert!(root
        .to_string_lossy()
        .starts_with("/tmp/drivesync-operator-live-"));
    let account = AccountId(required("DRIVESYNC_LIVE_ACCOUNT"));
    let engine = Engine::open(crate::EngineConfig {
        data_directory: PathBuf::from(required("DRIVESYNC_LIVE_ACCOUNTS")),
        application_id: required("DRIVESYNC_LIVE_APPLICATION"),
        google_oauth_file: std::env::var_os("DRIVESYNC_GOOGLE_OAUTH_FILE").map(PathBuf::from),
    })
    .await
    .unwrap();
    let remote_root = required("DRIVESYNC_LIVE_REMOTE_ROOT");
    std::fs::create_dir(&root).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    let sync = SyncId(format!(
        "operator-live-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let inventory = Arc::new(
        InventoryManager::open(OpenInventoryRequest {
            local_root: root.clone(),
            expected: SyncIdentity {
                sync_id: sync.clone(),
                account_id: account.clone(),
                remote_root_id: remote_root.clone(),
            },
            create_if_missing: true,
        })
        .unwrap()
        .inventory,
    );
    let operator = DefaultOperatorFactory
        .create(OperatorConfig {
            connector: engine.clone(),
            scope: OperatorScope {
                sync_id: sync,
                account_id: account.clone(),
                local_root: root.clone(),
                remote_root_id: RemoteFileId(remote_root.clone()),
            },
            journal: inventory,
        })
        .unwrap();
    let local = operator
        .create_file(CreateFileRequest {
            execution: empty("local-create"),
            parent: DirectoryRef::LocalRoot,
            name: ItemName::Local("created.bin".into()),
            content: Some(ContentSource::Bytes {
                data: b"initial".to_vec(),
            }),
            options: ExecutionOptions::default(),
        })
        .await
        .unwrap();
    let renamed = operator
        .rename(RenameRequest {
            execution: context("local-rename", &local),
            target: local.resource,
            new_name: ItemName::Local("renamed.bin".into()),
            preconditions: Preconditions::default(),
            options: ExecutionOptions::default(),
        })
        .await
        .unwrap();
    let Some(FileData::Local(metadata)) = &renamed.metadata else {
        panic!()
    };
    let updated = operator
        .update_content(UpdateContentRequest {
            execution: empty("local-update"),
            target: renamed.resource,
            source: ContentSource::Bytes {
                data: b"updated".to_vec(),
            },
            preconditions: Preconditions {
                expected_revision: Some(local_revision(metadata).unwrap()),
                expected_version: None,
            },
            options: ExecutionOptions::default(),
        })
        .await
        .unwrap()
        .item;
    let folder = operator
        .create_directory(CreateDirectoryRequest {
            execution: empty("local-folder"),
            parent: DirectoryRef::LocalRoot,
            name: ItemName::Local("destination".into()),
            options: ExecutionOptions::default(),
        })
        .await
        .unwrap();
    let moved = operator
        .move_item(MoveRequest {
            execution: context("local-move", &updated),
            target: updated.resource,
            destination: directory(&folder),
            preconditions: Preconditions::default(),
            options: ExecutionOptions::default(),
        })
        .await
        .unwrap();
    let trash = operator
        .trash(TrashRequest {
            execution: context("local-trash", &moved),
            target: moved.resource,
            preconditions: Preconditions::default(),
            options: ExecutionOptions::default(),
        })
        .await
        .unwrap();
    operator
        .restore(RestoreRequest {
            execution: context("local-restore", &trash),
            target: trash.resource,
            destination: None,
            preconditions: Preconditions::default(),
            options: ExecutionOptions::default(),
        })
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(root.join("destination/renamed.bin")).unwrap(),
        b"updated"
    );
    eprintln!("ACCEPTANCE local create/update/rename/move/trash/restore verified");
    let remote_parent = DirectoryRef::Remote {
        provider_id: ProviderId("google".into()),
        reference: RemoteFileRef {
            account_id: account.clone(),
            file_id: RemoteFileId(remote_root),
        },
    };
    let folder = operator
        .create_directory(CreateDirectoryRequest {
            execution: empty("remote-folder"),
            parent: remote_parent,
            name: ItemName::Remote(format!(
                "DriveSync Operator acceptance {}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            )),
            options: ExecutionOptions::default(),
        })
        .await
        .unwrap();
    let created = operator
        .create_file(CreateFileRequest {
            execution: empty("remote-create"),
            parent: directory(&folder),
            name: ItemName::Remote("created.bin".into()),
            content: Some(ContentSource::Bytes {
                data: b"initial".to_vec(),
            }),
            options: ExecutionOptions::default(),
        })
        .await
        .unwrap();
    let access = engine.operator_access(account.clone()).await.unwrap();
    let created = fresh(&access, &created).await;
    let ResourceRef::Remote { reference, .. } = &created.resource else {
        panic!()
    };
    let session = access.session().await.unwrap();
    let snapshot = access
        .client
        .files_get_snapshot(
            &session,
            &g::FileId(reference.file_id.0.clone()),
            g::FileGetOptions {
                supports_all_drives: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(
        snapshot.etag.is_some(),
        "endpoint did not supply a strong ETag; conditional mutations must remain blocked"
    );
    let error = access
        .client
        .files_update_conditional(
            &session,
            &g::FileId(reference.file_id.0.clone()),
            g::GoogleFileWrite {
                name: g::GooglePatch::Set("created.bin".into()),
                ..Default::default()
            },
            g::FileUpdateOptions {
                supports_all_drives: Some(true),
                ..Default::default()
            },
            "\"drivesync-intentionally-stale\"",
        )
        .await
        .expect_err("server ignored a stale If-Match; unsafe endpoint");
    assert_eq!(error.kind, g::GoogleApiError::PreconditionFailed);
    eprintln!("ACCEPTANCE Google rejected stale If-Match with HTTP 412");
    let renamed = operator
        .rename(RenameRequest {
            execution: context("remote-rename", &created),
            target: created.resource,
            new_name: ItemName::Remote("renamed.bin".into()),
            preconditions: Preconditions::default(),
            options: ExecutionOptions::default(),
        })
        .await
        .unwrap();
    let renamed = fresh(&access, &renamed).await;
    let Some(FileData::Remote(metadata)) = &renamed.metadata else {
        panic!()
    };
    let MetadataValue::Known(version) = &metadata.version else {
        panic!()
    };
    let updated = operator
        .update_content(UpdateContentRequest {
            execution: empty("remote-update"),
            target: renamed.resource,
            source: ContentSource::Bytes {
                data: b"updated".to_vec(),
            },
            preconditions: Preconditions {
                expected_version: Some(version.clone()),
                expected_revision: None,
            },
            options: ExecutionOptions::default(),
        })
        .await
        .unwrap()
        .item;
    let destination = operator
        .create_directory(CreateDirectoryRequest {
            execution: empty("remote-destination"),
            parent: directory(&folder),
            name: ItemName::Remote("destination".into()),
            options: ExecutionOptions::default(),
        })
        .await
        .unwrap();
    let updated = fresh(&access, &updated).await;
    let moved = operator
        .move_item(MoveRequest {
            execution: context("remote-move", &updated),
            target: updated.resource,
            destination: directory(&destination),
            preconditions: Preconditions::default(),
            options: ExecutionOptions::default(),
        })
        .await
        .unwrap();
    let moved = fresh(&access, &moved).await;
    let trash = operator
        .trash(TrashRequest {
            execution: context("remote-trash", &moved),
            target: moved.resource,
            preconditions: Preconditions::default(),
            options: ExecutionOptions::default(),
        })
        .await
        .unwrap();
    let trash = fresh(&access, &trash).await;
    let restored = operator
        .restore(RestoreRequest {
            execution: context("remote-restore", &trash),
            target: trash.resource,
            destination: None,
            preconditions: Preconditions::default(),
            options: ExecutionOptions::default(),
        })
        .await
        .unwrap();
    let restored = fresh(&access, &restored).await;
    let Some(SourceEvidence::Remote { metadata, revision }) =
        context("download-context", &restored).source
    else {
        panic!()
    };
    let downloaded = operator
        .download(DownloadRequest {
            execution: ExecutionContext {
                id: ExecutionId::new("download".into()).unwrap(),
                source: Some(SourceEvidence::Remote { metadata, revision }),
            },
            source: restored.resource,
            destination: DirectoryRef::LocalRoot,
            name: ItemName::Local("downloaded.bin".into()),
            options: ExecutionOptions::default(),
        })
        .await
        .unwrap();
    assert_eq!(downloaded.integrity, IntegrityStatus::Verified);
    assert_eq!(
        std::fs::read(root.join("downloaded.bin")).unwrap(),
        b"updated"
    );
    let folder = fresh(&access, &folder).await;
    operator
        .trash(TrashRequest {
            execution: context("test-folder-cleanup", &folder),
            target: folder.resource,
            preconditions: Preconditions::default(),
            options: ExecutionOptions::default(),
        })
        .await
        .unwrap();
    eprintln!("ACCEPTANCE remote create/update/rename/move/trash/restore/download verified; test folder sent to trash");
}
