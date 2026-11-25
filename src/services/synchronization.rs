//! Saved-folder lifecycle and localized presentation of the public synchronization contract.
use adw::gtk::{gio, prelude::*};
use drivesync_engine::{
    AccountId, Engine, EngineErrorKind, StartSynchronizationRequest, SynchronizationActivity,
    SynchronizationErrorKind, SynchronizationIssueKind, SynchronizationState,
    SynchronizationStatus,
};
use gettextrs::gettext;

/// Starts only a completed saved scope. Opening a dashboard never grants an initial association.
pub(crate) async fn ensure_started(engine: &Engine, account: &AccountId) -> Result<bool, String> {
    let Some(config) = super::configuration::load(account).await.map_err(|_| {
        gettext("Saved configuration could not be read. Your account has been preserved.")
    })?
    else {
        return Ok(false);
    };
    let path = gio::File::for_uri(&config.local_uri)
        .path()
        .ok_or_else(|| gettext("The selected folder is unavailable. Choose another folder."))?;
    let request = StartSynchronizationRequest {
        account_id: account.clone(),
        local_root: path,
        remote_root_id: drivesync_engine::models::RemoteId(config.remote_id),
    };
    match engine.start_synchronization(request.clone()).await {
        Ok(_) => {}
        Err(error)
            if error.kind
                == EngineErrorKind::Synchronization(SynchronizationErrorKind::AlreadyRunning) =>
        {
            engine
                .stop_synchronization(account.clone())
                .await
                .map_err(|e| crate::core::messages::engine_error(&e))?;
            engine
                .start_synchronization(request)
                .await
                .map_err(|e| crate::core::messages::engine_error(&e))?;
        }
        Err(error) => return Err(crate::core::messages::engine_error(&error)),
    }
    Ok(true)
}
/// Idle means this worker has no queued plan; it is never presented as whole-tree convergence.
pub(crate) fn description(status: &SynchronizationStatus) -> String {
    if status.state == SynchronizationState::Stopped {
        return gettext("Synchronization stopped.");
    }
    if status.state == SynchronizationState::Running {
        return match status.activity {
            SynchronizationActivity::Scanning => gettext("Scanning files…"),
            SynchronizationActivity::Recovering => {
                gettext("Checking what the interrupted operation changed.")
            }
            SynchronizationActivity::Analyzing => gettext("Analyzing changes…"),
            SynchronizationActivity::Uploading => gettext("Uploading…"),
            SynchronizationActivity::Downloading => gettext("Downloading…"),
            SynchronizationActivity::TrashingLocal | SynchronizationActivity::TrashingRemote => {
                gettext("Moving to trash…")
            }
            SynchronizationActivity::ProjectingWorkspace => {
                match status
                    .current_operation
                    .as_ref()
                    .and_then(|op| op.workspace_shortcut)
                {
                    Some(drivesync_engine::WorkspaceShortcutActivity::Create) => {
                        gettext("Creating Workspace shortcut…")
                    }
                    _ => gettext("Updating Workspace shortcut…"),
                }
            }
            SynchronizationActivity::CreatingDirectory => gettext("Creating directory…"),
            _ => gettext("Applying changes…"),
        };
    }
    if let Some(issue) = &status.issue {
        let message = match issue.kind {
            SynchronizationIssueKind::Deferred => gettext("Waiting for more information about this item."),
            SynchronizationIssueKind::Conflict => gettext("A conflict needs resolution before this item can continue."),
            SynchronizationIssueKind::AdoptionRequired => gettext("Initial association needs confirmation before this item can continue."),
            SynchronizationIssueKind::Unsupported(reason) => projection_message(reason),
            SynchronizationIssueKind::InvalidInput => gettext("The synchronization information is inconsistent. Refresh observations before continuing."),
            SynchronizationIssueKind::OperationFailed => gettext("A synchronization operation failed."),
            SynchronizationIssueKind::PreconditionFailed => gettext("The item changed before execution. Waiting for a fresh observation."),
            SynchronizationIssueKind::RecoveryRequired => {
                if let Some(name) = status.current_item.as_ref()
                    .and_then(|item| item.local_path.as_ref())
                    .and_then(|path| path.file_name())
                {
                    gettext("The result of the operation for {item} is still unknown.")
                        .replace("{item}", &name.to_string_lossy())
                } else {
                    gettext("An operation needs verification before synchronization can continue.")
                }
            }
            SynchronizationIssueKind::TrashRefused | SynchronizationIssueKind::TrashRecoveryRequired => {
                let name = status.current_item.as_ref()
                    .and_then(|item| item.local_path.as_ref())
                    .and_then(|path| path.file_name())
                    .map(|name| name.to_string_lossy())
                    .unwrap_or_else(|| gettext("Unidentified item").into());
                let message = if issue.kind == SynchronizationIssueKind::TrashRefused {
                    gettext("Move to trash refused for {item}.")
                } else {
                    gettext("The trash result for {item} requires recovery verification.")
                };
                message.replace("{item}", &name)
            }
            SynchronizationIssueKind::WorkerFailed => gettext("Synchronization could not continue. Stop and restart it before trying again."),
        };
        if matches!(
            issue.kind,
            SynchronizationIssueKind::Unsupported(_)
                | SynchronizationIssueKind::OperationFailed
                | SynchronizationIssueKind::Deferred
        ) {
            if let Some(path) = status
                .current_item
                .as_ref()
                .and_then(|item| item.local_path.as_ref())
            {
                return gettext("{item} — {reason}")
                    .replace("{reason}", &message)
                    .replace("{item}", &path.to_string_lossy());
            }
        }
        return message;
    }
    match status.state {
        SynchronizationState::Idle => gettext("No pending operations."),
        SynchronizationState::Failed => gettext("Synchronization failed."),
        _ => gettext("Waiting for observations."),
    }
}

