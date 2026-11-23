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
            SynchronizationActivity::Analyzing => gettext("Analyzing changes…"),
            SynchronizationActivity::Uploading => gettext("Uploading…"),
            SynchronizationActivity::Downloading => gettext("Downloading…"),
            _ => gettext("Applying changes…"),
        };
    }
    if let Some(issue) = &status.issue {
        return match issue.kind {
            SynchronizationIssueKind::Deferred => gettext("Waiting for more information about this item."),
            SynchronizationIssueKind::Conflict => gettext("A conflict needs resolution before this item can continue."),
            SynchronizationIssueKind::AdoptionRequired => gettext("Initial association needs confirmation before this item can continue."),
            SynchronizationIssueKind::Unsupported => gettext("This change is not supported yet."),
            SynchronizationIssueKind::InvalidInput => gettext("The synchronization information is inconsistent. Refresh observations before continuing."),
            SynchronizationIssueKind::OperationFailed => gettext("An operation failed. Check account and folder access."),
            SynchronizationIssueKind::PreconditionFailed => gettext("The item changed before execution. Waiting for a fresh observation."),
            SynchronizationIssueKind::RecoveryRequired => gettext("An operation needs verification before this item can continue."),
            SynchronizationIssueKind::WorkerFailed => gettext("Synchronization could not continue. Stop and restart it before trying again."),
        };
    }
    match status.state {
        SynchronizationState::Idle => gettext("No pending operations."),
        SynchronizationState::Failed => gettext("Synchronization failed."),
        _ => gettext("Waiting for observations."),
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
            gettext("An operation needs verification before this item can continue.")
        );
    }
}
