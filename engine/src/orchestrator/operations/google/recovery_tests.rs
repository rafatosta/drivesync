//! Crash-window reconciliation uses durable Inventory state and current loopback/temporary-FS facts.
use super::super::{
    interfaces::RemoteExecutor,
    tests::{request, setup, Sandbox},
    *,
};
use super::tests::{endpoint, metadata, operator, root, Endpoint};
use super::*;
use crate::inventory::{models::*, InventoryConfig};
use std::{io::Write, sync::Arc};

struct ScriptBackend {
    google: GoogleOperator,
    lose_receipt: bool,
}
impl RemoteExecutor for ScriptBackend {
    fn reserve<'a>(
        &'a self,
        _: &'a OperatorConfig,
        r: &'a ExecuteOperationRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Option<RemoteId>, OperatorError>> + Send + 'a>,
    > {
        Box::pin(async move {
            Ok(matches!(
                r.operation.action,
                OperationAction::Upload | OperationAction::CreateRemoteDirectory
            )
            .then(|| RemoteId("created".into())))
        })
    }
    fn execute<'a>(
        &'a self,
        c: &'a OperatorConfig,
        r: &'a ExecuteOperationRequest,
        fs: &'a Filesystem,
        journal: Option<&'a dyn ExecutionJournal>,
    ) -> EffectFuture<'a> {
        Box::pin(async move {
            let effect = self
                .google
                .run_with_journal(c, r, fs, &GoogleSession::new("test", "synthetic"), journal)
                .await?;
            if self.lose_receipt {
                Err(filesystem::stale().uncertain())
            } else {
                Ok(effect)
            }
        })
    }
    fn reconcile<'a>(
        &'a self,
        c: &'a OperatorConfig,
        r: &'a RecoveryRecord,
        fs: &'a Filesystem,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = RecoveryAssessment> + Send + 'a>> {
        Box::pin(async move {
            self.google
                .probe_recovery(c, r, fs, &GoogleSession::new("test", "synthetic"))
                .await
        })
    }
}
fn directory_metadata() -> String {
    serde_json::json!({"id":"remote","name":"file","parents":["remote-root"],"mimeType":FOLDER,"version":"1","trashed":false}).to_string()
}
async fn source_server() -> Endpoint {
    endpoint(|_| (200, directory_metadata(), vec![])).await
}
pub(super) fn use_backend(
    s: &Sandbox,
    executor: &mut PersistingOperator,
    client: GoogleDriveClient,
    lose_receipt: bool,
) {
    executor.remote = Arc::new(ScriptBackend {
        google: operator(s, client),
        lose_receipt,
    });
}
pub(super) fn pending_record(
    executor: &PersistingOperator,
    r: &ExecuteOperationRequest,
    record: RecoveryRecord,
    state: OperationState,
) -> u64 {
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
    if state == OperationState::Pending {
        return receipt.revision.unwrap();
    }
    pending.state = OperationState::Running;
    let receipt = executor.write(r, receipt.revision, PersistedEffect::Pending(pending));
    assert_eq!(receipt.status, WriteStatus::Written);
    receipt.revision.unwrap()
}
fn snapshots(executor: &PersistingOperator) -> Vec<InventorySnapshot> {
    executor
        .inventory
        .enumerate(InventoryScopeQuery {
            account_id: executor.config.account_id.clone(),
            sync_root_id: executor.config.sync_root_id.clone(),
        })
        .unwrap()
}
pub(super) fn reopen(s: &Sandbox, old: PersistingOperator) -> PersistingOperator {
    PersistingOperator {
        filesystem: Filesystem::open(&old.config.local_root).unwrap(),
        config: old.config.clone(),
        inventory: Arc::new(
            Inventory::open(InventoryConfig {
                directory: s.0.join("inventory"),
                account_id: old.config.account_id.clone(),
            })
            .unwrap(),
        ),
        remote: old.remote.clone(),
        gate: tokio::sync::Mutex::new(()),
    }
}
#[tokio::test]
async fn pending_is_not_dispatched_and_running_without_publication_checkpoint_is_rechecked_not_replayed(
) {
    for state in [OperationState::Pending, OperationState::Running] {
        let s = Sandbox::new();
        let mut executor = setup(&s);
        let server = source_server().await;
        use_backend(&s, &mut executor, server.client.clone(), false);
        let r = request(&executor, OperationAction::CreateLocalDirectory);
        let record = executor.prepare(&r).await.unwrap();
        pending_record(&executor, &r, record, state);
        let executor = reopen(&s, executor);
        let results = executor.recover().await.unwrap();
        assert_eq!(results[0].assessment.outcome, ExecutionEffect::NotApplied);
        assert_eq!(
            snapshots(&executor)[0].pending_operations[0].state,
            OperationState::Failed
        );
        assert!(!executor.config.local_root.join("file").exists());
        assert!(server
            .requests
            .lock()
            .unwrap()
            .iter()
            .all(|r| r.starts_with("GET ")));
        if state == OperationState::Pending {
            assert!(server.requests.lock().unwrap().is_empty());
        }
    }
}
#[tokio::test]
async fn local_effect_after_receipt_loss_is_attributed_and_confirmed_after_reopening() {
    let s = Sandbox::new();
    let mut executor = setup(&s);
    let server = source_server().await;
    use_backend(&s, &mut executor, server.client.clone(), true);
    let r = request(&executor, OperationAction::CreateLocalDirectory);
    assert_eq!(
        executor.execute(r).await.status,
        OperationResultStatus::Unconfirmed
    );
    let before = snapshots(&executor);
    assert_eq!(before[0].baseline.state, BaselineState::NeverSynced);
    assert!(before[0].pending_operations[0]
        .recovery
        .as_ref()
        .unwrap()
        .stage_identity
        .is_some());
    let executor = reopen(&s, executor);
    let results = executor.recover().await.unwrap();
    assert_eq!(results[0].assessment.outcome, ExecutionEffect::Applied);
    let confirmed = snapshots(&executor);
    assert_eq!(confirmed[0].baseline.state, BaselineState::Synced);
    assert_eq!(
        confirmed[0].pending_operations[0].state,
        OperationState::Confirmed
    );
    let revision = confirmed[0].revision;
    assert!(executor.recover().await.unwrap().is_empty());
    assert_eq!(snapshots(&executor)[0].revision, revision);
}
#[tokio::test]
async fn effect_before_receipt_and_receipt_before_baseline_are_both_revalidated() {
    for save_receipt in [false, true] {
        let s = Sandbox::new();
        let mut executor = setup(&s);
        let server = source_server().await;
        use_backend(&s, &mut executor, server.client.clone(), false);
        let r = request(&executor, OperationAction::CreateLocalDirectory);
        let record = executor.prepare(&r).await.unwrap();
        let revision = pending_record(&executor, &r, record.clone(), OperationState::Running);
        let journal = Journal::new(executor.inventory.clone(), r.clone(), revision, record);
        let mut effect = executor.filesystem.execute(&r, Some(&journal)).unwrap();
        effect.remote = Some(r.remote.clone());
        if save_receipt {
            assert_eq!(
                executor
                    .write(
                        &r,
                        journal.revision(),
                        PersistedEffect::OperationApplied(effect)
                    )
                    .status,
                WriteStatus::Written
            );
        }
        let executor = reopen(&s, executor);
        let result = executor.recover().await.unwrap();
        assert_eq!(result[0].assessment.outcome, ExecutionEffect::Applied);
        assert_eq!(
            snapshots(&executor)[0].baseline.state,
            BaselineState::Synced
        );
    }
}
#[tokio::test]
async fn a_matching_path_without_the_execution_marker_is_not_an_applied_effect() {
    let s = Sandbox::new();
    let mut executor = setup(&s);
    let server = source_server().await;
    use_backend(&s, &mut executor, server.client.clone(), true);
    assert_eq!(
        executor
            .execute(request(&executor, OperationAction::CreateLocalDirectory))
            .await
            .status,
        OperationResultStatus::Unconfirmed
    );
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(
        executor
            .config
            .local_root
            .join("file")
            .as_os_str()
            .as_bytes(),
    )
    .unwrap();
    assert_eq!(
        unsafe { libc::removexattr(path.as_ptr(), c"user.drivesync.execution".as_ptr()) },
        0
    );
    let result = executor.recover().await.unwrap();
    assert_eq!(result[0].assessment.outcome, ExecutionEffect::Unknown);
    assert_eq!(
        snapshots(&executor)[0].baseline.state,
        BaselineState::NeverSynced
    );
    assert!(executor.config.local_root.join("file").is_dir());
}
#[tokio::test]
async fn changed_root_or_scope_never_queries_a_foreign_effect() {
    for changed_scope in [false, true] {
        let s = Sandbox::new();
        let mut executor = setup(&s);
        let server = source_server().await;
        use_backend(&s, &mut executor, server.client.clone(), false);
        let r = request(&executor, OperationAction::CreateLocalDirectory);
        let mut record = executor.prepare(&r).await.unwrap();
        if changed_scope {
            record.remote_root_id = RemoteId("foreign-root".into());
        }
        pending_record(&executor, &r, record, OperationState::Running);
        if !changed_scope {
            std::fs::rename(&executor.config.local_root, s.0.join("preserved-root")).unwrap();
            std::fs::create_dir(&executor.config.local_root).unwrap();
        }
        let executor = reopen(&s, executor);
        let result = executor.recover().await.unwrap();
        assert_eq!(result[0].assessment.outcome, ExecutionEffect::Unknown);
        assert!(server.requests.lock().unwrap().is_empty());
        assert_eq!(
            snapshots(&executor)[0].baseline.state,
            BaselineState::NeverSynced
        );
    }
}
#[tokio::test]
async fn staged_bytes_and_transfer_references_survive_unknown_recovery() {
    let s = Sandbox::new();
    let mut executor = setup(&s);
    std::fs::write(executor.config.local_root.join("file"), vec![3u8; 300000]).unwrap();
    let server = endpoint(|r| {
        if r.starts_with("PUT ") {
            (
                308,
                String::new(),
                vec![("Range".into(), "bytes=0-262143".into())],
            )
        } else {
            (
                404,
                r#"{"error":{"code":404,"message":"missing"}}"#.into(),
                vec![],
            )
        }
    })
    .await;
    use_backend(&s, &mut executor, server.client.clone(), false);
    let mut r = request(&executor, OperationAction::Upload);
    r.operation.parent_id = Some(RemoteId("remote-root".into()));
    r.operation.remote_id = None;
    r.identity.remote_id = None;
    r.remote.id = None;
    r.remote.state = FileState::Absent;
    let mut record = executor.prepare(&r).await.unwrap();
    record.transfer = Some(TransferReference {
        session_uri: format!(
            "{}/upload/drive/v3/files?upload_id=sensitive-handle",
            server.base
        ),
        total_bytes: 300000,
        confirmed_offset: 0,
    });
    assert!(!format!("{record:?}").contains("sensitive-handle"));
    pending_record(&executor, &r, record.clone(), OperationState::Running);
    let orphan = executor
        .config
        .local_root
        .join(".drivesync-transfer-preserved");
    std::fs::write(&orphan, b"uncertain stage").unwrap();
    let result = executor.recover().await.unwrap();
    assert_eq!(result[0].assessment.outcome, ExecutionEffect::Unknown);
    assert_eq!(
        result[0].assessment.issue,
        Some(RecoveryIssue::TransferIncomplete)
    );
    let current = snapshots(&executor);
    assert_eq!(
        current[0].pending_operations[0]
            .recovery
            .as_ref()
            .unwrap()
            .transfer,
        record.transfer
    );
    assert_eq!(std::fs::read(orphan).unwrap(), b"uncertain stage");
    assert_eq!(
        std::fs::read(executor.config.local_root.join("file"))
            .unwrap()
            .len(),
        300000
    );
    let calls = server.requests.lock().unwrap();
    assert_eq!(calls.iter().filter(|r| r.starts_with("PUT ")).count(), 1);
    assert!(calls.iter().all(|r| !r.starts_with("POST ")));
    assert!(calls.last().unwrap().contains("content-length: 0"));
}
#[tokio::test]
async fn reserved_remote_identity_and_marker_locate_a_creation_without_its_receipt() {
    let s = Sandbox::new();
    let mut executor = setup(&s);
    std::fs::create_dir(executor.config.local_root.join("file")).unwrap();
    let server=endpoint(|_|(200,serde_json::json!({"id":"created","name":"file","parents":["remote-root"],"mimeType":FOLDER,"version":"1","trashed":false,"appProperties":{"drivesyncExecution":"execution"}}).to_string(),vec![])).await;
    use_backend(&s, &mut executor, server.client.clone(), false);
    let mut r = request(&executor, OperationAction::CreateRemoteDirectory);
    r.operation.remote_id = None;
    r.identity.remote_id = None;
    r.remote.id = None;
    r.remote.state = FileState::Absent;
    r.operation.name = Some("file".into());
    r.operation.parent_id = Some(RemoteId("remote-root".into()));
    let record = executor.prepare(&r).await.unwrap();
    assert_eq!(record.reserved_remote_id, Some(RemoteId("created".into())));
    pending_record(&executor, &r, record, OperationState::Running);
    let executor = reopen(&s, executor);
    let result = executor.recover().await.unwrap();
    assert_eq!(result[0].assessment.outcome, ExecutionEffect::Applied);
    assert_eq!(
        snapshots(&executor)[0].identity.remote_id,
        Some(RemoteId("created".into()))
    );
    assert_eq!(
        snapshots(&executor)[0].baseline.state,
        BaselineState::Synced
    );
    assert!(server
        .requests
        .lock()
        .unwrap()
        .iter()
        .all(|r| r.starts_with("GET ")));
}

