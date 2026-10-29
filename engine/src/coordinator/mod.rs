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
    /// Only transient connection loss permits automatic availability probes.
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
/// Classifies root-level failures only. Item 404s must remain item diagnostics.
pub(crate) fn remote(error: EngineError) -> EngineError {
    let reason = match error {
        EngineError::RemoteFolderUnavailable | EngineError::RemoteChanged => {
            RootSuspension::RemoteUnavailable
        }
        EngineError::RemoteForbidden => RootSuspension::RemoteAccessDenied,
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

#[cfg(test)]
mod tests;
