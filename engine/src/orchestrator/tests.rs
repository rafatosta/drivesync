//! Coordination tests use the real pure Analyzer and controlled Operator receipts.
use super::analyzer::tests::fixture;
use super::*;
use crate::{
    inventory::models::InventoryWriteResult,
    models::{Checksum, ProviderId, SyncRootId},
};
use operations::{
    models::{AppliedEffect, OperationFailure, OperationResult},
    OperationFuture,
};
use std::sync::Mutex;
use tokio::sync::Notify;
#[derive(Default)]
struct Calls {
    requests: Mutex<Vec<ExecuteOperationRequest>>,
}
struct MockOperator {
    calls: Arc<Calls>,
    outcome: OperationResultStatus,
    pause: Option<Arc<Pause>>,
    panic: bool,
}
#[derive(Default)]
struct Pause {
    started: Notify,
    release: Notify,
}
impl Operator for MockOperator {
    fn recover_with<'a>(
        &'a self,
        _: operations::RecoveryObserver<'a>,
    ) -> operations::RecoveryFuture<'a> {
        Box::pin(async { Ok(Vec::new()) })
    }
    fn execute<'a>(&'a self, r: ExecuteOperationRequest) -> OperationFuture<'a> {
        Box::pin(async move {
            self.calls.requests.lock().unwrap().push(r.clone());
            if self.panic {
                panic!("isolated executor panic");
            }
            if let Some(pause) = &self.pause {
                pause.started.notify_one();
                pause.release.notified().await;
            }
            let mut remote = r.remote.clone();
            let mut local = r.local.clone();
            if let Evidence::Known(version) = &mut remote.version {
                *version += 1;
            }
            if r.operation.action == OperationAction::Upload {
                remote.state = crate::models::FileState::Present;
                remote.id = Some(crate::models::RemoteId(format!(
                    "created-{}",
                    r.operation.item_ref.0
                )));
                remote.version = Evidence::Known(1);
            }
            let remote_id = remote.id.clone();
            match r.operation.action {
                OperationAction::UpdateRemoteContent => {
                    remote.checksum = local.checksum.clone();
                    remote.size = local.size.clone();
                }
                OperationAction::RenameRemote => {
                    remote.name = Evidence::Known(r.operation.name.clone().unwrap())
                }
                OperationAction::RenameLocal => {
                    local.name = Evidence::Known(r.operation.name.clone().unwrap());
                    local.path = Some(
                        local
                            .path
                            .unwrap()
                            .with_file_name(r.operation.name.as_ref().unwrap()),
                    );
                }
                OperationAction::MoveLocal => {
                    local.parent = Evidence::Known(r.operation.parent.clone().unwrap());
                    local.path = Some(
                        r.operation
                            .parent
                            .as_ref()
                            .unwrap()
                            .join(local.path.as_ref().unwrap().file_name().unwrap()),
                    );
                }
                _ => {}
            }
            OperationResult {
                operation_id: r.execution_id,
                item_id: r.operation.item_ref,
                action: r.operation.action,
                status: self.outcome,
                effect: Some(AppliedEffect {
                    local: Some(local),
                    remote: Some(remote),
                    remote_id,
                    verified: self.outcome == OperationResultStatus::Succeeded,
                }),
                completed_at: Some(timestamp()),
                error: if self.outcome == OperationResultStatus::Succeeded {
                    None
                } else {
                    Some(OperationFailure {
                        code: "Test".into(),
                        retryable: false,
                        message: "Controlled failure".into(),
                    })
                },
                inventory_write: Some(InventoryWriteResult {
                    status: WriteStatus::Written,
                    revision: Some(r.expected_revision.unwrap() + 3),
                    recorded_at: timestamp(),
                }),
            }
        })
    }
}
fn config() -> OrchestratorConfig {
    let f = fixture();
    OrchestratorConfig {
        expected_root_identity: None,
        trash_propagation_authorized: false,
        initial_association_authorized: false,
        roots: StartSynchronizationRequest {
            account_id: f.identity.account_id,
            local_root: f.environment.local_root.0,
            remote_root_id: f.environment.remote_root_id,
        },
        sync_root_id: SyncRootId("root-a".into()),
        provider_id: ProviderId("google".into()),
    }
}
fn mock(outcome: OperationResultStatus) -> (Box<dyn Operator>, Arc<Calls>) {
    let calls = Arc::new(Calls::default());
    (
        Box::new(MockOperator {
            calls: calls.clone(),
            outcome,
            pause: None,
            panic: false,
        }),
        calls,
    )
}
fn direct(operator: Box<dyn Operator>) -> (Orchestrator, watch::Sender<bool>) {
    let config = config();
    let (status, _) = watch::channel(SynchronizationStatus::new(
        config.roots.account_id.clone(),
        SynchronizationState::Waiting,
    ));
    let (stop, rx) = watch::channel(false);
    (
        Orchestrator {
            _writer: None,
            capture_epoch: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            local_refresh: Arc::new(Notify::new()),
            remote_refresh: Arc::new(Notify::new()),
            config,
            operator,
            status,
            stop: rx,
            blockers: BTreeMap::new(),
            pending: BTreeMap::new(),
            sources: BTreeMap::new(),
            scanning_sources: BTreeSet::new(),
            recovery_failed: false,
            complete_captures: BTreeMap::new(),
            refresh_after: None,
            refresh_wait: None,
            affected: Default::default(),
            unchanged: BTreeMap::new(),
        },
        stop,
    )
}
fn changed_file() -> File {
    let mut f = fixture();
    f.local.checksum = Evidence::Known(Checksum {
        algorithm: "sha256".into(),
        value: "new-content".into(),
    });
    f.local.name = Evidence::Known("renamed.txt".into());
    f
}
#[tokio::test]
async fn executes_in_order_and_refreshes_verified_versions_and_revisions() {
    let (operator, calls) = mock(OperationResultStatus::Succeeded);
    let (mut worker, _stop) = direct(operator);
    let outcome = worker.process(changed_file()).await;
    assert_eq!(outcome.decision, DecisionStatus::Ready);
    assert_eq!(outcome.results.len(), 2);
    let calls = calls.requests.lock().unwrap();
    assert_eq!(calls[0].operation.action, OperationAction::RenameRemote);
    assert_eq!(
        calls[1].operation.action,
        OperationAction::UpdateRemoteContent
    );
    assert_eq!(calls[1].operation.depends_on, vec![1]);
    assert_eq!(calls[1].completed_steps, vec![1]);
    assert_eq!(calls[1].expected_revision, Some(7));
    assert_eq!(
        calls[1]
            .operation
            .preconditions
            .as_ref()
            .unwrap()
            .observed_remote_version,
        Some(9)
    );
    assert_ne!(calls[0].execution_id, calls[1].execution_id);
    assert_eq!(worker.status.borrow().state, SynchronizationState::Idle);
}
#[tokio::test]
async fn blocked_decisions_do_not_execute_or_create_baselines() {
    let (operator, calls) = mock(OperationResultStatus::Succeeded);
    let (mut worker, _stop) = direct(operator);
    assert_eq!(
        worker.process(fixture()).await.decision,
        DecisionStatus::NoOp
    );
    let mut incomplete = fixture();
    incomplete.remote.checksum = Evidence::NotCollected;
    assert_eq!(
        worker.process(incomplete).await.decision,
        DecisionStatus::Defer
    );
    let mut conflict = changed_file();
    conflict.remote.checksum = Evidence::Known(Checksum {
        algorithm: "sha256".into(),
        value: "different".into(),
    });
    assert_eq!(
        worker.process(conflict).await.decision,
        DecisionStatus::Conflict
    );
    assert!(calls.requests.lock().unwrap().is_empty());
    assert_eq!(
        worker.status.borrow().issue.as_ref().unwrap().kind,
        SynchronizationIssueKind::OperationFailed
    );
}
#[tokio::test]
async fn failures_and_uncertain_effects_block_remaining_plan_steps() {
    for status in [
        OperationResultStatus::Failed,
        OperationResultStatus::Unconfirmed,
        OperationResultStatus::Skipped,
    ] {
        let (operator, calls) = mock(status);
        let (mut worker, _stop) = direct(operator);
        let outcome = worker.process(changed_file()).await;
        assert_eq!(outcome.results.len(), 1);
        assert_eq!(calls.requests.lock().unwrap().len(), 1);
        assert_eq!(
            worker.status.borrow().issue.as_ref().unwrap().kind,
            match status {
                OperationResultStatus::Failed => SynchronizationIssueKind::OperationFailed,
                OperationResultStatus::Unconfirmed => SynchronizationIssueKind::RecoveryRequired,
                _ => SynchronizationIssueKind::PreconditionFailed,
            }
        );
    }
}
#[tokio::test]
async fn scope_mismatch_never_reaches_the_operator() {
    let (operator, calls) = mock(OperationResultStatus::Succeeded);
    let (mut worker, _stop) = direct(operator);
    let mut f = changed_file();
    f.identity.account_id.0 = "other".into();
    assert_eq!(
        worker.process(f).await.decision,
        DecisionStatus::InvalidInput
    );
    assert!(calls.requests.lock().unwrap().is_empty());
    assert_eq!(worker.status.borrow().state, SynchronizationState::Failed);
}
#[tokio::test]
async fn another_items_noop_does_not_hide_an_unresolved_blocker() {
    let (operator, _) = mock(OperationResultStatus::Unconfirmed);
    let (mut worker, _stop) = direct(operator);
    worker.process(changed_file()).await;
    let mut another = fixture();
    another.identity.item_id = Some(crate::models::ItemId("another".into()));
    worker.process(another).await;
    assert_eq!(
        worker.status.borrow().issue.as_ref().unwrap().kind,
        SynchronizationIssueKind::RecoveryRequired
    );
}
#[tokio::test]
async fn stop_finishes_the_current_operation_but_never_starts_its_dependent_step() {
    let calls = Arc::new(Calls::default());
    let pause = Arc::new(Pause::default());
    let handle = start(
        config(),
        Box::new(MockOperator {
            calls: calls.clone(),
            outcome: OperationResultStatus::Succeeded,
            pause: Some(pause.clone()),
            panic: false,
        }),
        Arc::new(RwLock::new(())),
        2,
    );
    handle.submit(changed_file()).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), pause.started.notified())
        .await
        .unwrap();
    let stop = handle.stop();
    tokio::pin!(stop);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(20), &mut stop)
            .await
            .is_err()
    );
    pause.release.notify_one();
    let status = stop.await;
    assert_eq!(status.state, SynchronizationState::Stopped);
    assert_eq!(calls.requests.lock().unwrap().len(), 1);
    assert_eq!(
        handle.submit(fixture()).await.unwrap_err(),
        SynchronizationErrorKind::Stopped
    );
}
#[tokio::test]
async fn worker_panic_publishes_failure_and_releases_waiters() {
    let handle = start(
        config(),
        Box::new(MockOperator {
            calls: Arc::new(Calls::default()),
            outcome: OperationResultStatus::Succeeded,
            pause: None,
            panic: true,
        }),
        Arc::new(RwLock::new(())),
        1,
    );
    let mut subscription = handle.subscribe();
    handle.submit(changed_file()).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if subscription.current().state == SynchronizationState::Failed {
                break;
            }
            if subscription.changed().await.is_none() {
                break;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(
        subscription.current().issue.unwrap().kind,
        SynchronizationIssueKind::WorkerFailed
    );
    assert_eq!(handle.stop().await.state, SynchronizationState::Failed);
}

#[tokio::test]
async fn successful_receipt_without_persistence_or_with_a_crossed_identity_cannot_advance() {
    struct InvalidReceipt {
        inner: MockOperator,
        crossed: bool,
    }
    impl Operator for InvalidReceipt {
        fn recover_with<'a>(
            &'a self,
            checking: operations::RecoveryObserver<'a>,
        ) -> operations::RecoveryFuture<'a> {
            self.inner.recover_with(checking)
        }
        fn execute<'a>(&'a self, r: ExecuteOperationRequest) -> OperationFuture<'a> {
            Box::pin(async move {
                let mut result = self.inner.execute(r).await;
                if self.crossed {
                    result.item_id = crate::models::ItemId("other-item".into());
                } else {
                    result.inventory_write.as_mut().unwrap().status = WriteStatus::Unavailable;
                }
                result
            })
        }
    }
    for crossed in [true, false] {
        let calls = Arc::new(Calls::default());
        let (mut worker, _stop) = direct(Box::new(InvalidReceipt {
            inner: MockOperator {
                calls: calls.clone(),
                outcome: OperationResultStatus::Succeeded,
                pause: None,
                panic: false,
            },
            crossed,
        }));
        let result = worker.process(changed_file()).await;
        assert_eq!(result.results.len(), 1);
        assert_eq!(calls.requests.lock().unwrap().len(), 1);
        assert_eq!(
            worker.status.borrow().issue.as_ref().unwrap().kind,
            SynchronizationIssueKind::RecoveryRequired
        );
    }
}
#[tokio::test]
async fn queue_backpressure_is_bounded_and_the_operation_lease_excludes_removal() {
    let calls = Arc::new(Calls::default());
    let pause = Arc::new(Pause::default());
    let ownership = Arc::new(RwLock::new(()));
    let handle = start(
        config(),
        Box::new(MockOperator {
            calls,
            outcome: OperationResultStatus::Succeeded,
            pause: Some(pause.clone()),
            panic: false,
        }),
        ownership.clone(),
        1,
    );
    let mut file = changed_file();
    file.local.name = fixture().local.name;
    handle.submit(file.clone()).await.unwrap();
    pause.started.notified().await;
    assert!(ownership.clone().try_write_owned().is_err());
    handle.submit(file).await.unwrap();
    let third = handle.submit(fixture());
    tokio::pin!(third);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(20), &mut third)
            .await
            .is_err()
    );
    let stopping = handle.stop();
    tokio::pin!(stopping);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(20), &mut stopping)
            .await
            .is_err()
    );
    pause.release.notify_one();
    stopping.await;
    assert_eq!(third.await.unwrap_err(), SynchronizationErrorKind::Stopped);
    assert!(ownership.try_write_owned().is_ok());
}
#[tokio::test]
async fn refreshed_same_item_identity_cannot_clear_uncertainty_after_a_path_change() {
    let (operator, calls) = mock(OperationResultStatus::Unconfirmed);
    let (mut worker, _stop) = direct(operator);
    worker.process(changed_file()).await;
    let mut refreshed = fixture();
    refreshed.local.path = Some("moved/file.txt".into());
    assert_eq!(
        worker.process(refreshed).await.decision,
        DecisionStatus::Defer
    );
    assert_eq!(
        worker.status.borrow().issue.as_ref().unwrap().kind,
        SynchronizationIssueKind::RecoveryRequired
    );
    assert_eq!(calls.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn correlation_to_a_known_item_does_not_leave_a_phantom_anonymous_blocker() {
    let (operator, _) = mock(OperationResultStatus::Succeeded);
    let (mut worker, _stop) = direct(operator);
    let mut untracked = fixture();
    untracked.identity.item_id = None;
    untracked.inventory_lookup = crate::inventory::models::InventoryLookupStatus::NotFound;
    untracked.inventory_revision = None;
    untracked.baseline = crate::inventory::models::Baseline::never_synced();
    assert_eq!(
        worker.process(untracked).await.decision,
        DecisionStatus::Defer
    );
    assert_eq!(
        worker.process(fixture()).await.decision,
        DecisionStatus::NoOp
    );
    assert!(worker.blockers.is_empty());
}

#[tokio::test]
async fn incomplete_sources_suspend_dispatch_until_fresh_complete_observations() {
    let (operator, calls) = mock(OperationResultStatus::Succeeded);
    let (mut worker, _stop) = direct(operator);
    worker.sources.insert(Source::Local, None);
    worker.sources.insert(
        Source::Remote,
        Some(issue(SynchronizationIssueKind::Deferred, true)),
    );
    assert_eq!(
        worker.process(changed_file()).await.decision,
        DecisionStatus::Ready
    );
    assert!(calls.requests.lock().unwrap().is_empty());
    assert_eq!(worker.status.borrow().state, SynchronizationState::Waiting);
    worker.sources.insert(Source::Remote, None);
    assert_eq!(worker.process(changed_file()).await.results.len(), 2);
    assert_eq!(calls.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn unavailable_recovery_inventory_blocks_dispatch_before_any_analysis_plan_executes() {
    struct Unavailable;
    impl Operator for Unavailable {
        fn recover_with<'a>(
            &'a self,
            _: operations::RecoveryObserver<'a>,
        ) -> operations::RecoveryFuture<'a> {
            Box::pin(async {
                Err(operations::models::OperatorError {
                    code: OperatorErrorKind::Unavailable,
                    retryable: false,
                    effect: operations::models::ExecutionEffect::NotApplied,
                })
            })
        }
        fn execute<'a>(&'a self, _: ExecuteOperationRequest) -> OperationFuture<'a> {
            Box::pin(async { panic!("unreadable recovery state must prevent dispatch") })
        }
    }
    let handle = start(
        config(),
        Box::new(Unavailable),
        Arc::new(RwLock::new(())),
        2,
    );
    let mut subscription = handle.subscribe();
    handle.submit(changed_file()).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if subscription
                .current()
                .issue
                .as_ref()
                .is_some_and(|issue| issue.kind == SynchronizationIssueKind::RecoveryRequired)
            {
                break;
            }
            subscription.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert_eq!(handle.stop().await.state, SynchronizationState::Stopped);
}

