//! Opt-in real Drive effects with injected receipt loss, followed by normal Engine startup recovery.
use super::super::{interfaces::RemoteExecutor, *};
use super::*;
use crate::{
    accounts::{
        AccountAssociations, AccountsConfig, AccountsError, AccountsFailure, AccountsResult,
        ListAccountsRequest, RemovalGuard,
    },
    inventory::{models::*, InventoryConfig},
    Engine, EngineConfig, StartSynchronizationRequest,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, os::unix::ffi::OsStrExt, path::PathBuf, sync::Arc};
struct NoRemoval;
impl AccountAssociations for NoRemoval {
    fn lock_for_removal(&self, _: &AccountId) -> AccountsResult<Box<dyn RemovalGuard>> {
        Err(AccountsFailure::new(AccountsError::AccountInUse))
    }
}
struct LoseReceipt(GoogleOperator);
impl RemoteExecutor for LoseReceipt {
    fn reserve<'a>(
        &'a self,
        c: &'a OperatorConfig,
        r: &'a ExecuteOperationRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Option<RemoteId>, OperatorError>> + Send + 'a>,
    > {
        self.0.reserve(c, r)
    }
    fn execute<'a>(
        &'a self,
        c: &'a OperatorConfig,
        r: &'a ExecuteOperationRequest,
        fs: &'a Filesystem,
        journal: Option<&'a dyn ExecutionJournal>,
    ) -> EffectFuture<'a> {
        Box::pin(async move {
            self.0.execute(c, r, fs, journal).await?;
            Err(filesystem::stale().uncertain())
        })
    }
}
#[derive(Serialize, Deserialize)]
struct Expected {
    account: AccountId,
    scope: SyncRootId,
    items: BTreeMap<String, ItemId>,
    remote_ids: BTreeMap<String, RemoteId>,
}
fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        panic!("{name} must explicitly select an authorized isolated test fixture")
    })
}
fn opaque() -> String {
    use std::io::Read;
    let mut bytes = [0; 16];
    std::fs::File::open("/dev/urandom")
        .unwrap()
        .read_exact(&mut bytes)
        .unwrap();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub(super) fn scope(account: &AccountId, remote: &RemoteId, local: &std::path::Path) -> SyncRootId {
    let mut hash = Sha256::new();
    for bytes in [
        account.0.as_bytes(),
        remote.0.as_bytes(),
        local.as_os_str().as_bytes(),
    ] {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    SyncRootId(format!("{:x}", hash.finalize()))
}
pub(super) fn sync_directory(accounts: &std::path::Path) -> PathBuf {
    let mut name = accounts.file_name().unwrap().to_os_string();
    name.push(".synchronization");
    accounts.with_file_name(name)
}
fn absent(path: &str, directory: bool) -> LocalState {
    LocalState {
        state: FileState::Absent,
        path: Some(path.into()),
        name: Evidence::Known(path.into()),
        parent: Evidence::Known(PathBuf::new()),
        kind: Evidence::Known(if directory {
            FileKind::Directory
        } else {
            FileKind::Regular
        }),
        identity: Evidence::NotCollected,
        size: Evidence::NotCollected,
        modified_at: Evidence::NotCollected,
        changed_at: Evidence::NotCollected,
        checksum: Evidence::NotCollected,
        capabilities: Evidence::NotCollected,
    }
}
pub(super) fn absent_remote() -> RemoteState {
    RemoteState {
        state: FileState::Absent,
        id: None,
        name: Evidence::NotCollected,
        parents: Evidence::NotCollected,
        mime_type: Evidence::NotCollected,
        size: Evidence::NotCollected,
        checksum: Evidence::NotCollected,
        version: Evidence::NotCollected,
        modified_time: Evidence::NotCollected,
        trashed: Evidence::NotCollected,
        capabilities: Evidence::NotCollected,
    }
}
pub(super) fn fixture_request(
    executor: &PersistingOperator,
    path: &str,
    action: OperationAction,
    remote: RemoteState,
) -> ExecuteOperationRequest {
    let local = if executor.config.local_root.join(path).exists() {
        executor
            .filesystem
            .observe(std::path::Path::new(path))
            .unwrap()
    } else {
        absent(path, action == OperationAction::CreateLocalDirectory)
    };
    let id = ItemId(opaque());
    let remote_version = match remote.version {
        Evidence::Known(v) => Some(v),
        _ => None,
    };
    ExecuteOperationRequest {
        operation: Operation {
            conflict_id: None,
            step: 1,
            action,
            item_ref: id.clone(),
            depends_on: Vec::new(),
            remote_id: remote.id.clone(),
            local_path: Some(path.into()),
            name: Some(path.into()),
            parent_id: Some(executor.config.remote_root_id.clone()),
            parent: Some(PathBuf::new()),
            preconditions: Some(OperationPreconditions {
                observed_remote_version: remote_version,
                baseline_remote_version: None,
                local_unchanged: Evidence::Known(true),
                remote_unchanged: Evidence::Known(true),
                destination_free: Evidence::Known(true),
            }),
        },
        execution_id: Id(opaque()),
        expected_revision: Some(0),
        identity: FileIdentity {
            account_id: executor.config.account_id.clone(),
            sync_root_id: executor.config.sync_root_id.clone(),
            item_id: Some(id),
            remote_id: remote.id.clone(),
            local_identity: local.identity.clone(),
        },
        local,
        remote,
        completed_steps: Vec::new(),
    }
}
pub(super) fn seed_identity(executor: &PersistingOperator, r: &mut ExecuteOperationRequest) {
    let receipt = executor.write(
        r,
        Some(0),
        PersistedEffect::ItemRegistered {
            local_path: if r.local.state == FileState::Present {
                r.local.path.clone()
            } else {
                None
            },
        },
    );
    assert_eq!(receipt.status, WriteStatus::Written);
    r.expected_revision = receipt.revision;
}
fn leave_pending(
    executor: &PersistingOperator,
    r: &ExecuteOperationRequest,
    record: RecoveryRecord,
    running: bool,
) {
    let mut pending = PendingOperation {
        id: r.execution_id.clone(),
        item_id: r.operation.item_ref.clone(),
        action: r.operation.action,
        state: OperationState::Pending,
        remote_id: record
            .reserved_remote_id
            .clone()
            .or_else(|| r.operation.remote_id.clone()),
        applied_effect: None,
        idempotency_key: Some(r.execution_id.0.clone()),
        recovery: Some(Box::new(record)),
        recovery_diagnostic: None,
        created_at: now(),
        updated_at: now(),
    };
    let receipt = executor.write(
        r,
        r.expected_revision,
        PersistedEffect::Pending(pending.clone()),
    );
    assert_eq!(receipt.status, WriteStatus::Written);
    if running {
        pending.state = OperationState::Running;
        assert_eq!(
            executor
                .write(r, receipt.revision, PersistedEffect::Pending(pending))
                .status,
            WriteStatus::Written
        );
    }
}
#[tokio::test]
#[ignore = "requires an explicitly authorized test account, isolated real Drive root and Devel Secret Service session"]
async fn real_receipt_loss_and_new_engine_recovery() {
    let phase = env("DRIVESYNC_STAGE3_PHASE");
    assert!(matches!(phase.as_str(), "prepare" | "recover" | "verify"));
    let accounts_directory = PathBuf::from(env("DRIVESYNC_REAL_ACCOUNTS_DIRECTORY"));
    let oauth_file = PathBuf::from(env("DRIVESYNC_REAL_OAUTH_FILE"));
    let local_root = PathBuf::from(env("DRIVESYNC_STAGE3_LOCAL_ROOT"));
    let remote_root = RemoteId(env("DRIVESYNC_STAGE3_REMOTE_ROOT"));
    assert!(
        local_root.is_absolute()
            && local_root
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("DriveSync-stage3-validation-")
    );
    let manifest = local_root.join(".drivesync/stage3-validation.json");
    if phase == "prepare" {
        std::fs::create_dir(&local_root).unwrap();
        let client = GoogleDriveClient::default();
        let mut oauth = client
            .load_google_config(GoogleConfigSources {
                oauth_file_env: Some(oauth_file.to_string_lossy().into_owned()),
                ..Default::default()
            })
            .await
            .unwrap();
        oauth.scopes = vec![
            "openid".into(),
            "email".into(),
            "profile".into(),
            "https://www.googleapis.com/auth/drive".into(),
        ];
        let accounts = Accounts::new(
            AccountsConfig::new(
                accounts_directory.clone(),
                "com.rtosta.DriveSync.Devel",
                Arc::new(NoRemoval),
            )
            .with_google(client.clone(), oauth),
        )
        .unwrap();
        let available = accounts
            .list(ListAccountsRequest { provider: None })
            .unwrap();
        let found: Vec<_> = available
            .accounts
            .iter()
            .filter(|a| a.email.as_deref() == Some("rtosta.coc@gmail.com"))
            .collect();
        assert_eq!(found.len(), 1);
        let account = found[0].id.clone();
        let acquired = accounts
            .acquire_session(AcquireSessionRequest {
                account_id: account.clone(),
            })
            .await
            .unwrap();
        let session = acquired.session.google();
        let root = client
            .files_get(
                session,
                &FileId(remote_root.0.clone()),
                FileGetOptions {
                    fields: Some(FieldMask(FIELDS.into())),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(root
            .name
            .as_ref()
            .unwrap()
            .starts_with("DriveSync-stage3-validation-"));
        assert_eq!(root.mime_type, Some(MimeType(FOLDER.into())));
        let config = OperatorConfig {
            trash_propagation_authorized: false,
            initial_association_authorized: false,
            account_id: account.clone(),
            sync_root_id: scope(&account, &remote_root, &local_root),
            provider_id: ProviderId("google".into()),
            local_root: local_root.clone(),
            remote_root_id: remote_root.clone(),
        };
        let inventory = Arc::new(
            Inventory::open(InventoryConfig {
                directory: sync_directory(&accounts_directory),
                account_id: account.clone(),
            })
            .unwrap(),
        );
        let executor = PersistingOperator {
            filesystem: Filesystem::open(&local_root).unwrap(),
            config: config.clone(),
            inventory: inventory.clone(),
            remote: Arc::new(LoseReceipt(GoogleOperator::new(
                accounts.clone(),
                client.clone(),
            ))),
            gate: tokio::sync::Mutex::new(()),
        };
        std::fs::create_dir(local_root.join("remote-dir")).unwrap();
        std::fs::create_dir(local_root.join("uncertain-dir")).unwrap();
        let bytes: Vec<_> = (0..307200).map(|n| (n % 256) as u8).collect();
        std::fs::write(local_root.join("upload.bin"), &bytes).unwrap();
        std::fs::write(local_root.join("never-dispatched.bin"), b"not dispatched").unwrap();
        let mut expected = Expected {
            account: account.clone(),
            scope: config.sync_root_id.clone(),
            items: BTreeMap::new(),
            remote_ids: BTreeMap::new(),
        };
        for (path, action, source_id) in [
            (
                "local-dir",
                OperationAction::CreateLocalDirectory,
                Some(env("DRIVESYNC_STAGE3_SOURCE_DIRECTORY")),
            ),
            (
                "download.bin",
                OperationAction::Download,
                Some(env("DRIVESYNC_STAGE3_SOURCE_FILE")),
            ),
            ("remote-dir", OperationAction::CreateRemoteDirectory, None),
            ("upload.bin", OperationAction::Upload, None),
        ] {
            let remote = if let Some(id) = source_id {
                normalize(
                    &client
                        .files_get(
                            session,
                            &FileId(id),
                            FileGetOptions {
                                fields: Some(FieldMask(FIELDS.into())),
                                ..Default::default()
                            },
                        )
                        .await
                        .unwrap(),
                )
            } else {
                absent_remote()
            };
            let mut r = fixture_request(&executor, path, action, remote);
            seed_identity(&executor, &mut r);
            expected
                .items
                .insert(path.into(), r.operation.item_ref.clone());
            let result = executor.execute(r).await;
            assert_eq!(result.status, OperationResultStatus::Unconfirmed, "{path}");
            println!("prepared actual effect with injected receipt loss: {path}");
        }
        for (path, action, running) in [
            ("never-dispatched.bin", OperationAction::Upload, false),
            (
                "uncertain-dir",
                OperationAction::CreateRemoteDirectory,
                true,
            ),
        ] {
            let mut r = fixture_request(&executor, path, action, absent_remote());
            seed_identity(&executor, &mut r);
            expected
                .items
                .insert(path.into(), r.operation.item_ref.clone());
            let record = executor.prepare(&r).await.unwrap();
            leave_pending(&executor, &r, record, running);
        }
        let rows = inventory
            .enumerate(InventoryScopeQuery {
                account_id: account,
                sync_root_id: config.sync_root_id,
            })
            .unwrap();
        assert_eq!(rows.len(), 6);
        for row in rows {
            assert_eq!(row.baseline.state, BaselineState::NeverSynced);
            let pending = &row.pending_operations[0];
            if let Some(id) = pending
                .recovery
                .as_ref()
                .and_then(|record| record.reserved_remote_id.clone())
            {
                expected
                    .remote_ids
                    .insert(row.identity.item_id.unwrap().0, id);
            }
        }
        std::fs::create_dir(local_root.join(".drivesync")).unwrap();
        std::fs::write(manifest, serde_json::to_vec(&expected).unwrap()).unwrap();
        println!("prepare complete: six durable commands, four real effects, zero baselines; credentials and transfer handles not printed");
        return;
    }
    let expected: Expected = serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
    let inventory = Inventory::open(InventoryConfig {
        directory: sync_directory(&accounts_directory),
        account_id: expected.account.clone(),
    })
    .unwrap();
    let before = inventory
        .enumerate(InventoryScopeQuery {
            account_id: expected.account.clone(),
            sync_root_id: expected.scope.clone(),
        })
        .unwrap();
    let engine = Engine::open(EngineConfig {
        data_directory: accounts_directory,
        application_id: "com.rtosta.DriveSync.Devel".into(),
        google_oauth_file: Some(oauth_file),
    })
    .await
    .unwrap();
    engine
        .start_synchronization(StartSynchronizationRequest {
            account_id: expected.account.clone(),
            local_root: local_root.clone(),
            remote_root_id: remote_root,
        })
        .await
        .unwrap();
    let mut rows = Vec::new();
    for _ in 0..60 {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        rows = inventory
            .enumerate(InventoryScopeQuery {
                account_id: expected.account.clone(),
                sync_root_id: expected.scope.clone(),
            })
            .unwrap();
        let confirmed = rows
            .iter()
            .filter(|row| {
                row.pending_operations
                    .iter()
                    .any(|p| p.state == OperationState::Confirmed)
            })
            .count();
        if confirmed == 4 {
            break;
        }
    }
    engine
        .stop_synchronization(expected.account.clone())
        .await
        .unwrap();
    assert_eq!(rows.len(), 6);
    for (path, id) in &expected.items {
        let row = rows
            .iter()
            .find(|row| row.identity.item_id.as_ref() == Some(id))
            .unwrap();
        let pending = &row.pending_operations[0];
        if path == "never-dispatched.bin" {
            assert_eq!(pending.state, OperationState::Failed);
            assert_eq!(row.baseline.state, BaselineState::NeverSynced);
        } else if path == "uncertain-dir" {
            assert_eq!(pending.state, OperationState::Running);
            assert_eq!(
                pending.recovery_diagnostic.as_ref().unwrap().outcome,
                ExecutionEffect::Unknown
            );
            assert_eq!(row.baseline.state, BaselineState::NeverSynced);
        } else {
            assert_eq!(pending.state, OperationState::Confirmed);
            assert_eq!(row.baseline.state, BaselineState::Synced);
        }
        if phase == "verify" && path != "uncertain-dir" {
            assert_eq!(
                row,
                before
                    .iter()
                    .find(|old| old.identity.item_id.as_ref() == Some(id))
                    .unwrap()
            );
        }
    }
    let bytes: Vec<_> = (0..307200).map(|n| (n % 256) as u8).collect();
    assert_eq!(std::fs::read(local_root.join("upload.bin")).unwrap(), bytes);
    assert_eq!(
        std::fs::read(local_root.join("download.bin")).unwrap(),
        bytes
    );
    assert_eq!(
        std::fs::read(local_root.join("never-dispatched.bin")).unwrap(),
        b"not dispatched"
    );
    println!("{phase} complete: four confirmed baselines, one not-dispatched failure, one explicitly unknown blocked create; all sources preserved");
}
