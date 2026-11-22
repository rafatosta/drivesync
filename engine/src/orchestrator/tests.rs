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
    fn recover(&self) -> operations::RecoveryFuture<'_> {
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
                    remote_id: r.identity.remote_id,
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
            config,
            operator,
            status,
            stop: rx,
            blockers: BTreeMap::new(),
            sources: BTreeMap::new(),
            recovery_failed: false,
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
    assert_eq!(
        calls[0].operation.action,
        OperationAction::UpdateRemoteContent
    );
    assert_eq!(calls[1].operation.action, OperationAction::RenameRemote);
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
        SynchronizationIssueKind::Conflict
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
        fn recover(&self) -> operations::RecoveryFuture<'_> {
            self.inner.recover()
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
async fn refreshed_same_item_identity_clears_a_blocker_after_a_path_change() {
    let (operator, _) = mock(OperationResultStatus::Unconfirmed);
    let (mut worker, _stop) = direct(operator);
    worker.process(changed_file()).await;
    let mut refreshed = fixture();
    refreshed.local.path = Some("moved/file.txt".into());
    assert_eq!(
        worker.process(refreshed).await.decision,
        DecisionStatus::NoOp
    );
    assert!(worker.status.borrow().issue.is_none());
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
        fn recover<'a>(
            &'a self,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<
                            Vec<operations::models::RecoveryResult>,
                            operations::models::OperatorError,
                        >,
                    > + Send
                    + 'a,
            >,
        > {
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
