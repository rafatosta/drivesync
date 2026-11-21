//! Isolated executor tests never access real accounts, credentials, or Drive.
use super::*;
use crate::inventory::InventoryConfig;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static COUNTER: AtomicU64 = AtomicU64::new(0);
pub(super) struct Sandbox(pub(super) PathBuf);
impl Sandbox {
    pub(super) fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "drivesync-operations-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
struct LocalBackend;
impl RemoteExecutor for LocalBackend {
    fn execute<'a>(
        &'a self,
        _: &'a OperatorConfig,
        r: &'a ExecuteOperationRequest,
        fs: &'a Filesystem,
    ) -> EffectFuture<'a> {
        Box::pin(async move { fs.execute(r) })
    }
}
struct UncertainBackend;
impl RemoteExecutor for UncertainBackend {
    fn execute<'a>(
        &'a self,
        _: &'a OperatorConfig,
        _: &'a ExecuteOperationRequest,
        _: &'a Filesystem,
    ) -> EffectFuture<'a> {
        Box::pin(async { Err(OperatorError::new(OperatorErrorKind::Unavailable).uncertain()) })
    }
}
struct FailedBackend;
impl RemoteExecutor for FailedBackend {
    fn execute<'a>(
        &'a self,
        _: &'a OperatorConfig,
        _: &'a ExecuteOperationRequest,
        _: &'a Filesystem,
    ) -> EffectFuture<'a> {
        Box::pin(async { Err(OperatorError::new(OperatorErrorKind::PermissionDenied)) })
    }
}
pub(super) fn setup(s: &Sandbox) -> PersistingOperator {
    let root = s.0.join("root");
    fs::create_dir(&root).unwrap();
    PersistingOperator {
        config: OperatorConfig {
            account_id: AccountId("account".into()),
            sync_root_id: SyncRootId("root".into()),
            provider_id: ProviderId("google".into()),
            local_root: root.clone(),
            remote_root_id: RemoteId("remote-root".into()),
        },
        inventory: Arc::new(
            Inventory::open(InventoryConfig {
                directory: s.0.join("inventory"),
                account_id: AccountId("account".into()),
            })
            .unwrap(),
        ),
        filesystem: Filesystem::open(&root).unwrap(),
        remote: Arc::new(LocalBackend),
        gate: tokio::sync::Mutex::new(()),
    }
}
pub(super) fn request(
    executor: &PersistingOperator,
    action: OperationAction,
) -> ExecuteOperationRequest {
    let mut local = LocalState {
        state: FileState::Absent,
        path: Some(PathBuf::from("file")),
        name: Evidence::Known("file".into()),
        parent: Evidence::Known(PathBuf::new()),
        kind: Evidence::Known(FileKind::Directory),
        identity: Evidence::NotCollected,
        size: Evidence::NotApplicable,
        modified_at: Evidence::NotCollected,
        changed_at: Evidence::NotCollected,
        checksum: Evidence::NotApplicable,
        capabilities: Evidence::NotCollected,
    };
    if executor.config.local_root.join("file").exists() {
        local = executor
            .filesystem
            .observe(std::path::Path::new("file"))
            .unwrap();
    }
    ExecuteOperationRequest {
        operation: Operation {
            step: 1,
            action,
            item_ref: ItemId("item".into()),
            depends_on: vec![],
            remote_id: Some(RemoteId("remote".into())),
            local_path: Some("file".into()),
            name: Some("renamed".into()),
            parent_id: None,
            parent: Some("destination".into()),
            preconditions: Some(OperationPreconditions {
                observed_remote_version: Some(1),
                baseline_remote_version: None,
                local_unchanged: Evidence::Known(true),
                remote_unchanged: Evidence::Known(true),
                destination_free: Evidence::Known(true),
            }),
        },
        execution_id: Id("execution".into()),
        expected_revision: Some(0),
        identity: FileIdentity {
            account_id: executor.config.account_id.clone(),
            sync_root_id: executor.config.sync_root_id.clone(),
            item_id: Some(ItemId("item".into())),
            remote_id: Some(RemoteId("remote".into())),
            local_identity: local.identity.clone(),
        },
        local,
        remote: RemoteState {
            state: FileState::Present,
            id: Some(RemoteId("remote".into())),
            name: Evidence::Known("file".into()),
            parents: Evidence::Known(vec![RemoteId("remote-root".into())]),
            mime_type: Evidence::Known("application/vnd.google-apps.folder".into()),
            size: Evidence::NotApplicable,
            checksum: Evidence::NotApplicable,
            version: Evidence::Known(1),
            modified_time: Evidence::NotCollected,
            trashed: Evidence::Known(false),
            capabilities: Evidence::NotCollected,
        },
        completed_steps: vec![],
    }
}
fn snapshot(executor: &PersistingOperator) -> InventorySnapshot {
    executor
        .inventory
        .lookup(InventoryLookup {
            account_id: executor.config.account_id.clone(),
            sync_root_id: executor.config.sync_root_id.clone(),
            item_id: Some(ItemId("item".into())),
            remote_id: None,
            local_identity: Evidence::NotCollected,
            local_path: None,
        })
        .snapshot
        .unwrap()
}
#[tokio::test]
async fn directory_creation_is_persisted_and_replay_is_refused() {
    let s = Sandbox::new();
    let executor = setup(&s);
    let r = request(&executor, OperationAction::CreateLocalDirectory);
    let result = executor.execute(r.clone()).await;
    assert_eq!(result.status, OperationResultStatus::Succeeded);
    assert!(executor.config.local_root.join("file").is_dir());
    let state = snapshot(&executor);
    assert_eq!(state.pending_operations[0].state, OperationState::Confirmed);
    assert_eq!(state.baseline.state, BaselineState::NeverSynced);
    assert_eq!(
        executor.execute(r).await.status,
        OperationResultStatus::Skipped
    );
}
#[tokio::test]
async fn confirmed_whole_item_convergence_advances_baseline() {
    struct BothSides;
    impl RemoteExecutor for BothSides {
        fn execute<'a>(
            &'a self,
            _: &'a OperatorConfig,
            r: &'a ExecuteOperationRequest,
            fs: &'a Filesystem,
        ) -> EffectFuture<'a> {
            Box::pin(async move {
                let mut effect = fs.execute(r)?;
                effect.remote = Some(r.remote.clone());
                Ok(effect)
            })
        }
    }
    let s = Sandbox::new();
    let mut executor = setup(&s);
    executor.remote = Arc::new(BothSides);
    assert_eq!(
        executor
            .execute(request(&executor, OperationAction::CreateLocalDirectory))
            .await
            .status,
        OperationResultStatus::Succeeded
    );
    let state = snapshot(&executor);
    assert_eq!(state.baseline.state, BaselineState::Synced);
    assert_eq!(state.pending_operations[0].state, OperationState::Confirmed);
}
#[tokio::test]
async fn uncertain_network_effect_is_durable_and_blocks_new_executions() {
    let s = Sandbox::new();
    let mut executor = setup(&s);
    executor.remote = Arc::new(UncertainBackend);
    let result = executor
        .execute(request(&executor, OperationAction::RenameRemote))
        .await;
    assert_eq!(result.status, OperationResultStatus::Unconfirmed);
    assert!(!result.error.unwrap().retryable);
    let state = snapshot(&executor);
    assert_eq!(
        state.pending_operations[0].state,
        OperationState::AppliedUnconfirmed
    );
    assert_eq!(state.baseline.state, BaselineState::NeverSynced);
    let mut retry = request(&executor, OperationAction::RenameRemote);
    retry.execution_id = Id("other".into());
    retry.expected_revision = Some(state.revision);
    assert_eq!(
        executor.execute(retry).await.status,
        OperationResultStatus::Skipped
    );
}
#[tokio::test]
async fn known_failure_is_not_a_success_or_baseline() {
    let s = Sandbox::new();
    let mut executor = setup(&s);
    executor.remote = Arc::new(FailedBackend);
    assert_eq!(
        executor
            .execute(request(&executor, OperationAction::RenameRemote))
            .await
            .status,
        OperationResultStatus::Failed
    );
    let state = snapshot(&executor);
    assert_eq!(state.pending_operations[0].state, OperationState::Failed);
    assert_eq!(state.baseline.state, BaselineState::NeverSynced);
}
#[tokio::test]
async fn mismatched_scope_missing_dependencies_and_stale_revisions_have_no_effect() {
    let s = Sandbox::new();
    let executor = setup(&s);
    let original = request(&executor, OperationAction::CreateLocalDirectory);
    for index in 0..7 {
        let mut r = original.clone();
        match index {
            0 => r.identity.account_id = AccountId("other".into()),
            1 => r.identity.sync_root_id = SyncRootId("other".into()),
            2 => r.identity.item_id = Some(ItemId("other".into())),
            3 => r.operation.depends_on = vec![1],
            4 => r.expected_revision = Some(42),
            5 => r.operation.local_path = Some("../outside".into()),
            _ => r.operation.preconditions = None,
        }
        assert_eq!(
            executor.execute(r).await.status,
            OperationResultStatus::Skipped
        );
    }
    assert!(!executor.config.local_root.join("file").exists());
}
#[tokio::test]
async fn rename_and_move_preserve_bytes_and_refuse_collisions() {
    let s = Sandbox::new();
    let executor = setup(&s);
    fs::write(executor.config.local_root.join("file"), b"original").unwrap();
    fs::write(executor.config.local_root.join("renamed"), b"occupied").unwrap();
    assert!(executor
        .filesystem
        .execute(&request(&executor, OperationAction::RenameLocal))
        .is_err());
    assert_eq!(
        fs::read(executor.config.local_root.join("renamed")).unwrap(),
        b"occupied"
    );
    fs::create_dir(executor.config.local_root.join("destination")).unwrap();
    let effect = executor
        .filesystem
        .execute(&request(&executor, OperationAction::MoveLocal))
        .unwrap();
    assert_eq!(effect.local.unwrap().path, Some("destination/file".into()));
    assert_eq!(
        fs::read(executor.config.local_root.join("destination/file")).unwrap(),
        b"original"
    );
}
#[test]
fn symlinks_traversal_internal_paths_and_special_files_are_rejected() {
    use std::os::unix::fs::symlink;
    let s = Sandbox::new();
    let executor = setup(&s);
    let outside = s.0.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("secret"), b"untouched").unwrap();
    symlink(&outside, executor.config.local_root.join("link")).unwrap();
    for path in [
        "../outside/secret",
        "/etc/passwd",
        "link/secret",
        ".drivesync/inventory",
    ] {
        assert!(executor
            .filesystem
            .read(std::path::Path::new(path))
            .is_err());
    }
    symlink(
        outside.join("secret"),
        executor.config.local_root.join("file"),
    )
    .unwrap();
    assert!(executor
        .filesystem
        .observe(std::path::Path::new("file"))
        .is_err());
    assert_eq!(fs::read(outside.join("secret")).unwrap(), b"untouched");
    assert!(Filesystem::open(&executor.config.local_root.join("link")).is_err());
}
#[test]
fn changed_local_evidence_and_non_utf8_paths_are_handled() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let s = Sandbox::new();
    let executor = setup(&s);
    fs::write(executor.config.local_root.join("file"), b"original").unwrap();
    let r = request(&executor, OperationAction::RenameLocal);
    fs::write(executor.config.local_root.join("file"), b"changed").unwrap();
    assert!(executor.filesystem.execute(&r).is_err());
    let path = PathBuf::from(OsString::from_vec(vec![0xff]));
    fs::write(executor.config.local_root.join(&path), b"bytes").unwrap();
    assert_eq!(executor.filesystem.observe(&path).unwrap().path, Some(path));
}
#[test]
fn download_staging_never_overwrites_existing_destination() {
    let s = Sandbox::new();
    let executor = setup(&s);
    let r = request(&executor, OperationAction::Download);
    fs::write(executor.config.local_root.join("file"), b"concurrent edit").unwrap();
    let mut bytes = executor.filesystem.temporary().unwrap();
    use std::io::Write;
    bytes.write_all(b"download").unwrap();
    assert!(executor.filesystem.install(&r, &mut bytes).is_err());
    assert_eq!(
        fs::read(executor.config.local_root.join("file")).unwrap(),
        b"concurrent edit"
    );
}
#[test]
fn baseline_requires_complete_matching_evidence() {
    let s = Sandbox::new();
    let executor = setup(&s);
    let r = request(&executor, OperationAction::CreateLocalDirectory);
    let mut effect = AppliedEffect {
        local: Some(r.local),
        remote: Some(r.remote),
        remote_id: r.operation.remote_id,
        verified: true,
    };
    assert!(executor.baseline(&effect).is_none());
    effect.local.as_mut().unwrap().state = FileState::Present;
    assert!(executor.baseline(&effect).is_some());
    effect.remote.as_mut().unwrap().name = Evidence::Known("different".into());
    assert!(executor.baseline(&effect).is_none());
}

