//! Sanitized engine-level failures for authentication, folder setup and Drive.
//!
//! Errors do not incorporate raw provider payloads or private folder names.
use crate::AuthError;

/// Failures exposed by the engine facade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    NotImplemented(&'static str),
    Authentication(AuthError),
    InvalidFolder,
    InvalidAccountDescription,
    LocalFolderIdentity,
    FolderOverlap,
    RemoteFolderUnavailable,
    RemoteForbidden,
    DriveApiDisabled,
    RemoteRateLimited,
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotImplemented(operation) => write!(f, "{operation} is not implemented"),
            Self::Authentication(error) => error.fmt(f),
            Self::LocalFolderIdentity => f.write_str("Could not verify this folder’s location. Check that your desktop portal is up to date and choose the folder again."),
            Self::InvalidAccountDescription => f.write_str("Use up to 128 characters without control characters for the account description."),
            Self::InvalidFolder => f.write_str("Choose an existing, writable local folder."),
            Self::FolderOverlap => f.write_str("The local folder overlaps another account’s folder. Choose a different location."),
            Self::RemoteFolderUnavailable => f.write_str("This Google Drive folder is unavailable. Choose it again."),
            Self::RemoteForbidden => f.write_str("Google Drive denied access to this folder. Check this account’s access or choose another folder."),
            Self::DriveApiDisabled => f.write_str("Google Drive access is unavailable because Drive API is disabled for this app. Contact the DriveSync distributor."),
            Self::RemoteRateLimited => f.write_str("Google Drive is receiving too many requests. Please try again later."),
        }
    }
}
impl std::error::Error for EngineError {}