#[tokio::test]
async fn downloaded_bytes_including_empty_files_are_recovered_without_another_download() {
    for content in ["", "downloaded bytes"] {
        let s = Sandbox::new();
        let mut executor = setup(&s);
        let server = endpoint(move |r| {
            if r.contains("alt=media") {
                (200, content.into(), vec![])
            } else {
                (200, metadata("file", "remote-root", 1, content), vec![])
            }
        })
        .await;
        use_backend(&s, &mut executor, server.client.clone(), true);
        let mut r = request(&executor, OperationAction::Download);
        r.remote =
            normalize(&serde_json::from_str(&metadata("file", "remote-root", 1, content)).unwrap());
        assert_eq!(
            executor.execute(r).await.status,
            OperationResultStatus::Unconfirmed
        );
        let executor = reopen(&s, executor);
        assert_eq!(
            executor.recover().await.unwrap()[0].assessment.outcome,
            ExecutionEffect::Applied
        );
        assert_eq!(
            snapshots(&executor)[0].baseline.state,
            BaselineState::Synced
        );
        assert_eq!(
            std::fs::read(executor.config.local_root.join("file")).unwrap(),
            content.as_bytes()
        );
        assert_eq!(
            server
                .requests
                .lock()
                .unwrap()
                .iter()
                .filter(|r| r.contains("alt=media"))
                .count(),
            1
        );
    }
}
#[tokio::test]
async fn a_staged_but_unpublished_file_remains_blocked_and_is_not_cleaned_up() {
    struct StopAfterStage<'a>(&'a Journal);
    impl ExecutionJournal for StopAfterStage<'_> {
        fn record(&self) -> RecoveryRecord {
            self.0.record()
        }
        fn checkpoint(&self, checkpoint: RecoveryCheckpoint) -> Result<(), OperatorError> {
            self.0.checkpoint(checkpoint)?;
            Err(filesystem::stale().uncertain())
        }
    }
    let s = Sandbox::new();
    let mut executor = setup(&s);
    let server = endpoint(|_| {
        (
            200,
            metadata("file", "remote-root", 1, "staged bytes"),
            vec![],
        )
    })
    .await;
    use_backend(&s, &mut executor, server.client.clone(), false);
    let mut r = request(&executor, OperationAction::Download);
    r.remote = normalize(
        &serde_json::from_str(&metadata("file", "remote-root", 1, "staged bytes")).unwrap(),
    );
    let record = executor.prepare(&r).await.unwrap();
    let stage = record.stage_path.clone().unwrap();
    let revision = pending_record(&executor, &r, record.clone(), OperationState::Running);
    let journal = Journal::new(executor.inventory.clone(), r.clone(), revision, record);
    let mut bytes = executor.filesystem.temporary().unwrap();
    bytes.write_all(b"staged bytes").unwrap();
    assert!(executor
        .filesystem
        .install(&r, &mut bytes, Some(&StopAfterStage(&journal)))
        .is_err());
    assert!(!executor.config.local_root.join("file").exists());
    let result = executor.recover().await.unwrap();
    assert_eq!(result[0].assessment.outcome, ExecutionEffect::Unknown);
    assert_eq!(
        std::fs::read(executor.config.local_root.join(stage)).unwrap(),
        b"staged bytes"
    );
    assert_eq!(
        snapshots(&executor)[0].baseline.state,
        BaselineState::NeverSynced
    );
}
#[tokio::test]
async fn remote_creation_with_missing_marker_or_inaccessible_id_is_never_replayed() {
    for accessible in [false, true] {
        let s = Sandbox::new();
        let mut executor = setup(&s);
        std::fs::create_dir(executor.config.local_root.join("file")).unwrap();
        let server=endpoint(move |_|if accessible {(200,serde_json::json!({"id":"created","name":"file","parents":["remote-root"],"mimeType":FOLDER,"version":"1","trashed":false}).to_string(),vec![])}else{(404,r#"{"error":{"code":404,"message":"missing or inaccessible"}}"#.into(),vec![])}).await;
        use_backend(&s, &mut executor, server.client.clone(), false);
        let mut r = request(&executor, OperationAction::CreateRemoteDirectory);
        r.operation.remote_id = None;
        r.identity.remote_id = None;
        r.remote.id = None;
        r.remote.state = FileState::Absent;
        r.operation.name = Some("file".into());
        r.operation.parent_id = Some(RemoteId("remote-root".into()));
        let record = executor.prepare(&r).await.unwrap();
        pending_record(&executor, &r, record, OperationState::Running);
        assert_eq!(
            executor.recover().await.unwrap()[0].assessment.outcome,
            ExecutionEffect::Unknown
        );
        assert_eq!(
            snapshots(&executor)[0].baseline.state,
            BaselineState::NeverSynced
        );
        assert!(server
            .requests
            .lock()
            .unwrap()
            .iter()
            .all(|r| r.starts_with("GET ")));
    }
}
#[tokio::test]
async fn a_remote_create_response_timeout_is_reconciled_by_reserved_id_without_a_second_post() {
    use crate::accounts::tests::http_support::{Reply, Server};
    use std::time::Duration;
    let s = Sandbox::new();
    let mut executor = setup(&s);
    std::fs::create_dir(executor.config.local_root.join("file")).unwrap();
    let created=serde_json::json!({"id":"created","name":"file","parents":["remote-root"],"mimeType":FOLDER,"version":"1","trashed":false,"appProperties":{"drivesyncExecution":"execution"}}).to_string();
    let mut delayed = Reply::json(200, r#"{"id":"created"}"#);
    delayed.delay = Duration::from_millis(300);
    let server = Server::start(vec![
        Reply::json(200, &root()),
        Reply::json(200, r#"{"files":[],"incompleteSearch":false}"#),
        delayed,
        Reply::json(200, &created),
        Reply::json(200, &created),
        Reply::json(200, &created),
    ]);
    let client = GoogleDriveClient::with_base_urls(
        format!("{}/drive/v3", server.base),
        format!("{}/upload/drive/v3", server.base),
    )
    .unwrap();
    let short = client
        .clone()
        .with_limits(GoogleClientLimits {
            request_timeout: Duration::from_millis(100),
            max_metadata_response_bytes: 1024 * 1024,
        })
        .unwrap();
    use_backend(&s, &mut executor, short, false);
    let mut r = request(&executor, OperationAction::CreateRemoteDirectory);
    r.operation.remote_id = None;
    r.identity.remote_id = None;
    r.remote.id = None;
    r.remote.state = FileState::Absent;
    r.operation.name = Some("file".into());
    r.operation.parent_id = Some(RemoteId("remote-root".into()));
    assert_eq!(
        executor.execute(r).await.status,
        OperationResultStatus::Unconfirmed
    );
    let slow = client
        .with_limits(GoogleClientLimits {
            request_timeout: Duration::from_secs(2),
            max_metadata_response_bytes: 1024 * 1024,
        })
        .unwrap();
    executor.remote = Arc::new(ScriptBackend {
        google: GoogleOperator::new(
            Accounts::new(crate::accounts::AccountsConfig::new(
                s.0.join("recovery-accounts"),
                "test.recovery",
                Arc::new(super::tests::Associations),
            ))
            .unwrap(),
            slow,
        ),
        lose_receipt: false,
    });
    assert_eq!(
        executor.recover().await.unwrap()[0].assessment.outcome,
        ExecutionEffect::Applied
    );
    let calls = server.requests.lock().unwrap();
    assert_eq!(calls.iter().filter(|r| r.method == "POST").count(), 1);
    let post = calls.iter().find(|r| r.method == "POST").unwrap();
    let body: serde_json::Value = serde_json::from_slice(&post.body).unwrap();
    assert_eq!(body["id"], "created");
    assert_eq!(body["appProperties"]["drivesyncExecution"], "execution");
}

#[tokio::test]
async fn source_changes_during_remote_reconciliation_prevent_confirmation() {
    let s = Sandbox::new();
    let mut executor = setup(&s);
    std::fs::write(executor.config.local_root.join("file"), b"source").unwrap();
    let path = executor.config.local_root.join("file");
    let reads = std::sync::Mutex::new(0usize);
    let server = endpoint(move |_| {
        let mut count = reads.lock().unwrap();
        *count += 1;
        if *count == 3 {
            std::fs::write(&path, b"concurrent change").unwrap();
        }
        let mut value: serde_json::Value =
            serde_json::from_str(&metadata("file", "remote-root", 1, "source")).unwrap();
        value["id"] = "created".into();
        value["appProperties"] = serde_json::json!({"drivesyncExecution":"execution"});
        (200, value.to_string(), vec![])
    })
    .await;
    use_backend(&s, &mut executor, server.client.clone(), false);
    let mut r = request(&executor, OperationAction::Upload);
    r.operation.parent_id = Some(RemoteId("remote-root".into()));
    r.operation.remote_id = None;
    r.identity.remote_id = None;
    r.remote.id = None;
    r.remote.state = FileState::Absent;
    r.operation.name = Some("file".into());
    r.operation.parent_id = Some(RemoteId("remote-root".into()));
    let record = executor.prepare(&r).await.unwrap();
    pending_record(&executor, &r, record, OperationState::Running);
    let result = executor.recover().await.unwrap();
    assert_eq!(result[0].assessment.outcome, ExecutionEffect::Unknown);
    assert_eq!(
        result[0].assessment.issue,
        Some(RecoveryIssue::SourceChanged)
    );
    assert_eq!(
        snapshots(&executor)[0].baseline.state,
        BaselineState::NeverSynced
    );
    assert_eq!(
        std::fs::read(executor.config.local_root.join("file")).unwrap(),
        b"concurrent change"
    );
}
#[tokio::test]
async fn concurrent_inventory_revision_rejects_recovery_without_losing_the_effect() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let s = Sandbox::new();
    let mut executor = setup(&s);
    let r = request(&executor, OperationAction::CreateLocalDirectory);
    let inventory = executor.inventory.clone();
    let identity = r.identity.clone();
    let race = Arc::new(AtomicBool::new(false));
    let trigger = race.clone();
    let server = endpoint(move |_| {
        if trigger.swap(false, Ordering::SeqCst) {
            let read = inventory.lookup(InventoryLookup {
                account_id: identity.account_id.clone(),
                sync_root_id: identity.sync_root_id.clone(),
                item_id: identity.item_id.clone(),
                remote_id: None,
                local_identity: Evidence::NotCollected,
                local_path: None,
            });
            assert_eq!(
                inventory
                    .write(InventoryWrite {
                        operation_id: Id("race".into()),
                        item_id: identity.item_id.clone().unwrap(),
                        identity: identity.clone(),
                        expected_revision: read.snapshot.map(|s| s.revision),
                        effect: PersistedEffect::ConflictUpdated(None),
                        recorded_at: now()
                    })
                    .status,
                WriteStatus::Written
            );
        }
        (200, directory_metadata(), vec![])
    })
    .await;
    use_backend(&s, &mut executor, server.client.clone(), true);
    assert_eq!(
        executor.execute(r).await.status,
        OperationResultStatus::Unconfirmed
    );
    race.store(true, Ordering::SeqCst);
    let result = executor.recover().await.unwrap();
    assert_eq!(
        result[0].assessment.issue,
        Some(RecoveryIssue::InventoryConflict)
    );
    assert_eq!(
        snapshots(&executor)[0].baseline.state,
        BaselineState::NeverSynced
    );
    assert!(executor.config.local_root.join("file").is_dir());
    assert_eq!(
        executor.recover().await.unwrap()[0].assessment.outcome,
        ExecutionEffect::Applied
    );
}

#[tokio::test]
async fn a_lost_update_receipt_is_recovered_by_marker_without_retransmitting_content() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let s = Sandbox::new();
    let mut executor = setup(&s);
    std::fs::write(executor.config.local_root.join("file"), b"replacement").unwrap();
    let address = Arc::new(std::sync::Mutex::new(String::new()));
    let location = address.clone();
    let uploaded = Arc::new(AtomicBool::new(false));
    let changed = uploaded.clone();
    let server = endpoint(move |r| {
        if r.starts_with("PUT ") && r.contains("uploadType=resumable") {
            return (
                200,
                String::new(),
                vec![(
                    "Location".into(),
                    format!(
                        "{}/upload/drive/v2/files/remote?upload_id=update",
                        location.lock().unwrap()
                    ),
                )],
            );
        }
        if r.starts_with("PUT ") {
            changed.store(true, Ordering::SeqCst);
        }
        let complete = changed.load(Ordering::SeqCst);
        let mut value: serde_json::Value = serde_json::from_str(&metadata(
            "file",
            "remote-root",
            if complete { 2 } else { 1 },
            if complete { "replacement" } else { "old" },
        ))
        .unwrap();
        if complete {
            value["appProperties"] = serde_json::json!({"drivesyncExecution":"execution"});
        }
        (
            200,
            value.to_string(),
            vec![("ETag".into(), "\"v1\"".into())],
        )
    })
    .await;
    *address.lock().unwrap() = server.base.clone();
    use_backend(&s, &mut executor, server.client.clone(), true);
    let mut r = request(&executor, OperationAction::UpdateRemoteContent);
    r.operation.name = Some("file".into());
    r.remote =
        normalize(&serde_json::from_str(&metadata("file", "remote-root", 1, "old")).unwrap());
    let result = executor.execute(r).await;
    assert_eq!(result.status, OperationResultStatus::Unconfirmed);
    assert!(uploaded.load(Ordering::SeqCst));
    let path = executor.config.local_root.join("file");
    let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
    std::fs::File::open(&path)
        .unwrap()
        .set_modified(modified + std::time::Duration::from_secs(1))
        .unwrap();
    let reopened = reopen(&s, executor);
    let result = reopened.recover().await.unwrap();
    assert_eq!(result[0].assessment.outcome, ExecutionEffect::Applied);
    assert_eq!(
        snapshots(&reopened)[0].baseline.state,
        BaselineState::Synced
    );
    assert_eq!(
        server
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.starts_with("PUT ") && !r.contains("uploadType=resumable"))
            .count(),
        1
    );
}

#[tokio::test]
async fn local_update_after_exchange_recovers_new_inode_and_preserves_old_bytes_in_trash() {
    let s = Sandbox::new();
    let mut executor = setup(&s);
    std::fs::write(executor.config.local_root.join("file"), b"old bytes").unwrap();
    let server = endpoint(|_| {
        (
            200,
            metadata("file", "remote-root", 2, "replacement"),
            vec![],
        )
    })
    .await;
    use_backend(&s, &mut executor, server.client.clone(), false);
    let mut r = request(&executor, OperationAction::UpdateLocalContent);
    r.remote = normalize(
        &serde_json::from_str(&metadata("file", "remote-root", 1, "replacement")).unwrap(),
    );
    let old = r.local.identity.clone();
    let record = executor.prepare(&r).await.unwrap();
    let revision = pending_record(&executor, &r, record.clone(), OperationState::Running);
    let journal = Journal::new(executor.inventory.clone(), r.clone(), revision, record);
    let data = s.0.join("trash-home");
    std::fs::create_dir(&data).unwrap();
    let mut bytes = executor.filesystem.temporary().unwrap();
    bytes.write_all(b"replacement").unwrap();
    let local = executor
        .filesystem
        .install_with_test_trash(&r, &mut bytes, &data, &journal)
        .unwrap();
    assert_ne!(local.identity, old);
    drop(journal);
    let reopened = reopen(&s, executor);
    let recovered = reopened.recover().await.unwrap();
    assert_eq!(recovered[0].assessment.outcome, ExecutionEffect::Applied);
    assert_eq!(
        snapshots(&reopened)[0].baseline.state,
        BaselineState::Synced
    );
    let files = std::fs::read_dir(data.join("Trash/files"))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(std::fs::read(files[0].path()).unwrap(), b"old bytes");
    assert!(server
        .requests
        .lock()
        .unwrap()
        .iter()
        .all(|r| r.starts_with("GET ") && !r.contains("alt=media")));
}
