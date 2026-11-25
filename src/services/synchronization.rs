//! Localized presentation of the public synchronization contract.
use drivesync_engine::{
    SynchronizationActivity, SynchronizationConfirmedAction, SynchronizationIssueKind,
    SynchronizationState, SynchronizationStatus, SynchronizationVerificationSource,
    SynchronizationWait,
};
use gettextrs::gettext;

/// Idle means this worker has no queued plan; it is never presented as whole-tree convergence.
pub(crate) fn description(status: &SynchronizationStatus) -> String {
    if status.state == SynchronizationState::Stopped {
        return gettext("Synchronization stopped.");
    }
    if status.state == SynchronizationState::Running && status.issue.is_none() {
        let procedure = match status.activity {
            SynchronizationActivity::Scanning => gettext("Checking local files…"),
            SynchronizationActivity::ScanningRemote => {
                gettext("Reading files and folders on Drive…")
            }
            SynchronizationActivity::Observing => {
                gettext("Checking local files and Drive for changes…")
            }
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
            SynchronizationActivity::Renaming => gettext("Renaming…"),
            SynchronizationActivity::Moving => gettext("Moving…"),
            _ => gettext("Applying changes…"),
        };
        return procedure;
    }
    if let Some(issue) = &status.issue {
        let message = match issue.kind {
            SynchronizationIssueKind::Deferred => gettext("Waiting for confirmation of the current state before making changes."),
            SynchronizationIssueKind::DeferredFor(reason) => match reason {
                SynchronizationWait::LocalObservation if status.activity == SynchronizationActivity::Observing && issue.retryable =>
                    gettext("The computer scan did not finish. DriveSync will try again."),
                SynchronizationWait::RemoteObservation if status.activity == SynchronizationActivity::Observing && issue.retryable =>
                    gettext("The Drive scan did not finish. DriveSync will try again."),
                SynchronizationWait::LocalObservation => gettext("Waiting for a complete scan of the computer."),
                SynchronizationWait::RemoteObservation => gettext("Waiting for a complete scan on Drive."),
                SynchronizationWait::FreshObservations => verification_description(status.verification),
                SynchronizationWait::CompleteObservations => gettext("Waiting for complete scans of the computer and Drive before applying changes."),
                SynchronizationWait::ItemVerification { source, confirmed_action } => item_verification(source, confirmed_action),
            },
            SynchronizationIssueKind::Conflict => gettext("Review the local and Drive versions to resolve the conflict."),
            SynchronizationIssueKind::AdoptionRequired => gettext("Initial association needs confirmation before this item can continue."),
            SynchronizationIssueKind::Unsupported(reason) => projection_message(reason),
            SynchronizationIssueKind::InvalidInput => gettext("The synchronization information is inconsistent. Refresh observations before continuing."),
            SynchronizationIssueKind::OperationFailed => gettext("A synchronization operation failed."),
            SynchronizationIssueKind::PreconditionFailed => gettext("The item changed before execution. Waiting for a fresh observation."),
            SynchronizationIssueKind::RecoveryRequired => gettext("An operation needs verification before synchronization can continue."),
            SynchronizationIssueKind::TrashRefused => gettext("DriveSync could not move the item to trash."),
            SynchronizationIssueKind::TrashRecoveryRequired => gettext("DriveSync needs to verify the result of the move to trash."),
            SynchronizationIssueKind::WorkerFailed => gettext("Synchronization could not continue. Stop and restart it before trying again."),
        };
        return message;
    }
    match status.state {
        SynchronizationState::Idle => gettext("No pending operations."),
        SynchronizationState::Failed => gettext("Synchronization failed."),
        _ if status.activity == SynchronizationActivity::Observing => {
            gettext("Waiting for the computer and Drive scans to finish.")
        }
        _ => gettext("Waiting for observations."),
    }
}

pub(crate) fn verification_description(
    verification: Option<drivesync_engine::SynchronizationVerification>,
) -> String {
    use drivesync_engine::SynchronizationVerification as Verification;
    gettext(match verification {
        Some(Verification {
            local_read: true,
            remote_read: false,
        }) => "Computer scan complete. Checking Drive…",
        Some(Verification {
            local_read: false,
            remote_read: true,
        }) => "Drive scan complete. Checking the computer…",
        Some(Verification {
            local_read: true,
            remote_read: true,
        }) => "Scans complete. Checking the result of the last operation…",
        _ => "Checking the computer and Drive again after the last operation.",
    })
}

fn item_verification(
    source: SynchronizationVerificationSource,
    confirmed_action: Option<SynchronizationConfirmedAction>,
) -> String {
    use SynchronizationConfirmedAction as Action;
    use SynchronizationVerificationSource as Source;
    let verification = gettext(match source {
        Source::Local => "Waiting to check the copy on the computer.",
        Source::Remote => "Waiting to check the copy on Drive.",
        Source::Both => "Waiting to check the copies on the computer and on Drive.",
        Source::Unspecified => "Waiting to verify the item.",
    });
    let Some(action) = confirmed_action else {
        return verification;
    };
    let completed = gettext(match action {
        Action::Download => "Download confirmed.",
        Action::Upload => "Upload confirmed.",
        Action::CreateLocalDirectory => "Folder created on the computer.",
        Action::CreateRemoteDirectory => "Folder created on Drive.",
    });
    format!("{completed} {verification}")
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
            account_id: drivesync_engine::AccountId("account".into()),
            state: SynchronizationState::Waiting,
            activity: SynchronizationActivity::Observing,
            current_item: None,
            current_operation: None,
            progress: None,
            verification: None,
            issue: None,
            updated_at: drivesync_engine::Timestamp { unix_millis: 0 },
        };
        assert_eq!(
            description(&status),
            gettext("Waiting for the computer and Drive scans to finish.")
        );
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
    fn confirmed_download_distinguishes_completion_from_pending_drive_verification() {
        let message = item_verification(
            SynchronizationVerificationSource::Remote,
            Some(SynchronizationConfirmedAction::Download),
        );
        assert_eq!(
            message,
            format!(
                "{} {}",
                gettext("Download confirmed."),
                gettext("Waiting to check the copy on Drive."),
            )
        );
    }

    #[test]
    fn unsupported_download_explains_the_missing_evidence() {
        use drivesync_engine::{
            SynchronizationIssue, SynchronizationItem, SynchronizationProjectionIssue,
        };
        let file = "ORIENTAÇÕES/Responsáveis por seção.docx";
        let status = SynchronizationStatus {
            account_id: drivesync_engine::AccountId("account".into()),
            state: SynchronizationState::Waiting,
            activity: SynchronizationActivity::None,
            current_item: Some(SynchronizationItem {
                display_name: None,
                item_id: None,
                local_path: Some(file.into()),
                remote_id: None,
            }),
            current_operation: None,
            progress: None,
            verification: None,
            issue: Some(SynchronizationIssue {
                kind: SynchronizationIssueKind::Unsupported(Some(
                    SynchronizationProjectionIssue::MissingRemoteContentEvidence,
                )),
                retryable: false,
            }),
            updated_at: drivesync_engine::Timestamp { unix_millis: 0 },
        };
        let message = description(&status);
        assert!(!message.contains(file));
        assert!(message.contains(&gettext("Drive did not provide the information needed to verify a download. The local file was kept.")));
    }
}