fn projection_message(reason: Option<drivesync_engine::SynchronizationProjectionIssue>) -> String {
    use drivesync_engine::SynchronizationProjectionIssue::*;
    match reason {
        Some(SpecialLocalFile) => gettext("Symbolic links and special files are skipped."),
        Some(InvalidLocalName) => gettext("This local file name cannot be used for synchronization."),
        Some(InvalidRemoteName) => gettext("This Drive file name cannot be represented on this computer."),
        Some(DriveShortcut) => gettext("Drive shortcuts are skipped."),
        Some(UnsupportedDriveType) => gettext("This Google Drive file type cannot be synchronized."),
        Some(NameCollision) => gettext("Multiple items would use the same name. Rename one of them to continue."),
        Some(OccupiedWorkspacePath) => gettext("An existing local file occupies the Google Workspace shortcut path."),
        Some(MissingRemoteContentEvidence) => gettext("Drive did not provide the information needed to verify a download. The local file was kept."),
        None => gettext("DriveSync cannot apply this change. Existing copies were kept."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn waiting_idle_and_uncertain_status_do_not_claim_synchronization() {
        let mut status = SynchronizationStatus {
            account_id: AccountId("account".into()),
            state: SynchronizationState::Waiting,
            activity: SynchronizationActivity::Observing,
            current_item: None,
            current_operation: None,
            progress: None,
            issue: None,
            updated_at: drivesync_engine::Timestamp { unix_millis: 0 },
        };
        assert_eq!(description(&status), gettext("Waiting for observations."));
        status.state = SynchronizationState::Idle;
        assert_eq!(description(&status), gettext("No pending operations."));
        status.issue = Some(drivesync_engine::SynchronizationIssue {
            kind: SynchronizationIssueKind::RecoveryRequired,
            retryable: false,
        });
        assert_eq!(
            description(&status),
            gettext("An operation needs verification before synchronization can continue.")
        );
    }
    #[test]
    fn unsupported_download_identifies_the_affected_file_and_the_missing_evidence() {
        use drivesync_engine::{
            SynchronizationIssue, SynchronizationItem, SynchronizationProjectionIssue,
        };
        let file = "ORIENTAÇÕES/Responsáveis por seção.docx";
        let status = SynchronizationStatus {
            account_id: AccountId("account".into()),
            state: SynchronizationState::Waiting,
            activity: SynchronizationActivity::None,
            current_item: Some(SynchronizationItem {
                item_id: None,
                local_path: Some(file.into()),
                remote_id: None,
            }),
            current_operation: None,
            progress: None,
            issue: Some(SynchronizationIssue {
                kind: SynchronizationIssueKind::Unsupported(Some(
                    SynchronizationProjectionIssue::MissingRemoteContentEvidence,
                )),
                retryable: false,
            }),
            updated_at: drivesync_engine::Timestamp { unix_millis: 0 },
        };
        let message = description(&status);
        assert!(message.contains(file));
        assert!(message.contains(&gettext("Drive did not provide the information needed to verify a download. The local file was kept.")));
    }
}
