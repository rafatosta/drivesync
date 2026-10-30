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
    /// No transfer started; review this exact batch before an explicit retry.
    BulkReviewRequired(crate::BulkReview),
    /// An explicit proposal became stale even though impact is now below policy.
    BulkReviewChanged,
    RootSuspended(crate::RootSuspension),
    InventoryLimit,
    StateStorage,
    StateRecoveryPending,
    InvalidPlan,
    DownloadConflict,
    LocalWriting,
    LocalTemporary,
    ObservationUnknown,
    DownloadIntegrity,
    DownloadIo,
    RemoteChanged,
    ChangeCursorRejected,
    SyncBusy,
    InvalidAccountDescription,
    LocalFolderIdentity,
    FolderOverlap,
    FolderIdentityMismatch,
    FolderIdentityStorage,
    RemoteFolderUnavailable,
    RemoteForbidden,
    DriveApiDisabled,
    RemoteRateLimited,
    /// The persisted upload deadline has not elapsed; no replay was admitted.
    UploadDeferred,
    /// Only read-only reconciliation is allowed for this preserved upload intent.
    UploadRetryExhausted,
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BulkReviewRequired(review) => review.fmt(f),
            Self::BulkReviewChanged => f.write_str("The reviewed batch changed and this attempt was cancelled. Start a new check before continuing. Existing data was preserved."),
            Self::RootSuspended(reason) => reason.fmt(f),
            Self::NotImplemented(operation) => write!(f, "{operation} is not implemented"),
            Self::Authentication(error) => error.fmt(f),
            Self::LocalFolderIdentity => f.write_str("Could not verify this folder’s location. Check that your desktop portal is up to date and choose the folder again."),
            Self::InvalidAccountDescription => f.write_str("Use up to 128 characters without control characters for the account description."),
            Self::StateRecoveryPending => f.write_str("Uploads are blocked because their previous results cannot be verified. Preserved upload evidence requires reconciliation before uploads can be enabled."),
            Self::StateStorage => f.write_str("The synchronization database could not be opened or validated. Existing data was preserved."),
            Self::LocalWriting => f.write_str("This file is still changing. Synchronization will check it again on the next cycle."),
            Self::LocalTemporary => f.write_str("This looks like a temporary file. Wait for the application to save its final file."),
            Self::ObservationUnknown => f.write_str("This item could not be verified. Check access and synchronize again. Existing versions were preserved."),
            Self::DownloadConflict => f.write_str("A local file or folder conflicts with this download. Existing data was preserved."),
            Self::DownloadIntegrity => f.write_str("The downloaded file failed its size or checksum verification. It was not installed."),
            Self::DownloadIo => f.write_str("Could not safely read, write or synchronize the local folder. Check permissions and free space."),
            Self::ChangeCursorRejected => f.write_str("The Drive change cursor was rejected. A complete reconciliation is required."),
            Self::RemoteChanged => f.write_str("The item or its folder changed on Google Drive during verification. Run synchronization again. Existing local files were preserved."),
            Self::SyncBusy => f.write_str("This local folder is already being synchronized."),
            Self::InvalidPlan => f.write_str("The download plan is inconsistent or its configuration changed. Scan the folder again."),
            Self::InventoryLimit => f.write_str("The remote inventory exceeded its safety limits. Choose a smaller folder."),
            Self::InvalidFolder => f.write_str("Choose an existing, writable local folder."),
            Self::FolderOverlap => f.write_str("The local folder overlaps another account’s folder. Choose a different location."),
            Self::FolderIdentityMismatch => f.write_str("This local folder is already associated with a different DriveSync account or Drive folder. Its files were left unchanged."),
            Self::FolderIdentityStorage => f.write_str("Could not safely read or write this folder’s DriveSync identity. Its files were left unchanged."),
            Self::RemoteFolderUnavailable => f.write_str("This Google Drive folder is unavailable. Choose it again."),
            Self::RemoteForbidden => f.write_str("Google Drive denied access to this folder. Check this account’s access or choose another folder."),
            Self::DriveApiDisabled => f.write_str("Google Drive access is unavailable because Drive API is disabled for this app. Contact the DriveSync distributor."),
            Self::UploadDeferred => f.write_str("This upload is waiting before another attempt. Existing data was preserved."),
            Self::UploadRetryExhausted => f.write_str("This upload reached its attempt limit. Only remote verification will continue; local data and upload evidence were preserved."),
            Self::RemoteRateLimited => f.write_str("Google Drive is receiving too many requests. Please try again later."),
        }
    }
}
impl std::error::Error for EngineError {}