#[tokio::test]
async fn committed_effects_require_new_reads_of_both_sides_before_reusing_input() {
    let (operator, calls) = mock(OperationResultStatus::Succeeded);
    let (mut worker, _stop) = direct(operator);
    let mut file = changed_file();
    file.observations.local_generation = Some(10);
    file.observations.remote_generation = Some(20);
    file.observations.local_epoch = Some(0);
    file.observations.remote_epoch = Some(0);
    assert!(!worker.process(file.clone()).await.results.is_empty());
    let count = calls.requests.lock().unwrap().len();
    assert_eq!(
        worker.process(file.clone()).await.decision,
        DecisionStatus::Defer
    );
    file.observations.local_generation = Some(11);
    file.observations.local_epoch = Some(
        worker
            .capture_epoch
            .load(std::sync::atomic::Ordering::SeqCst),
    );
    assert_eq!(
        worker.process(file.clone()).await.decision,
        DecisionStatus::Defer
    );
    assert_eq!(calls.requests.lock().unwrap().len(), count);
    file.observations.remote_generation = Some(21);
    file.observations.remote_epoch = file.observations.local_epoch;
    for (local, remote) in [
        (ObservationCoverage::Complete, ObservationCoverage::Partial),
        (ObservationCoverage::Partial, ObservationCoverage::Complete),
    ] {
        file.observations.local_coverage = local;
        file.observations.remote_coverage = remote;
        assert_eq!(
            worker.process(file.clone()).await.decision,
            DecisionStatus::Defer
        );
        assert_eq!(calls.requests.lock().unwrap().len(), count);
        assert!(worker.refresh_after.is_some());
    }
    file.observations.local_coverage = ObservationCoverage::Complete;
    file.observations.remote_coverage = ObservationCoverage::Complete;
    assert!(!worker.process(file).await.results.is_empty());
}

