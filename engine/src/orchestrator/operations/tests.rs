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
        journal: Option<&'a dyn ExecutionJournal>,
    ) -> EffectFuture<'a> {
        Box::pin(async move {
            if matches!(
                r.operation.action,
                OperationAction::RegisterIdentity
                    | OperationAction::Adopt
                    | OperationAction::ConfirmBaseline
            ) {
                let local = if r.local.state == FileState::Present {
                    fs.check(&r.local)?
                } else {
                    r.local.clone()
                };
                return Ok(AppliedEffect {
                    local: Some(local),
                    remote: Some(r.remote.clone()),
                    remote_id: r.remote.id.clone(),
                    verified: true,
                });
            }
            fs.execute(r, journal)
        })
    }
}
struct UncertainBackend;
impl RemoteExecutor for UncertainBackend {
    fn execute<'a>(
        &'a self,
        _: &'a OperatorConfig,
        _: &'a ExecuteOperationRequest,
        _: &'a Filesystem,
        _: Option<&'a dyn ExecutionJournal>,
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
        _: Option<&'a dyn ExecutionJournal>,
    ) -> EffectFuture<'a> {
        Box::pin(async { Err(OperatorError::new(OperatorErrorKind::PermissionDenied)) })
    }
}
pub(super) fn setup(s: &Sandbox) -> PersistingOperator {
    let root = s.0.join("root");
    fs::create_dir(&root).unwrap();
    PersistingOperator {
        config: OperatorConfig {
            expected_root_identity: None,
            trash_propagation_authorized: false,
            initial_association_authorized: false,
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
            conflict_id: None,
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
            journal: Option<&'a dyn ExecutionJournal>,
        ) -> EffectFuture<'a> {
            Box::pin(async move {
                let mut effect = fs.execute(r, journal)?;
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
        .execute(&request(&executor, OperationAction::RenameLocal), None)
        .is_err());
    assert_eq!(
        fs::read(executor.config.local_root.join("renamed")).unwrap(),
        b"occupied"
    );
    fs::create_dir(executor.config.local_root.join("destination")).unwrap();
    let effect = executor
        .filesystem
        .execute(&request(&executor, OperationAction::MoveLocal), None)
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
    assert!(executor.filesystem.execute(&r, None).is_err());
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
    assert!(executor.filesystem.install(&r, &mut bytes, None).is_err());
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
    effect.local.as_mut().unwrap().identity = Evidence::Known(LocalIdentity {
        device_id: 1,
        inode: 2,
        mount_id: None,
        birth_ns: None,
    });
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
            journal: Option<&'a dyn ExecutionJournal>,
        ) -> EffectFuture<'a> {
            Box::pin(async move {
                let effect = fs.execute(r, journal)?;
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
            _: Option<&'a dyn ExecutionJournal>,
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

#[tokio::test]
async fn confirmed_replacement_refreshes_persisted_local_identity() {
    struct Replace;
    impl RemoteExecutor for Replace {
        fn execute<'a>(
            &'a self,
            config: &'a OperatorConfig,
            r: &'a ExecuteOperationRequest,
            filesystem: &'a Filesystem,
            _: Option<&'a dyn ExecutionJournal>,
        ) -> EffectFuture<'a> {
            Box::pin(async move {
                let temporary = config.local_root.join("replacement");
                fs::write(&temporary, b"replacement bytes").unwrap();
                fs::rename(temporary, config.local_root.join("file")).unwrap();
                let local = filesystem.observe(std::path::Path::new("file"))?;
                Ok(AppliedEffect {
                    local: Some(local),
                    remote: None,
                    remote_id: r.operation.remote_id.clone(),
                    verified: true,
                })
            })
        }
    }
    let s = Sandbox::new();
    let mut executor = setup(&s);
    fs::write(executor.config.local_root.join("file"), b"old bytes").unwrap();
    let r = request(&executor, OperationAction::UpdateLocalContent);
    let old_identity = r.local.identity.clone();
    executor.remote = Arc::new(Replace);
    let result = executor.execute(r).await;
    assert_eq!(result.status, OperationResultStatus::Succeeded);
    let actual = result.effect.unwrap().local.unwrap().identity;
    assert_ne!(actual, old_identity);
    assert_eq!(snapshot(&executor).identity.local_identity, actual);
}

fn association_request(
    executor: &PersistingOperator,
    path: &str,
    id: &str,
    parent: &str,
) -> ExecuteOperationRequest {
    let mut r = request(executor, OperationAction::Adopt);
    r.local = executor
        .filesystem
        .observe(std::path::Path::new(path))
        .unwrap();
    r.operation.local_path = r.local.path.clone();
    r.operation.name = match &r.local.name {
        Evidence::Known(name) => Some(name.clone()),
        _ => None,
    };
    r.operation.parent = match &r.local.parent {
        Evidence::Known(parent) => Some(parent.clone()),
        _ => None,
    };
    r.operation.parent_id = Some(RemoteId(parent.into()));
    r.operation.item_ref = ItemId(id.into());
    r.operation.remote_id = Some(RemoteId(format!("remote-{id}")));
    r.identity.item_id = Some(ItemId(id.into()));
    r.identity.remote_id = r.operation.remote_id.clone();
    r.identity.local_identity = r.local.identity.clone();
    r.execution_id = Id(format!("associate-{id}"));
    r.remote.id = r.operation.remote_id.clone();
    r.remote.name = r.local.name.clone();
    r.remote.parents = Evidence::Known(vec![RemoteId(parent.into())]);
    r.remote.size = r.local.size.clone();
    r.remote.checksum = r.local.checksum.clone();
    r.remote.mime_type = Evidence::Known(
        if r.local.kind == Evidence::Known(FileKind::Directory) {
            "application/vnd.google-apps.folder"
        } else {
            "application/octet-stream"
        }
        .into(),
    );
    r
}
fn rows(executor: &PersistingOperator) -> Vec<InventorySnapshot> {
    executor
        .inventory
        .enumerate(InventoryScopeQuery {
            account_id: executor.config.account_id.clone(),
            sync_root_id: executor.config.sync_root_id.clone(),
        })
        .unwrap()
}
#[tokio::test]
async fn adoption_requires_explicit_authorization_and_preserves_existing_bytes() {
    let s = Sandbox::new();
    let mut executor = setup(&s);
    fs::write(executor.config.local_root.join("file"), b"preserved").unwrap();
    let r = association_request(&executor, "file", "item", "remote-root");
    assert_eq!(
        executor.execute(r.clone()).await.status,
        OperationResultStatus::Skipped
    );
    assert!(rows(&executor).is_empty());
    executor.config.initial_association_authorized = true;
    let result = executor.execute(r).await;
    assert_eq!(result.status, OperationResultStatus::Succeeded);
    let registered = rows(&executor);
    assert_eq!(registered.len(), 1);
    assert_eq!(registered[0].revision, 1);
    assert_eq!(registered[0].baseline.state, BaselineState::Synced);
    assert!(registered[0].pending_operations.is_empty());
    assert_eq!(
        fs::read(executor.config.local_root.join("file")).unwrap(),
        b"preserved"
    );
    let reopened = Inventory::open(InventoryConfig {
        directory: s.0.join("inventory"),
        account_id: executor.config.account_id.clone(),
    })
    .unwrap();
    let lookup = reopened.lookup(InventoryLookup {
        account_id: executor.config.account_id.clone(),
        sync_root_id: executor.config.sync_root_id.clone(),
        item_id: None,
        remote_id: Some(RemoteId("remote-item".into())),
        local_identity: registered[0].identity.local_identity.clone(),
        local_path: Some(Path::from("file")),
    });
    assert_eq!(lookup.status, InventoryLookupStatus::Found);
    assert_eq!(lookup.snapshot.unwrap(), registered[0]);
}
#[tokio::test]
async fn metadata_registration_has_no_baseline_and_concurrent_proposals_cannot_duplicate_a_path() {
    let s = Sandbox::new();
    let executor = setup(&s);
    fs::write(executor.config.local_root.join("file"), b"local-only").unwrap();
    let mut r = association_request(&executor, "file", "item", "remote-root");
    r.operation.action = OperationAction::RegisterIdentity;
    r.remote.id = None;
    r.identity.remote_id = None;
    r.operation.remote_id = None;
    r.remote.state = FileState::Absent;
    r.operation
        .preconditions
        .as_mut()
        .unwrap()
        .observed_remote_version = None;
    assert_eq!(
        executor.execute(r.clone()).await.status,
        OperationResultStatus::Succeeded
    );
    let registered = rows(&executor);
    assert_eq!(registered[0].baseline.state, BaselineState::NeverSynced);
    assert_eq!(registered[0].local_path, Some(Path::from("file")));
    r.identity.item_id = Some(ItemId("other-proposal".into()));
    r.operation.item_ref = ItemId("other-proposal".into());
    assert_eq!(
        executor.execute(r).await.status,
        OperationResultStatus::Skipped
    );
    assert_eq!(rows(&executor).len(), 1);
}
#[tokio::test]
async fn nested_directories_and_empty_files_adopt_without_content_transfer() {
    let s = Sandbox::new();
    let mut executor = setup(&s);
    executor.config.initial_association_authorized = true;
    fs::create_dir_all(executor.config.local_root.join("a/b/c")).unwrap();
    fs::write(executor.config.local_root.join("a/b/c/empty"), b"").unwrap();
    for (path, id, parent) in [
        ("a", "a", "remote-root"),
        ("a/b", "b", "remote-a"),
        ("a/b/c", "c", "remote-b"),
        ("a/b/c/empty", "empty", "remote-c"),
    ] {
        let r = association_request(&executor, path, id, parent);
        assert_eq!(
            executor.execute(r).await.status,
            OperationResultStatus::Succeeded,
            "{path}"
        );
    }
    assert_eq!(
        rows(&executor)
            .iter()
            .filter(|r| r.baseline.state == BaselineState::Synced)
            .count(),
        4
    );
    assert_eq!(
        fs::read(executor.config.local_root.join("a/b/c/empty")).unwrap(),
        b""
    );
}
#[tokio::test]
async fn stale_revision_or_divergent_effect_never_confirms_a_baseline() {
    let s = Sandbox::new();
    let mut executor = setup(&s);
    executor.config.initial_association_authorized = true;
    fs::write(executor.config.local_root.join("file"), b"preserved").unwrap();
    let mut r = association_request(&executor, "file", "item", "remote-root");
    r.remote.checksum = Evidence::Known(Checksum {
        algorithm: "sha256".into(),
        value: "different".into(),
    });
    assert_eq!(
        executor.execute(r).await.status,
        OperationResultStatus::Skipped
    );
    assert!(rows(&executor).is_empty());
    let r = association_request(&executor, "file", "item", "remote-root");
    assert_eq!(
        executor.execute(r.clone()).await.status,
        OperationResultStatus::Succeeded
    );
    assert_eq!(
        executor.execute(r).await.status,
        OperationResultStatus::Skipped
    );
    assert_eq!(rows(&executor)[0].revision, 1);
}
#[tokio::test]
async fn parent_revision_is_checked_in_the_child_confirmation_transaction() {
    let s = Sandbox::new();
    let mut executor = setup(&s);
    executor.config.initial_association_authorized = true;
    fs::create_dir_all(executor.config.local_root.join("a")).unwrap();
    let parent = association_request(&executor, "a", "a", "remote-root");
    assert_eq!(
        executor.execute(parent.clone()).await.status,
        OperationResultStatus::Succeeded
    );
    fs::write(executor.config.local_root.join("a/child"), b"child").unwrap();
    let child = association_request(&executor, "a/child", "child", "remote-a");
    let effect = AppliedEffect {
        local: Some(child.local.clone()),
        remote: Some(child.remote.clone()),
        remote_id: child.remote.id.clone(),
        verified: true,
    };
    let (baseline, dependency) = executor.convergence(&effect).unwrap();
    assert_eq!(
        executor
            .write(&parent, Some(1), PersistedEffect::ConflictUpdated(None))
            .status,
        WriteStatus::Written
    );
    let receipt = executor.write(
        &child,
        Some(0),
        PersistedEffect::ConvergenceConfirmed {
            baseline,
            effect: Box::new(effect),
            parent: dependency,
        },
    );
    assert_eq!(receipt.status, WriteStatus::RevisionConflict);
    assert_eq!(rows(&executor).len(), 1);
}

#[test]
fn tree_movement_collisions_hardlinks_and_replaced_destinations_preserve_sources() {
    for scenario in ["collision", "hardlink", "parent"] {
        let s = Sandbox::new();
        let executor = setup(&s);
        fs::write(executor.config.local_root.join("file"), b"source").unwrap();
        fs::create_dir(executor.config.local_root.join("destination")).unwrap();
        let r = request(&executor, OperationAction::MoveLocal);
        let record =
            super::recovery::record(&executor.config, &r, &executor.filesystem, None).unwrap();
        let journal = Journal::new(executor.inventory.clone(), r.clone(), 0, record);
        if scenario == "collision" {
            fs::write(
                executor.config.local_root.join("destination/file"),
                b"occupied",
            )
            .unwrap();
        }
        if scenario == "hardlink" {
            fs::hard_link(
                executor.config.local_root.join("file"),
                s.0.join("outside-alias"),
            )
            .unwrap();
        }
        if scenario == "parent" {
            fs::rename(
                executor.config.local_root.join("destination"),
                executor.config.local_root.join("old-destination"),
            )
            .unwrap();
            fs::create_dir(executor.config.local_root.join("destination")).unwrap();
        }
        assert!(
            executor.filesystem.execute(&r, Some(&journal)).is_err(),
            "{scenario}"
        );
        assert_eq!(
            fs::read(executor.config.local_root.join("file")).unwrap(),
            b"source"
        );
        if scenario == "collision" {
            assert_eq!(
                fs::read(executor.config.local_root.join("destination/file")).unwrap(),
                b"occupied"
            );
        }
    }
}

#[tokio::test]
async fn workspace_content_writes_and_remote_shortcuts_are_refused_before_dispatch() {
    for action in [
        OperationAction::Upload,
        OperationAction::UpdateRemoteContent,
        OperationAction::Download,
        OperationAction::RenameRemote,
        OperationAction::TrashRemote,
    ] {
        let s = Sandbox::new();
        let executor = setup(&s);
        let mut r = request(&executor, action);
        r.remote.mime_type = Evidence::Known("application/vnd.google-apps.document".into());
        assert_eq!(
            executor.execute(r).await.status,
            OperationResultStatus::Skipped
        );
        assert!(rows(&executor).is_empty());
    }
    let s = Sandbox::new();
    let executor = setup(&s);
    let mut r = request(&executor, OperationAction::CreateLocalShortcut);
    r.remote.mime_type = Evidence::Known("application/vnd.google-apps.shortcut".into());
    assert_eq!(
        executor.execute(r).await.status,
        OperationResultStatus::Skipped
    );
    assert!(rows(&executor).is_empty());
}
