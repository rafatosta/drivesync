//! User-facing error text, without exposing engine implementation diagnostics.
use drivesync_engine::{AccountsError, EngineError, EngineErrorKind};
pub(crate) fn engine_error(error: &EngineError) -> String {
    gettextrs::gettext(match error.kind {
        EngineErrorKind::AuthorizationUnavailable | EngineErrorKind::InvalidConfiguration => {
            "Google sign-in is not configured for this installation."
        }
        EngineErrorKind::Cancelled => "Sign-in was cancelled.",
        EngineErrorKind::TimedOut => "Sign-in expired. Please try again.",
        EngineErrorKind::AuthorizationDenied => {
            "Google authorization was not completed. Please try again."
        }
        EngineErrorKind::ProviderUnavailable => {
            "Google could not be reached. Check your connection and try again."
        }
        EngineErrorKind::Accounts(AccountsError::AccountInUse) => {
            "Finish or cancel account authorization before disconnecting."
        }
        EngineErrorKind::Accounts(AccountsError::IdentityMismatch) => {
            "Sign in with the Google account selected in DriveSync."
        }
        EngineErrorKind::Accounts(AccountsError::SecureStorageUnavailable) => {
            "Secure credential storage is unavailable. Unlock your keyring and try again."
        }
        EngineErrorKind::Accounts(AccountsError::StorageUnavailable) => {
            "Saved accounts could not be accessed. Your data has been kept."
        }
        EngineErrorKind::Accounts(AccountsError::InvalidRequest) => {
            "The account name could not be saved. Check the value and try again."
        }
        EngineErrorKind::RemoteFolder(
            drivesync_engine::RemoteFolderErrorKind::AuthorizationRequired,
        ) => "Authorize Drive access again to browse your folders.",
        EngineErrorKind::RemoteFolder(
            drivesync_engine::RemoteFolderErrorKind::PermissionDenied,
        ) => "You do not have permission to access this folder.",
        EngineErrorKind::RemoteFolder(drivesync_engine::RemoteFolderErrorKind::NotFound) => {
            "Folder not found."
        }
        EngineErrorKind::RemoteFolder(_) => "Unable to load folders. Try again.",
        _ => "The account request could not be completed. Please try again.",
    })
}