#[tokio::test]
async fn source_recovery_reopens_admission_and_accounts_keep_independent_barriers() {
    let (operator, _) = mock(OperationResultStatus::Succeeded);
    let (mut a, _stop) = direct(operator);
    a.recovery_failed = true;
    a.recover_pending().await;
    assert!(!a.recovery_failed);
    let mut first = changed_file();
    first.observations.local_generation = Some(1);
    first.observations.remote_generation = Some(1);
    first.observations.local_epoch = Some(0);
    first.observations.remote_epoch = Some(0);
    assert!(!a.process(first.clone()).await.results.is_empty());
    let (operator, _) = mock(OperationResultStatus::Succeeded);
    let (mut b, _stop_b) = direct(operator);
    b.config.roots.account_id = crate::models::AccountId("account-b".into());
    first.identity.account_id = b.config.roots.account_id.clone();
    assert!(!b.process(first).await.results.is_empty());
    assert!(a.refresh_after.is_some());
    assert!(b.refresh_after.is_some());
}

#[tokio::test]
async fn unchanged_eligible_input_is_processed_when_source_availability_returns() {
    let (operator, calls) = mock(OperationResultStatus::Succeeded);
    let (mut worker, _stop) = direct(operator);
    worker.sources.insert(Source::Local, None);
    worker.sources.insert(
        Source::Remote,
        Some(issue(SynchronizationIssueKind::Deferred, true)),
    );
    let file = changed_file();
    assert!(worker.process(file.clone()).await.results.is_empty());
    assert!(calls.requests.lock().unwrap().is_empty());
    worker.sources.insert(Source::Remote, None);
    assert!(!worker.process(file).await.results.is_empty());
    assert!(!calls.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn trash_refusal_requires_an_operation_dispatched_to_the_executor() {
    use crate::inventory::models::ProjectionKind;
    use crate::models::FileState;

    let (operator, calls) = mock(OperationResultStatus::Failed);
    let (mut worker, _stop) = direct(operator);
    let mut file = fixture();
    file.baseline.projection = Some(ProjectionKind::Native);
    file.environment.destination_local_parent = file.local.parent.clone();
    file.environment.destination_remote_parent_id =
        Evidence::Known(crate::models::RemoteId("parent-a".into()));
    file.environment.trash_propagation_authorized = true;
    file.remote.trashed = Evidence::Known(false);
    worker.config.trash_propagation_authorized = true;

    for state in [
        FileState::OutsideScope,
        FileState::Unknown,
        FileState::Unavailable,
        FileState::Absent,
    ] {
        file.remote.state = state;
        worker.process(file.clone()).await;
        assert!(matches!(
            worker.status.borrow().issue.as_ref().unwrap().kind,
            SynchronizationIssueKind::Deferred | SynchronizationIssueKind::DeferredFor(_)
        ));
    }
    worker.config.trash_propagation_authorized = false;
    file.remote.trashed = Evidence::Known(true);
    file.remote.version = Evidence::Known(9);
    worker.process(file.clone()).await;
    assert!(matches!(
        worker.status.borrow().issue.as_ref().unwrap().kind,
        SynchronizationIssueKind::Deferred | SynchronizationIssueKind::DeferredFor(_)
    ));
    assert!(calls.requests.lock().unwrap().is_empty());

    worker.config.trash_propagation_authorized = true;
    worker.process(file).await;
    assert_eq!(
        calls.requests.lock().unwrap()[0].operation.action,
        OperationAction::RemoveLocal
    );
    assert_eq!(
        worker.status.borrow().issue.as_ref().unwrap().kind,
        SynchronizationIssueKind::TrashRefused
    );
}

#[tokio::test]
async fn a_confirmed_directory_waits_for_metadata_and_resumes_without_another_operation() {
    use crate::{
        inventory::models::{OperationState, PendingOperation},
        models::FileKind,
    };

    let (operator, calls) = mock(OperationResultStatus::Succeeded);
    let (mut worker, _stop) = direct(operator);
    let mut file = fixture();
    file.local.path = Some("UEFS".into());
    file.local.name = Evidence::Known("UEFS".into());
    file.local.parent = Evidence::Known(PathBuf::new());
    file.local.kind = Evidence::Known(FileKind::Directory);
    file.local.checksum = Evidence::NotApplicable;
    file.local.size = Evidence::NotApplicable;
    file.remote.name = Evidence::Known("UEFS".into());
    file.remote.parents = Evidence::Known(vec![file.environment.remote_root_id.clone()]);
    file.remote.mime_type = Evidence::Known("application/vnd.google-apps.folder".into());
    file.remote.checksum = Evidence::NotApplicable;
    file.remote.size = Evidence::NotApplicable;
    file.baseline.local = Some(file.local.clone());
    file.baseline.remote = Some(file.remote.clone());

    file.pending_operations.push(PendingOperation {
        id: Id("confirmed-folder".into()),
        item_id: file.identity.item_id.clone().unwrap(),
        action: OperationAction::CreateLocalDirectory,
        state: OperationState::Confirmed,
        remote_id: file.remote.id.clone(),
        applied_effect: None,
        idempotency_key: None,
        recovery: None,
        recovery_diagnostic: None,
        created_at: timestamp(),
        updated_at: timestamp(),
    });
    file.remote.trashed = Evidence::NotCollected;
    assert_eq!(
        worker.process(file.clone()).await.decision,
        DecisionStatus::Defer
    );
    {
        let status = worker.status.borrow();
        assert_eq!(
            status
                .current_item
                .as_ref()
                .unwrap()
                .display_name
                .as_deref(),
            Some("UEFS")
        );
        assert_eq!(
            status.issue.as_ref().unwrap().kind,
            SynchronizationIssueKind::DeferredFor(SynchronizationWait::ItemVerification {
                source: SynchronizationVerificationSource::Remote,
                confirmed_action: Some(SynchronizationConfirmedAction::CreateLocalDirectory),
            }),
        );
    }

    file.remote.trashed = Evidence::Known(false);
    assert_eq!(worker.process(file).await.decision, DecisionStatus::NoOp);
    assert!(worker.status.borrow().issue.is_none());
    assert!(calls.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn independent_uploads_share_a_capture_and_reject_stale_replay() {
    let (operator, calls) = mock(OperationResultStatus::Succeeded);
    let (mut worker, _stop) = direct(operator);
    let mut first = super::analyzer::tests::initial_local(false, true);
    first.observations.local_epoch = Some(0);
    first.observations.remote_epoch = Some(0);

    let mut second = first.clone();
    second.identity.item_id = Some(crate::models::ItemId("another-item".into()));
    second.local.name = Evidence::Known("second.txt".into());
    second.local.path = Some("folder/second.txt".into());
    second.remote.name = second.local.name.clone();
    if let Evidence::Known(identity) = &mut second.identity.local_identity {
        identity.inode += 1;
    }
    second.local.identity = second.identity.local_identity.clone();

    for file in [first.clone(), second] {
        let result = worker.process(file).await;
        assert_eq!(result.results.len(), 1, "{result:?}");
        assert_eq!(result.results[0].action, OperationAction::Upload);
        assert_eq!(result.results[0].status, OperationResultStatus::Succeeded);
    }
    assert_eq!(worker.process(first).await.decision, DecisionStatus::Defer);
    assert_eq!(calls.requests.lock().unwrap().len(), 2);
    assert!(worker.refresh_after.is_none());
}

#[tokio::test]
async fn a_noop_is_reused_only_for_the_same_decision_evidence() {
    let (operator, calls) = mock(OperationResultStatus::Succeeded);
    let (mut worker, _stop) = direct(operator);
    let mut file = fixture();
    file.observations.local_epoch = Some(0);
    file.observations.remote_epoch = Some(0);
    assert_eq!(
        worker.process(file.clone()).await.decision,
        DecisionStatus::NoOp
    );
    assert_eq!(worker.unchanged.len(), 1);

    file.observations.local_generation = Some(40);
    assert_eq!(
        worker.process(file.clone()).await.decision,
        DecisionStatus::NoOp
    );
    assert!(calls.requests.lock().unwrap().is_empty());

    file = changed_file();
    file.observations.local_epoch = Some(0);
    file.observations.remote_epoch = Some(0);
    assert_eq!(worker.process(file).await.decision, DecisionStatus::Ready);
    assert!(!calls.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn finishing_the_local_scan_preserves_the_active_drive_scan() {
    let (operator, _) = mock(OperationResultStatus::Succeeded);
    let (mut worker, _stop) = direct(operator);
    worker.sources.insert(Source::Local, None);
    worker.sources.insert(
        Source::Remote,
        Some(issue(
            SynchronizationIssueKind::DeferredFor(SynchronizationWait::RemoteObservation),
            true,
        )),
    );
    worker.scanning_sources.insert(Source::Remote);
    worker.settle();
    assert_eq!(
        worker.status.borrow().activity,
        SynchronizationActivity::ScanningRemote
    );
    assert_eq!(worker.status.borrow().state, SynchronizationState::Running);
    worker.scanning_sources.remove(&Source::Remote);
    worker.settle();
    assert_eq!(worker.status.borrow().state, SynchronizationState::Waiting);
    assert_eq!(
        worker.status.borrow().issue.as_ref().unwrap().kind,
        SynchronizationIssueKind::DeferredFor(SynchronizationWait::RemoteObservation)
    );
}

#[tokio::test]
async fn post_operation_verification_requires_a_fresh_complete_capture_from_each_side() {
    let (operator, _) = mock(OperationResultStatus::Succeeded);
    let (mut worker, _stop) = direct(operator);
    let mut subscription = SynchronizationSubscription {
        receiver: worker.status.subscribe(),
    };
    worker.wait_for_refresh(2);
    worker.publish(
        SynchronizationState::Waiting,
        SynchronizationActivity::None,
        None,
        None,
        Some(issue(
            SynchronizationIssueKind::DeferredFor(SynchronizationWait::FreshObservations),
            true,
        )),
    );
    let expected = |local_read, remote_read| {
        Some(SynchronizationVerification {
            local_read,
            remote_read,
        })
    };
    assert_eq!(subscription.current().verification, expected(false, false));

    worker.record_capture(Source::Remote, Some(1));
    assert!(!subscription.receiver.has_changed().unwrap());
    worker.record_capture(Source::Local, Some(2));
    assert!(subscription.receiver.has_changed().unwrap());
    assert_eq!(subscription.current().verification, expected(true, false));

    worker.record_capture(Source::Remote, Some(2));
    assert_eq!(subscription.current().verification, expected(true, true));
    worker.record_capture(Source::Local, None);
    assert_eq!(subscription.current().verification, expected(false, true));

    worker.wait_for_refresh(3);
    assert_eq!(worker.verification(), expected(false, false).unwrap());
    worker.publish(
        SynchronizationState::Idle,
        SynchronizationActivity::None,
        None,
        None,
        None,
    );
    assert_eq!(subscription.current().verification, None);
}

#[tokio::test]
async fn repeated_item_wait_keeps_its_age_and_exposes_missing_attributes() {
    let (operator, _) = mock(OperationResultStatus::Succeeded);
    let (mut worker, _stop) = direct(operator);
    let mut file = fixture();
    file.remote.trashed = Evidence::NotCollected;
    assert_eq!(
        worker.process(file.clone()).await.decision,
        DecisionStatus::Defer
    );
    let key = key(&worker.status.borrow().pending.as_ref().unwrap().item);
    worker.pending.get_mut(&key).unwrap().since = DateTime { unix_millis: 123 };
    worker.process(file.clone()).await;
    let status = worker.status.borrow().clone();
    let pending = status.pending.unwrap();
    assert_eq!(pending.since.unix_millis, 123);
    assert!(pending.missing.contains(&SynchronizationMissingEvidence {
        source: SynchronizationVerificationSource::Remote,
        attribute: SynchronizationAttribute::Trashed,
    }));
    assert!(status.work.analysis.is_some());
    // A source update changes current work, not the identity or age of the wait.
    worker.scanning_sources.insert(Source::Remote);
    worker.settle();
    assert_eq!(
        worker
            .status
            .borrow()
            .pending
            .as_ref()
            .unwrap()
            .since
            .unix_millis,
        123
    );
    file.remote.trashed = Evidence::Known(false);
    assert_eq!(worker.process(file).await.decision, DecisionStatus::NoOp);
    assert!(worker.status.borrow().pending.is_none());
}

#[tokio::test]
async fn detector_telemetry_bypasses_a_busy_operator_without_granting_coverage() {
    let calls = Arc::new(Calls::default());
    let pause = Arc::new(Pause::default());
    let handle = start(
        config(),
        Box::new(MockOperator {
            calls,
            outcome: OperationResultStatus::Succeeded,
            pause: Some(pause.clone()),
            panic: false,
        }),
        Arc::new(RwLock::new(())),
        1,
    );
    handle.submit(changed_file()).await.unwrap();
    pause.started.notified().await;
    let operation = handle.status().current_operation.unwrap().id;
    let mut subscription = handle.subscribe();
    subscription.current();
    handle.scan_progress(Source::Remote, Some("Documents/Teaching".into()), 340, 4);
    let status = subscription.changed().await.unwrap();
    assert_eq!(status.account_id, config().roots.account_id);
    assert_eq!(status.current_operation.unwrap().id, operation);
    assert_eq!(status.work.remote.as_ref().unwrap().items, 340);
    assert_eq!(
        status.work.remote.as_ref().unwrap().path.as_deref(),
        Some(std::path::Path::new("Documents/Teaching"))
    );
    assert!(status.verification.is_none());
    handle.scan_state(
        Source::Remote,
        SynchronizationScanState::RetryScheduled,
        None,
    );
    assert!(subscription
        .changed()
        .await
        .unwrap()
        .work
        .remote
        .unwrap()
        .retry_at
        .is_some());
    pause.release.notify_one();
    let stopped = handle.stop().await;
    assert_eq!(stopped.work, SynchronizationWork::default());
    assert!(stopped.pending.is_none());
    handle.scan_progress(Source::Local, Some("late".into()), 1, 0);
    assert_eq!(handle.status().work, SynchronizationWork::default());
}
