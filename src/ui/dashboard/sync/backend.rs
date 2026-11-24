//! Backend contracts use presentation values exclusively.
use super::models::*;
use gettextrs::gettext;
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

/// Maps the coalescing public API; transfer bytes are unavailable.
pub struct EngineAdapter {
    engine: drivesync_engine::Engine,
    account: drivesync_engine::AccountId,
    subscription: Option<drivesync_engine::SynchronizationSubscription>,
    ended: bool,
}
impl EngineAdapter {
    pub fn new(engine: drivesync_engine::Engine, account: drivesync_engine::AccountId) -> Self {
        Self {
            engine,
            account,
            subscription: None,
            ended: false,
        }
    }
}
impl SyncBackend for EngineAdapter {
    fn next(&mut self) -> NextSnapshot<'_> {
        Box::pin(async move {
            if self.ended {
                return None;
            }
            if let Some(subscription) = &mut self.subscription {
                subscription.changed().await?;
            } else {
                match self
                    .engine
                    .subscribe_synchronization(self.account.clone())
                    .await
                {
                    Ok(subscription) => self.subscription = Some(subscription),
                    Err(error) => {
                        self.ended = true;
                        return Some(Snapshot {
                            scope: self.account.0.clone(),
                            status: StatusView::Failed,
                            description: crate::core::messages::engine_error(&error),
                            activity: None,
                            attention: vec![],
                        });
                    }
                }
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
            let updates_local = status
                .current_operation
                .as_ref()
                .is_some_and(|op| op.updates_local_content);
            let updates_remote = status
                .current_operation
                .as_ref()
                .is_some_and(|op| op.updates_remote_content);
            let rename = status
                .current_operation
                .as_ref()
                .and_then(|op| op.rename.as_ref());
            let movement = status
                .current_operation
                .as_ref()
                .and_then(|op| op.movement.as_ref());
            let directory_creation = status
                .current_operation
                .as_ref()
                .and_then(|op| op.directory_creation);
            let (kind, direction) = match status.activity {
                A::CreatingDirectory | A::Applying if directory_creation.is_some() => {
                    use drivesync_engine::SynchronizationDirectoryDestination;
                    let direction = match directory_creation? {
                        SynchronizationDirectoryDestination::Local => Direction::RemoteToLocal,
                        SynchronizationDirectoryDestination::Remote => Direction::LocalToRemote,
                    };
                    (ActivityKind::CreateDirectory, direction)
                }
                A::Moving | A::Applying if movement.is_some() => {
                    use drivesync_engine::SynchronizationMoveOrigin;
                    let movement = movement?;
                    let display_parent = |path: &std::path::Path| {
                        if path.as_os_str().is_empty() {
                            gettext("Sync root")
                        } else {
                            path.to_string_lossy().into_owned()
                        }
                    };
                    (
                        ActivityKind::Move {
                            source_parent: display_parent(&movement.source_parent),
                            destination_parent: display_parent(&movement.destination_parent),
                        },
                        match movement.origin {
                            SynchronizationMoveOrigin::Local => Direction::LocalToRemote,
                            SynchronizationMoveOrigin::Remote => Direction::RemoteToLocal,
                        },
                    )
                }
                A::Renaming | A::Applying if rename.is_some() => {
                    let rename = rename?;
                    let direction = match rename.origin {
                        drivesync_engine::SynchronizationRenameOrigin::Local => {
                            Direction::LocalToRemote
                        }
                        drivesync_engine::SynchronizationRenameOrigin::Remote => {
                            Direction::RemoteToLocal
                        }
                    };
                    (
                        ActivityKind::Rename {
                            previous_name: rename.previous_name.clone(),
                        },
                        direction,
                    )
                }
                A::Renaming | A::Moving => (ActivityKind::Apply, Direction::None),
                A::Downloading | A::Applying if updates_local => {
                    (ActivityKind::UpdateLocalContent, Direction::RemoteToLocal)
                }
                A::Downloading
                    if status
                        .current_operation
                        .as_ref()
                        .is_some_and(|op| op.newly_observed_remote) =>
                {
                    (
                        ActivityKind::DownloadDiscoveredRemote,
                        Direction::RemoteToLocal,
                    )
                }
                A::Downloading => (ActivityKind::Download, Direction::RemoteToLocal),
                A::Uploading
                    if status
                        .current_operation
                        .as_ref()
                        .is_some_and(|op| op.newly_observed_local) =>
                {
                    (
                        ActivityKind::UploadDiscoveredLocal,
                        Direction::LocalToRemote,
                    )
                }
                A::Uploading | A::Applying if updates_remote => {
                    (ActivityKind::UpdateRemoteContent, Direction::LocalToRemote)
                }
                A::Uploading => (ActivityKind::Upload, Direction::LocalToRemote),
                A::Applying => (ActivityKind::Apply, Direction::None),
                _ => return None,
            };
            let item_id = status
                .current_operation
                .as_ref()
                .map(|op| op.item_id.0.clone())
                .or_else(|| {
                    item.item_id
                        .as_ref()
                        .map(|v| v.0.clone())
                        .or_else(|| item.remote_id.as_ref().map(|v| v.0.clone()))
                })?;
            let name = status
                .current_operation
                .as_ref()
                .and_then(|op| op.display_name.clone())
                .or_else(|| {
                    item.local_path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .map(|p| p.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| gettextrs::gettext("Unidentified item"));
            Some(ActivityView {
                scope: scope.clone(),
                operation_id: status.current_operation.as_ref().map(|op| op.id.clone()),
                item_id,
                name,
                kind,
                direction,
                phase: if (updates_local
                    || updates_remote
                    || rename.is_some()
                    || movement.is_some()
                    || directory_creation.is_some())
                    && status.activity == A::Applying
                {
                    Phase::Confirmed
                } else {
                    Phase::Running
                },
                progress: if matches!(status.activity, A::Renaming | A::Moving)
                    || rename.is_some()
                    || movement.is_some()
                    || directory_creation.is_some()
                {
                    ProgressView::None
                } else {
                    ProgressView::Indeterminate
                },
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
