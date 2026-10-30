//! Availability gates shared by preparation, downloads and create-only uploads.
//! A suspension never means that the root or its descendants were deleted.
use crate::{
    configuration::identity, download::filesystem::Root, AuthError, EngineError,
    FolderConfiguration,
};
use serde::{Deserialize, Serialize};

/// Sanitized, durable reasons why a configured root cannot authorize work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum RootSuspension {
    StateUnavailable,
    LocalUnavailable,
    IdentityUnverified,
    RemoteUnavailable,
    RemoteAccessDenied,
    Offline,
    RemoteRetry,
    AuthenticationRequired,
}
impl std::fmt::Display for RootSuspension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::StateUnavailable => "Synchronization suspended: local state could not be validated. Choose Recover synchronization state to rebuild observations while preserving existing files and database evidence.",
            Self::LocalUnavailable => "Synchronization suspended: the local folder is unavailable. Reconnect the volume or restore folder access, then try again. Existing data was preserved.",
            Self::IdentityUnverified => "Synchronization suspended: the selected folder's identity could not be verified. Restore the original folder and its DriveSync metadata. Existing data was preserved.",
            Self::RemoteUnavailable => "Synchronization suspended: the Drive folder is unavailable or cannot be verified. It may have been moved, removed or become inaccessible. Restore access and try again; local files were preserved.",
            Self::RemoteAccessDenied => "Synchronization suspended: Drive denied access. Restore this account's folder permissions and try again. Local files were preserved.",
            Self::Offline => "Synchronization suspended: Drive could not be reached. Synchronization will verify the folders again when the connection recovers.",
            Self::RemoteRetry => "Synchronization suspended: Drive is temporarily unavailable or limiting requests. A bounded retry delay applies; local files were preserved.",
            Self::AuthenticationRequired => "Synchronization suspended: reconnect this Google account to verify folder access. Local files were preserved.",
        })
    }
}
impl RootSuspension {
    /// Only transient connection or provider failures permit automatic probes.
    pub fn retryable(self) -> bool {
        matches!(self, Self::Offline | Self::RemoteRetry)
    }
}
/// Opens and locks one physical root before any preparation or state access.
/// The caller retains this descriptor until all effects in the cycle finish.
pub(crate) fn open(
    client: &str,
    folder: &FolderConfiguration,
    expected: Option<&str>,
) -> Result<Root, EngineError> {
    let root = Root::open(&folder.local).map_err(|e| {
        if e == EngineError::SyncBusy {
            e
        } else {
            EngineError::RootSuspended(RootSuspension::LocalUnavailable)
        }
    })?;
    verify(&root, client, folder, expected)?;
    Ok(root)
}
pub(crate) fn verify(
    root: &Root,
    client: &str,
    folder: &FolderConfiguration,
    expected: Option<&str>,
) -> Result<(), EngineError> {
    root.validate()
        .map_err(|_| EngineError::RootSuspended(RootSuspension::LocalUnavailable))?;
    let marker = identity::verify_at(&root.file, folder, client)?;
    if expected != Some(marker.as_str()) {
        return Err(EngineError::RootSuspended(
            RootSuspension::IdentityUnverified,
        ));
    }
    Ok(())
}
/// Transport failures suspend work without claiming that a particular item vanished.
pub(crate) fn connection(error: EngineError) -> EngineError {
    let reason = match error {
        EngineError::Authentication(AuthError::Network | AuthError::TimedOut) => {
            RootSuspension::Offline
        }
        EngineError::Authentication(AuthError::ProviderUnavailable)
        | EngineError::RemoteRateLimited => RootSuspension::RemoteRetry,
        EngineError::Authentication(
            AuthError::ReauthorizationRequired | AuthError::MissingPermissions,
        ) => RootSuspension::AuthenticationRequired,
        other => return other,
    };
    EngineError::RootSuspended(reason)
}

/// Classifies root-level failures only. Item 404s must remain item diagnostics.
pub(crate) fn remote(error: EngineError) -> EngineError {
    let reason = match connection(error) {
        EngineError::RemoteFolderUnavailable
        | EngineError::RemoteChanged
        | EngineError::Authentication(AuthError::InvalidResponse) => {
            RootSuspension::RemoteUnavailable
        }
        EngineError::RemoteForbidden => RootSuspension::RemoteAccessDenied,
        other => return other,
    };
    EngineError::RootSuspended(reason)
}

/// A failed transfer cannot fall through into another direction or clear suspension.
pub(crate) fn report(report: &crate::DownloadReport) -> Result<(), EngineError> {
    for issue in &report.issues {
        if let error @ EngineError::RootSuspended(_) = connection(issue.error.clone()) {
            return Err(error);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

/// Suspends all effects unless this attempt explicitly confirms identical evidence.
/// Approval is borrowed for this cycle only and is never saved in a preference.
pub(crate) fn bulk_gate(
    client: &str,
    record: &crate::state::Reconciliation,
    approval: Option<&crate::BulkReview>,
) -> Result<(), EngineError> {
    match (
        crate::planner::bulk_impact(client, record, crate::BulkPolicy::default())?,
        approval,
    ) {
        (Some(review), Some(approved)) if &review == approved => Ok(()),
        (Some(review), _) => Err(EngineError::BulkReviewRequired(review)),
        (None, Some(_)) => Err(EngineError::BulkReviewChanged),
        (None, None) => Ok(()),
    }
}
