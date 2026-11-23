//! Backend contracts use presentation values exclusively.
use super::models::*;
use std::{cell::RefCell, collections::VecDeque, future::Future, pin::Pin, rc::Rc};
pub type NextSnapshot<'a> = Pin<Box<dyn Future<Output = Option<Snapshot>> + 'a>>;
pub trait SyncBackend {
    fn next(&mut self) -> NextSnapshot<'_>;
}
/// Deterministic fixture source; never opens an Engine or a network connection.
pub struct FakeSyncBackend {
    pub snapshots: Rc<RefCell<VecDeque<Snapshot>>>,
}
impl FakeSyncBackend {
    pub fn new(values: impl IntoIterator<Item = Snapshot>) -> Self {
        Self {
            snapshots: Rc::new(RefCell::new(values.into_iter().collect())),
        }
    }
}
impl SyncBackend for FakeSyncBackend {
    fn next(&mut self) -> NextSnapshot<'_> {
        Box::pin(async { self.snapshots.borrow_mut().pop_front() })
    }
}

/// Maps the coalescing public API; transfer bytes and operation IDs are unavailable.
pub struct EngineAdapter {
    engine: drivesync_engine::Engine,
    account: drivesync_engine::AccountId,
    subscription: Option<drivesync_engine::SynchronizationSubscription>,
}
impl EngineAdapter {
    pub fn new(engine: drivesync_engine::Engine, account: drivesync_engine::AccountId) -> Self {
        Self {
            engine,
            account,
            subscription: None,
        }
    }
}
impl SyncBackend for EngineAdapter {
    fn next(&mut self) -> NextSnapshot<'_> {
        Box::pin(async move {
            if let Some(subscription) = &mut self.subscription {
                subscription.changed().await?;
            } else {
                self.subscription = self
                    .engine
                    .subscribe_synchronization(self.account.clone())
                    .await
                    .ok();
            }
            let subscription = self.subscription.as_mut()?;
            let status = subscription.current();
            // Audit is horizon-bound. Re-read after the await so a newer event invalidates it.
            let audited = if status.state == drivesync_engine::SynchronizationState::Idle
                && status.issue.is_none()
            {
                self.engine
                    .synchronization_audit(self.account.clone())
                    .await
                    .ok()
                    .is_some_and(|a| {
                        a.roots.account_id == self.account
                            && a.initial_complete
                            && a.coverage_complete
                            && a.unconfirmed_operations == 0
                            && a.unresolved_conflicts == 0
                    })
            } else {
                false
            };
            let latest = subscription.current();
            Some(map_status(&latest, audited && latest == status))
        })
    }
}
pub fn map_status(status: &drivesync_engine::SynchronizationStatus, audited: bool) -> Snapshot {
    use drivesync_engine::{
        SynchronizationActivity as A, SynchronizationIssueKind as I, SynchronizationState as S,
    };
    let scope = status.account_id.0.clone();
    let state = if status.issue.is_some() {
        StatusView::Attention
    } else {
        match status.state {
            S::Stopped => StatusView::Stopped,
            S::Failed => StatusView::Failed,
            S::Running if status.activity == A::Scanning => StatusView::Scanning,
            S::Running => StatusView::Synchronizing,
            S::Idle if audited => StatusView::Synced,
            _ => StatusView::Initializing,
        }
    };
    let activity = if status.state == S::Running {
        status.current_item.as_ref().and_then(|item| {
            let (kind, direction) = match status.activity {
                A::Downloading => (ActivityKind::Download, Direction::RemoteToLocal),
                A::Uploading => (ActivityKind::Upload, Direction::LocalToRemote),
                A::Applying => (ActivityKind::Apply, Direction::None),
                _ => return None,
            };
            let item_id = item
                .item_id
                .as_ref()
                .map(|v| v.0.clone())
                .or_else(|| item.remote_id.as_ref().map(|v| v.0.clone()))?;
            let name = item
                .local_path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| gettextrs::gettext("Unidentified item"));
            Some(ActivityView {
                scope: scope.clone(),
                operation_id: None,
                item_id,
                name,
                kind,
                direction,
                phase: Phase::Running,
                progress: ProgressView::Indeterminate,
            })
        })
    } else {
        None
    };
    let description = crate::services::synchronization::description(status);
    let attention = status
        .issue
        .as_ref()
        .map(|issue| AttentionItemView {
            scope: scope.clone(),
            id: "current-blocker".into(),
            title: gettextrs::gettext("Synchronization needs attention"),
            summary: description.clone(),
            action: if issue.kind == I::Conflict {
                AttentionAction::OpenConflictList
            } else {
                AttentionAction::OpenIssue("current-blocker".into())
            },
        })
        .into_iter()
        .collect();
    Snapshot {
        scope,
        status: state,
        description,
        activity,
        attention,
    }
}
