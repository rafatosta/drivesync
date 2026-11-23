//! Explicit real-account interruption between upload blocks, then recovery in another Engine.
use super::super::*;
use super::live_recovery_tests::{
    absent_remote, fixture_request, scope, seed_identity, sync_directory,
};
use super::*;
use crate::{
    accounts::*,
    inventory::{models::*, InventoryConfig},
    Engine, EngineConfig, StartSynchronizationRequest,
};
use std::{path::PathBuf, sync::Arc};
struct NoRemoval;
impl AccountAssociations for NoRemoval {
    fn lock_for_removal(&self, _: &AccountId) -> AccountsResult<Box<dyn RemovalGuard>> {
        Err(AccountsFailure::new(AccountsError::AccountInUse))
    }
}
struct FirstChunk(GoogleOperator);
struct Interrupt<'a>(&'a dyn ExecutionJournal);
impl ExecutionJournal for Interrupt<'_> {
    fn record(&self) -> RecoveryRecord {
        self.0.record()
    }
    fn checkpoint(&self, checkpoint: RecoveryCheckpoint) -> Result<(), OperatorError> {
        self.0.checkpoint(checkpoint.clone())?;
        if matches!(checkpoint,RecoveryCheckpoint::Transfer(t) if t.confirmed_offset==262144) {
            return Err(filesystem::stale().uncertain());
        }
        Ok(())
    }
}
impl RemoteExecutor for FirstChunk {
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
        j: Option<&'a dyn ExecutionJournal>,
    ) -> EffectFuture<'a> {
        Box::pin(async move { self.0.execute(c, r, fs, Some(&Interrupt(j.unwrap()))).await })
    }
}
fn env(name: &str) -> String {
    std::env::var(name)
        .unwrap_or_else(|_| panic!("explicit isolated real fixture required: {name}"))
}
#[tokio::test]
#[ignore = "requires the authorized real test account and an explicitly selected isolated fixture"]
async fn real_partial_upload_recovers_the_same_session_in_a_new_engine() {
    let phase = env("DRIVESYNC_STAGE5_RESUME_PHASE");
    assert!(matches!(
        phase.as_str(),
        "prepare" | "recover" | "verify" | "inspect"
    ));
    let directory = PathBuf::from(env("DRIVESYNC_REAL_ACCOUNTS_DIRECTORY"));
    let oauth = PathBuf::from(env("DRIVESYNC_REAL_OAUTH_FILE"));
    let local = PathBuf::from(env("DRIVESYNC_STAGE5_LOCAL_ROOT"));
    assert!(
        local.is_absolute()
            && local
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("DriveSync-stage5-validation-")
            && local
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with("-resume")
    );
    let root = RemoteId(env("DRIVESYNC_STAGE5_REMOTE_ROOT"));
    let client = GoogleDriveClient::default();
    let mut oauth_config = client
        .load_google_config(GoogleConfigSources {
            oauth_file_env: Some(oauth.to_string_lossy().into_owned()),
            ..Default::default()
        })
        .await
        .unwrap();
    oauth_config.scopes = vec![
        "openid".into(),
        "email".into(),
        "profile".into(),
        "https://www.googleapis.com/auth/drive".into(),
    ];
    let accounts = Accounts::new(
        AccountsConfig::new(
            directory.clone(),
            "com.rtosta.DriveSync.Devel",
            Arc::new(NoRemoval),
        )
        .with_google(client.clone(), oauth_config),
    )
    .unwrap();
    let listed = accounts
        .list(ListAccountsRequest { provider: None })
        .unwrap();
    let matching: Vec<_> = listed
        .accounts
        .iter()
        .filter(|a| a.email.as_deref() == Some("rtosta.coc@gmail.com"))
        .collect();
    assert_eq!(matching.len(), 1);
    let account = matching[0].id.clone();
    let id = scope(&account, &root, &local);
    let acquired = accounts
        .acquire_session(AcquireSessionRequest {
            account_id: account.clone(),
        })
        .await
        .unwrap();
    let remote = client
        .files_get(
            acquired.session.google(),
            &FileId(root.0.clone()),
            FileGetOptions::default(),
        )
        .await
        .unwrap();
    assert!(
        remote
            .name
            .unwrap()
            .starts_with("DriveSync-stage5-validation-")
            && remote.mime_type == Some(MimeType(FOLDER.into()))
    );
    let inventory = Arc::new(
        Inventory::open(InventoryConfig {
            directory: sync_directory(&directory),
            account_id: account.clone(),
        })
        .unwrap(),
    );
    let query = InventoryScopeQuery {
        account_id: account.clone(),
        sync_root_id: id.clone(),
    };
    if phase == "prepare" {
        let children = client
            .files_list(
                acquired.session.google(),
                FileListOptions {
                    q: Some(format!("'{}' in parents and trashed = false", root.0)),
                    fields: Some(FieldMask("files(id),incompleteSearch,nextPageToken".into())),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(
            children.files.is_empty()
                && children.incomplete_search == Some(false)
                && children.next_page_token.is_none()
        );
        let source: Vec<_> = (0..786432).map(|n| (n % 251) as u8).collect();
        if local.exists() {
            let rows = inventory.enumerate(query.clone()).unwrap();
            assert_eq!(rows.len(), 1);
            assert!(rows[0].pending_operations.is_empty());
            assert_eq!(rows[0].baseline.state, BaselineState::NeverSynced);
            assert_eq!(std::fs::read(local.join("resume.bin")).unwrap(), source);
        } else {
            std::fs::create_dir(&local).unwrap();
            std::fs::write(local.join("resume.bin"), source).unwrap();
        }
        let executor = PersistingOperator {
            filesystem: Filesystem::open(&local).unwrap(),
            config: OperatorConfig {
                expected_root_identity: None,
                trash_propagation_authorized: false,
                initial_association_authorized: false,
                account_id: account.clone(),
                provider_id: ProviderId("google".into()),
                sync_root_id: id,
                local_root: local.clone(),
                remote_root_id: root,
            },
            inventory: inventory.clone(),
            remote: Arc::new(FirstChunk(GoogleOperator::new(accounts, client))),
            gate: tokio::sync::Mutex::new(()),
        };
        let mut r = fixture_request(
            &executor,
            "resume.bin",
            OperationAction::Upload,
            absent_remote(),
        );
        let existing = inventory.enumerate(query.clone()).unwrap();
        if existing.is_empty() {
            seed_identity(&executor, &mut r);
        } else {
            r.identity = existing[0].identity.clone();
            r.operation.item_ref = r.identity.item_id.clone().unwrap();
            r.expected_revision = Some(existing[0].revision);
        }
        let result = executor.execute(r).await;
        assert_eq!(
            result.status,
            OperationResultStatus::Unconfirmed,
            "{:?}",
            result.error
        );
        let rows = inventory.enumerate(query).unwrap();
        assert_eq!(rows.len(), 1);
        let record = rows[0].pending_operations[0].recovery.as_ref().unwrap();
        assert_eq!(record.transfer.as_ref().unwrap().confirmed_offset, 262144);
        assert_eq!(rows[0].baseline.state, BaselineState::NeverSynced);
        println!("prepared: first 262144 bytes acknowledged and persisted; session handle retained privately, no baseline");
        return;
    }
    let before = inventory.enumerate(query.clone()).unwrap();
    assert_eq!(before.len(), 1);
    let reserved = before[0].pending_operations[0]
        .recovery
        .as_ref()
        .unwrap()
        .reserved_remote_id
        .clone()
        .unwrap();
    if phase == "inspect" {
        let observed = client
            .files_get(
                acquired.session.google(),
                &FileId(reserved.0.clone()),
                FileGetOptions {
                    fields: Some(FieldMask(FIELDS.into())),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let record = before[0].pending_operations[0].recovery.as_ref().unwrap();
        let fs = Filesystem::open(&local).unwrap();
        let current = fs.check(&record.local).unwrap();
        println!("readback: name {:?}, mime {:?}, size {:?}, hash {:?}, parents {:?}, version {:?}, marker matches {}, equivalent {}", observed.name, observed.mime_type, observed.size, observed.sha256_checksum, observed.parents, observed.version, observed.app_properties.as_ref().and_then(|p| p.get("drivesyncExecution")) == Some(&record.execution_id.0), crate::orchestrator::evidence::equivalent(&current, &normalize(&observed)));
        return;
    }
    drop(accounts);
    let engine = Engine::open(EngineConfig {
        data_directory: directory,
        application_id: "com.rtosta.DriveSync.Devel".into(),
        google_oauth_file: Some(oauth),
    })
    .await
    .unwrap();
    engine
        .start_synchronization(StartSynchronizationRequest {
            account_id: account.clone(),
            local_root: local.clone(),
            remote_root_id: root,
        })
        .await
        .unwrap();
    for _ in 0..60 {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let rows = inventory.enumerate(query.clone()).unwrap();
        if rows[0].baseline.state == BaselineState::Synced {
            break;
        }
    }
    engine.stop_synchronization(account).await.unwrap();
    let rows = inventory.enumerate(query).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].baseline.state, BaselineState::Synced);
    assert_eq!(rows[0].identity.remote_id, Some(reserved.clone()));
    assert_eq!(rows[0].pending_operations.len(), 1);
    assert_eq!(
        rows[0].pending_operations[0].state,
        OperationState::Confirmed
    );
    let old = before[0].pending_operations[0]
        .recovery
        .as_ref()
        .unwrap()
        .transfer
        .as_ref()
        .unwrap();
    let new = rows[0].pending_operations[0]
        .recovery
        .as_ref()
        .unwrap()
        .transfer
        .as_ref()
        .unwrap();
    assert_eq!(old.session_uri, new.session_uri);
    assert_eq!(new.confirmed_offset, 786432);
    let bytes = client
        .files_get(
            acquired.session.google(),
            &FileId(reserved.0.clone()),
            FileGetOptions {
                fields: Some(FieldMask("id,size,sha256Checksum".into())),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let source = std::fs::read(local.join("resume.bin")).unwrap();
    assert_eq!(
        bytes.sha256_checksum,
        Some(format!("{:x}", Sha256::digest(&source)))
    );
    if phase == "verify" {
        assert_eq!(before, rows);
    }
    println!("{phase}: same reserved ID {:?}, same persisted session, 786432 bytes confirmed, one material execution",reserved);
}
