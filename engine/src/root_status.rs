//! Persisted reasons why a folder identity cannot be verified.
use crate::EngineError;
use serde::{Deserialize, Serialize};

/// Durable root availability values retained for existing metadata.
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
    LocalStorageFull,
    RemoteStorageFull,
    RemoteQuotaExceeded,
}

impl std::fmt::Display for RootSuspension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::LocalStorageFull => return EngineError::LocalStorageFull.fmt(f),
            Self::RemoteStorageFull => return EngineError::RemoteStorageFull.fmt(f),
            Self::RemoteQuotaExceeded => return EngineError::RemoteQuotaExceeded.fmt(f),
            Self::StateUnavailable => "The saved folder state could not be validated. Existing data was preserved.",
            Self::LocalUnavailable => "The local folder is unavailable. Restore folder access; existing data was preserved.",
            Self::IdentityUnverified => "The selected folder's identity could not be verified. Restore the original folder and its DriveSync metadata. Existing data was preserved.",
            Self::RemoteUnavailable => "The Google Drive folder is unavailable or cannot be verified.",
            Self::RemoteAccessDenied => "Google Drive denied access to this folder.",
            Self::Offline => "Google Drive could not be reached.",
            Self::RemoteRetry => "Google Drive is temporarily unavailable or limiting requests.",
            Self::AuthenticationRequired => "Reconnect this Google account to verify folder access.",
        })
    }
}

impl RootSuspension {
    /// Indicates whether the stored reason describes a transient provider failure.
    pub fn retryable(self) -> bool {
        matches!(self, Self::Offline | Self::RemoteRetry)
    }
}
