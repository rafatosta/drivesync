//! Backend contracts use presentation values exclusively.
use super::models::*;
use crate::ui::components::sync_overview::SyncOverviewState;
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
    monitor: drivesync_engine::SynchronizationMonitor,
    received: bool,
    refresh_latest: bool,
    last_synced: bool,
    binding_revision: u64,
}
impl EngineAdapter {
    pub fn new(engine: drivesync_engine::Engine, account: drivesync_engine::AccountId) -> Self {
        Self {
            monitor: engine.monitor_synchronization(account.clone()),
            engine,
            account,
            received: false,
            refresh_latest: false,
            last_synced: false,
            binding_revision: 0,
        }
    }

    fn failure(&mut self, error: &drivesync_engine::EngineError) -> Snapshot {
        self.last_synced = false;
        self.refresh_latest = false;
        Snapshot {
            overview: Some(SyncOverviewState::Attention {
                pending_items: None,
            }),
            scope: self.account.0.clone(),
            status: StatusView::Failed,
            description: crate::core::messages::engine_error(error),
            item: None,
            active: false,
            activity: None,
            attention: vec![],
        }
    }
}
impl SyncBackend for EngineAdapter {
    fn next(&mut self) -> NextSnapshot<'_> {
        Box::pin(async move {
            if self.received && !self.refresh_latest {
                self.monitor.changed().await;
            }
            self.received = true;
            let status = match self.monitor.current().await {
                Ok(status) => status,
                Err(error) => return Some(self.failure(&error)),
            };
            let revision = self.monitor.binding_revision();
            if revision != self.binding_revision {
                self.binding_revision = revision;
                self.last_synced = false;
            }
            // Audit is horizon-bound. Re-read after the await so a newer event invalidates it.
            let audit = if !self.last_synced
                && status.state == drivesync_engine::SynchronizationState::Idle
                && status.issue.is_none()
            {
                self.engine
                    .synchronization_audit(self.account.clone())
                    .await
                    .ok()
                    .filter(|audit| audit.roots.account_id == self.account)
            } else {
                None
            };
            let conflicts = self.engine.list_conflicts(self.account.clone()).await.ok();
            let latest = match self.monitor.current().await {
                Ok(status) => status,
                Err(error) => return Some(self.failure(&error)),
            };
            let replaced = self.monitor.binding_revision() != revision;
            self.refresh_latest = replaced || latest != status;
            if replaced {
                self.last_synced = false;
            }
            let background = self.last_synced && is_background_check(&latest);
            // A new operation or issue ends the previously audited idle presentation.
            let audit = audit.filter(|_| !self.refresh_latest);
            let mut snapshot = map_status(
                &latest,
                self.last_synced || audit.as_ref().is_some_and(|audit| audit.initial_complete),
            );
            if snapshot.status == StatusView::VerificationIncomplete {
                snapshot.description = incomplete_description(audit.as_ref());
            }
            let pending_items = conflicts
                .as_ref()
                .filter(|rows| latest.issue.is_none() && !rows.is_empty())
                .map(|rows| rows.len() as u64);
            if let Some(rows) = conflicts.filter(|rows| !replaced && !rows.is_empty()) {
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
                        || snapshot.status == StatusView::Checking
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
            snapshot.overview = overview_state(&snapshot);
            if snapshot.status == StatusView::Attention {
                snapshot.overview = Some(SyncOverviewState::Attention { pending_items });
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
        && matches!(
            status.activity,
            A::Scanning | A::ScanningRemote | A::Observing | A::Analyzing
        )
}

fn incomplete_description(audit: Option<&drivesync_engine::SynchronizationAudit>) -> String {
    match audit {
        None => gettext("No pending operations."),
        Some(audit) if !audit.coverage_complete => {
            gettext("DriveSync needs current scans of both folders to confirm synchronization.")
        }
        Some(audit) if audit.unconfirmed_operations > 0 => gettextrs::ngettext(
            "{count} operation still needs confirmation.",
            "{count} operations still need confirmation.",
            u32::try_from(audit.unconfirmed_operations).unwrap_or(u32::MAX),
        )
        .replace("{count}", &audit.unconfirmed_operations.to_string()),
        Some(_) => gettext("There are no queued operations, but some items could not be verified."),
    }
}

pub fn map_status(status: &drivesync_engine::SynchronizationStatus, audited: bool) -> Snapshot {
    use drivesync_engine::{
        SynchronizationActivity as A, SynchronizationIssueKind as I, SynchronizationState as S,
        SynchronizationWait as W,
    };
    let scope = status.account_id.0.clone();
    let background = audited && is_background_check(status);
    let issue = status.issue.as_ref().map(|issue| issue.kind);
    let observing = status.state == S::Waiting
        && status.activity == A::Observing
        && status.current_operation.is_none()
        && status.current_item.is_none()
        && issue.is_none();
    let checking_after_operation = status.activity == A::Applying
        && status
            .verification
            .is_some_and(|check| !check.local_read || !check.remote_read);
    let effective_state = if background { S::Idle } else { status.state };
    let state = match (effective_state, issue) {
        (S::Waiting, _) if audited && observing => StatusView::Synced,
        (S::Stopped, _) => StatusView::Stopped,
        (S::Failed, _) => StatusView::Failed,
        (_, Some(I::DeferredFor(W::FreshObservations))) => StatusView::Checking,
        (
            _,
            Some(I::DeferredFor(W::ItemVerification {
                confirmed_action: Some(_),
                ..
            })),
        ) => StatusView::Finalizing,
        (S::Waiting, None) if observing => StatusView::Checking,
        (_, Some(I::Deferred | I::DeferredFor(_) | I::PreconditionFailed)) | (S::Waiting, None) => {
            StatusView::WaitingForEvidence
        }
        (_, Some(_)) => StatusView::Attention,
        (S::Running, None) if status.activity == A::Recovering => StatusView::Recovering,
        (S::Running, None) if checking_after_operation || is_background_check(status) => {
            StatusView::Checking
        }
        (S::Running, None) => StatusView::Synchronizing,
        (S::Idle, None) if audited => StatusView::Synced,
        (S::Idle, None) => StatusView::VerificationIncomplete,
    };
    let item_presentation = status.current_item.as_ref().and_then(|item| {
        let path = item
            .local_path
            .as_ref()
            .filter(|p| !p.as_os_str().is_empty());
        let name = status
            .current_operation
            .as_ref()
            .and_then(|op| op.display_name.clone())
            .or_else(|| {
                path.and_then(|p| p.file_name())
                    .map(|p| p.to_string_lossy().into_owned())
            })
            .or_else(|| item.display_name.clone())?;
        Some(StatusItem {
            name,
            path: path.map(|p| p.to_string_lossy().into_owned()),
        })
    });
    let activity = if effective_state == S::Running && issue.is_none() {
        status.current_item.as_ref().and_then(|item| {
            let updates_local = status
                .current_operation
                .as_ref()
                .is_some_and(|op| op.updates_local_content);
            let updates_remote = status
                .current_operation
                .as_ref()
                .is_some_and(|op| op.updates_remote_content);
            let newly_observed_remote = status
                .current_operation
                .as_ref()
                .is_some_and(|op| op.newly_observed_remote);
            let newly_observed_local = status
                .current_operation
                .as_ref()
                .is_some_and(|op| op.newly_observed_local);
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
                A::Downloading | A::Applying if newly_observed_remote => (
                    ActivityKind::DownloadDiscoveredRemote,
                    Direction::RemoteToLocal,
                ),
                A::Downloading => (ActivityKind::Download, Direction::RemoteToLocal),
                A::Uploading | A::Applying if newly_observed_local => (
                    ActivityKind::UploadDiscoveredLocal,
                    Direction::LocalToRemote,
                ),
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
            let name = item_presentation
                .as_ref()
                .map(|item| item.name.clone())
                .unwrap_or_else(|| gettext("Unidentified item"));
            Some(ActivityView {
                scope: scope.clone(),
                operation_id: status.current_operation.as_ref().map(|op| op.id.clone()),
                item_id,
                name,
                kind,
                direction,
                phase: if (updates_local
                    || updates_remote
                    || newly_observed_remote
                    || newly_observed_local
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
    } else if state == StatusView::VerificationIncomplete {
        incomplete_description(None)
    } else if state == StatusView::Checking && checking_after_operation && issue.is_none() {
        crate::services::synchronization::verification_description(status.verification)
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
    let item =
        item_presentation.filter(|_| !matches!(state, StatusView::Synced | StatusView::Stopped));
    let mut snapshot = Snapshot {
        overview: None,
        scope,
        status: state,
        description,
        item,
        active: effective_state == S::Running && issue.is_none(),
        activity,
        attention,
    };
    snapshot.overview = overview_state(&snapshot);
    snapshot
}

/// Operation steps and byte-less telemetry must never become account item progress.
pub(super) fn overview_state(snapshot: &Snapshot) -> Option<SyncOverviewState> {
    use crate::ui::components::sync_overview::SyncOverviewState as Overview;
    match snapshot.status {
        StatusView::Checking | StatusView::Initializing => Some(Overview::Checking {
            checked_items: None,
        }),
        StatusView::WaitingForEvidence | StatusView::VerificationIncomplete => {
            Some(Overview::Inactive {
                message: snapshot.description.clone(),
            })
        }
        StatusView::Synchronizing | StatusView::Recovering | StatusView::Finalizing => {
            Some(Overview::Syncing {
                completed_items: None,
                total_items: None,
                remaining_items: None,
                remaining_bytes: None,
                progress: None,
            })
        }
        StatusView::Synced => Some(Overview::Completed { last_sync: None }),
        StatusView::Attention | StatusView::Failed => Some(Overview::Attention {
            pending_items: None,
        }),
        StatusView::Stopped | StatusView::Offline => Some(Overview::Inactive {
            message: snapshot.description.clone(),
        }),
    }
}
