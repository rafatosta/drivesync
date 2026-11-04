//! Sanitized failures for authentication and folder configuration.
use crate::AuthError;

/// Failures exposed by the account and folder facade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    Authentication(AuthError),
    InvalidFolder,
    InvalidAccountDescription,
    LocalFolderIdentity,
    FolderOverlap,
    RemoteFolderUnavailable,
    RemoteForbidden,
    DriveApiDisabled,
    RemoteRateLimited,
    RemoteStorageFull,
    RemoteQuotaExceeded,
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Authentication(error) => error.fmt(f),
            Self::InvalidFolder => f.write_str("Choose an existing, writable local folder."),
            Self::InvalidAccountDescription => f.write_str("Use up to 128 characters without control characters for the account description."),
            Self::LocalFolderIdentity => f.write_str("Could not verify this folder’s location. Check that your desktop portal is up to date and choose the folder again."),
            Self::FolderOverlap => f.write_str("The local folder overlaps another account’s folder. Choose a different location."),
            Self::RemoteFolderUnavailable => f.write_str("This Google Drive folder is unavailable. Choose it again."),
            Self::RemoteForbidden => f.write_str("Google Drive denied access to this folder. Check this account’s access or choose another folder."),
            Self::DriveApiDisabled => f.write_str("Google Drive access is unavailable because Drive API is disabled for this app. Contact the DriveSync distributor."),
            Self::RemoteRateLimited => f.write_str("Google Drive is receiving too many requests. Please try again later."),
            Self::RemoteStorageFull => f.write_str("Google Drive storage is full. Free space or increase this account’s storage."),
            Self::RemoteQuotaExceeded => f.write_str("The app’s Google Drive API quota was exceeded. Contact the DriveSync distributor or wait for the quota to reset."),
        }
    }
}

impl std::error::Error for EngineError {}