#[tokio::test]
async fn applied_effect_is_retained_when_the_final_inventory_write_conflicts() {
    struct RevisionRace {
        inventory: Arc<Inventory>,
    }
    impl RemoteExecutor for RevisionRace {
        fn execute<'a>(
            &'a self,
            _: &'a OperatorConfig,
            r: &'a ExecuteOperationRequest,
            fs: &'a Filesystem,
        ) -> EffectFuture<'a> {
            Box::pin(async move {
                let effect = fs.execute(r)?;
                let write = self.inventory.write(InventoryWrite {
                    operation_id: r.execution_id.clone(),
                    item_id: r.operation.item_ref.clone(),
                    identity: r.identity.clone(),
                    expected_revision: None,
                    effect: PersistedEffect::ConflictUpdated(None),
                    recorded_at: now(),
                });
                assert_eq!(write.status, WriteStatus::Written);
                Ok(effect)
            })
        }
    }
    let s = Sandbox::new();
    let mut executor = setup(&s);
    executor.remote = Arc::new(RevisionRace {
        inventory: executor.inventory.clone(),
    });
    let result = executor
        .execute(request(&executor, OperationAction::CreateLocalDirectory))
        .await;
    assert_eq!(result.status, OperationResultStatus::Unconfirmed);
    assert!(result.effect.unwrap().verified);
    assert_eq!(
        result.inventory_write.unwrap().status,
        WriteStatus::RevisionConflict
    );
    assert_eq!(
        snapshot(&executor).pending_operations[0].state,
        OperationState::Running
    );
    assert!(executor.config.local_root.join("file").is_dir());
}
#[tokio::test]
async fn cancellation_after_dispatch_keeps_a_running_record_for_recovery() {
    struct Paused {
        started: tokio::sync::Notify,
    }
    impl RemoteExecutor for Paused {
        fn execute<'a>(
            &'a self,
            _: &'a OperatorConfig,
            _: &'a ExecuteOperationRequest,
            _: &'a Filesystem,
        ) -> EffectFuture<'a> {
            Box::pin(async move {
                self.started.notify_one();
                std::future::pending().await
            })
        }
    }
    let s = Sandbox::new();
    let mut executor = setup(&s);
    let backend = Arc::new(Paused {
        started: tokio::sync::Notify::new(),
    });
    executor.remote = backend.clone();
    let executor = Arc::new(executor);
    let r = request(&executor, OperationAction::CreateLocalDirectory);
    let running = executor.clone();
    let task = tokio::spawn(async move { running.execute(r).await });
    backend.started.notified().await;
    task.abort();
    let _ = task.await;
    assert_eq!(
        snapshot(&executor).pending_operations[0].state,
        OperationState::Running
    );
    assert_eq!(
        executor
            .execute(request(&executor, OperationAction::CreateLocalDirectory))
            .await
            .status,
        OperationResultStatus::Skipped
    );
}

#[tokio::test]
async fn replaced_root_is_rejected_before_writing_or_mutating() {
    let s = Sandbox::new();
    let executor = setup(&s);
    let r = request(&executor, OperationAction::CreateLocalDirectory);
    let moved = s.0.join("moved-root");
    fs::rename(&executor.config.local_root, &moved).unwrap();
    fs::create_dir(&executor.config.local_root).unwrap();
    assert_eq!(
        executor.execute(r).await.status,
        OperationResultStatus::Skipped
    );
    assert!(!moved.join("file").exists());
    assert!(!executor.config.local_root.join("file").exists());
}
