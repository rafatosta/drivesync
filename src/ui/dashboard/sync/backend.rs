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
    refresh_latest: bool,
    last_synced: bool,
}
impl EngineAdapter {
    pub fn new(engine: drivesync_engine::Engine, account: drivesync_engine::AccountId) -> Self {
        Self {
            engine,
            account,
            subscription: None,
            ended: false,
            refresh_latest: false,
            last_synced: false,
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
                if !self.refresh_latest {
                    subscription.changed().await?;
                }
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
            let audited = if !self.last_synced
                && status.state == drivesync_engine::SynchronizationState::Idle
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
            let conflicts = self.engine.list_conflicts(self.account.clone()).await.ok();
            let latest = subscription.current();
            self.refresh_latest = latest != status;
            let background = is_background_check(&latest);
            // A new operation or issue ends the previously audited idle presentation.
            let mut snapshot = map_status(
                &latest,
                self.last_synced || (audited && !self.refresh_latest),
            );
            if let Some(rows) = conflicts.filter(|rows| !rows.is_empty()) {
                let count = rows.len();
                let summary = gettextrs::ngettext(
                    "{count} unresolved conflict.",
                    "{count} unresolved conflicts.",
                    u32::try_from(count).unwrap_or(u32::MAX),
                )
                .replace("{count}", &count.to_string());
                use drivesync_engine::SynchronizationState as State;
                let has_procedure = snapshot.status != StatusView::Synced
                    && (latest.issue.is_some()
                        || (!background && matches!(latest.state, State::Running | State::Failed)));
                if has_procedure && !snapshot.description.is_empty() {
                    snapshot.description = format!("{}\n{summary}", snapshot.description);
                } else {
                    snapshot.description = summary.clone();
                }
                if snapshot.status != StatusView::Failed {
                    snapshot.status = StatusView::Attention;
                }
                snapshot
                    .attention
                    .retain(|item| item.action != AttentionAction::OpenConflictList);
                snapshot.attention.push(AttentionItemView {
                    scope: snapshot.scope.clone(),
                    id: "persisted-conflicts".into(),
                    title: gettext("Synchronization conflicts"),
                    summary,
                    action: AttentionAction::OpenConflictList,
                });
            }
            self.last_synced = snapshot.status == StatusView::Synced;
            Some(snapshot)
        })
    }
}
fn is_background_check(status: &drivesync_engine::SynchronizationStatus) -> bool {
    use drivesync_engine::{SynchronizationActivity as A, SynchronizationState as S};
    status.state == S::Running
        && status.issue.is_none()
        && status.current_operation.is_none()
        && matches!(status.activity, A::Scanning | A::Observing | A::Analyzing)
}

pub fn map_status(status: &drivesync_engine::SynchronizationStatus, audited: bool) -> Snapshot {
    use drivesync_engine::{
        SynchronizationActivity as A, SynchronizationIssueKind as I, SynchronizationState as S,
        SynchronizationWait as W,
    };
    let scope = status.account_id.0.clone();
    let background = is_background_check(status);
    let issue = status.issue.as_ref().map(|issue| issue.kind);
    let observing = status.state == S::Waiting
        && status.activity == A::Observing
        && status.current_operation.is_none()
        && status.current_item.is_none()
        && matches!(
            issue,
            None | Some(I::DeferredFor(W::LocalObservation | W::RemoteObservation))
        );
    let effective_state = if background { S::Idle } else { status.state };
    let state = match (effective_state, issue) {
        (S::Waiting, _) if audited && observing => StatusView::Synced,
        (S::Stopped, _) => StatusView::Stopped,
        (S::Failed, _) => StatusView::Failed,
        (_, Some(I::Deferred | I::DeferredFor(_) | I::PreconditionFailed)) | (S::Waiting, None) => {
            StatusView::WaitingForEvidence
        }
        (_, Some(_)) => StatusView::Attention,
        (S::Running, None) if status.activity == A::Recovering => StatusView::Recovering,
        (S::Running, None) => StatusView::Synchronizing,
        (S::Idle, None) if audited => StatusView::Synced,
        (S::Idle, None) => StatusView::Waiting,
    };
    let activity = if effective_state == S::Running {
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
            let workspace_shortcut = status
                .current_operation
                .as_ref()
                .and_then(|op| op.workspace_shortcut);
            let trash = status.current_operation.as_ref().and_then(|op| op.trash);
            let (kind, direction) = match status.activity {
                A::Recovering => (ActivityKind::Recover, Direction::None),
                A::ProjectingWorkspace | A::Applying if workspace_shortcut.is_some() => {
                    use drivesync_engine::WorkspaceShortcutActivity;
                    let kind = match workspace_shortcut? {
                        WorkspaceShortcutActivity::Create => ActivityKind::CreateWorkspaceShortcut,
                        WorkspaceShortcutActivity::Update => ActivityKind::UpdateWorkspaceShortcut,
                    };
                    (kind, Direction::RemoteToLocal)
                }
                A::TrashingLocal | A::TrashingRemote | A::Applying if trash.is_some() => {
                    use drivesync_engine::SynchronizationTrashDestination;
                    let direction = match trash? {
                        SynchronizationTrashDestination::Local => Direction::RemoteToLocal,
                        SynchronizationTrashDestination::Remote => Direction::LocalToRemote,
                    };
                    (ActivityKind::Trash, direction)
                }
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
                .or_else(|| item.display_name.clone())
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
                    || directory_creation.is_some()
                    || workspace_shortcut.is_some()
                    || trash.is_some())
                    && status.activity == A::Applying
                {
                    Phase::Confirmed
                } else {
                    Phase::Running
                },
                progress: if matches!(status.activity, A::Renaming | A::Moving | A::Recovering)
                    || rename.is_some()
                    || movement.is_some()
                    || directory_creation.is_some()
                    || workspace_shortcut.is_some()
                    || trash.is_some()
                {
                    ProgressView::None
                } else if let Some(progress) = status
                    .progress
                    .as_ref()
                    .filter(|progress| progress.total > 1 && progress.completed <= progress.total)
                {
                    ProgressView::Steps {
                        completed: progress.completed,
                        total: progress.total,
                    }
                } else {
                    ProgressView::Indeterminate
                },
            })
        })
    } else {
        None
    };
    let description = if state == StatusView::Synced {
        gettext("Your files are up to date on this computer and on Drive.")
    } else if background {
        gettext("No pending operations.")
    } else if activity.is_some() && status.issue.is_none() {
        String::new()
    } else {
        crate::services::synchronization::description(status)
    };
    let blocker_id = match status.issue.as_ref().map(|issue| issue.kind) {
        Some(I::RecoveryRequired | I::TrashRecoveryRequired) => "recovery-blocker",
        _ => "current-blocker",
    };
    let attention = status
        .issue
        .as_ref()
        .filter(|_| matches!(state, StatusView::Attention | StatusView::Failed))
        .map(|issue| AttentionItemView {
            scope: scope.clone(),
            id: blocker_id.into(),
            title: match issue.kind {
                I::Unsupported(_) => gettext("Item could not be synchronized"),
                I::TrashRefused => gettext("Move to trash refused"),
                I::RecoveryRequired => gettext("Interrupted operation needs verification"),
                I::TrashRecoveryRequired => gettext("Trash recovery needs verification"),
                _ => gettext("Synchronization needs attention"),
            },
            summary: description.clone(),
            action: if issue.kind == I::Conflict {
                AttentionAction::OpenConflictList
            } else {
                AttentionAction::OpenIssue(blocker_id.into())
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
