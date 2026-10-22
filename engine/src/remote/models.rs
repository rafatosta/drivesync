//! Data models for read-only Google Drive folder metadata.
//!
//! The provider's JSON models are private to this module. Their decoded
//! values are validated before the engine receives a [`FolderPage`].
use crate::{DriveFolder, EngineError};
use serde::Deserialize;

/// Google Drive MIME type used to identify a folder.
pub(super) const MIME: &str = "application/vnd.google-apps.folder";

/// A page of validated folders returned by a Google Drive listing.
///
/// Pagination is explicit: when `next_page` is present, callers can request
/// subsequent pages without treating a partial listing as complete.
#[derive(Debug, Clone)]
pub struct FolderPage {
    /// Non-trashed entries confirmed as Google Drive folders.
    pub folders: Vec<DriveFolder>,
    /// Opaque token for the next page, or `None` at the end.
    pub next_page: Option<String>,
}

/// Metadata returned by Drive's files API.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Metadata {
    id: String,
    name: String,
    mime_type: String,
    trashed: bool,
}
impl Metadata {
    /// Converts raw Drive metadata into a valid folder.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::RemoteFolderUnavailable`] for a trashed,
    /// non-folder or invalid remote entry.
    pub(super) fn folder(self) -> Result<DriveFolder, EngineError> {
        let folder = DriveFolder {
            id: self.id,
            name: self.name,
        };
        if self.mime_type != MIME || self.trashed || !folder.valid() {
            return Err(EngineError::RemoteFolderUnavailable);
        }
        Ok(folder)
    }
}

/// Decoded response for a Drive folder-listing request.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Page {
    /// Unvalidated folder metadata supplied by the provider.
    pub(super) files: Vec<Metadata>,
    /// Opaque continuation token; checked before exposing it to callers.
    pub(super) next_page_token: Option<String>,
    /// Indicates a partial search that must not be treated as complete.
    #[serde(default)]
    pub(super) incomplete_search: bool,
}

/// Provider reason code; raw provider text is never surfaced to users.
#[derive(Deserialize)]
pub(super) struct Reason {
    pub(super) reason: Option<String>,
}
#[derive(Deserialize)]
/// Structured error details returned by Drive; only reason codes are read.
pub(super) struct ProviderError {
    #[serde(default)]
    pub(super) errors: Vec<Reason>,
    #[serde(default)]
    pub(super) details: Vec<Reason>,
}
#[derive(Deserialize)]
/// Top-level error object used to classify HTTP 403 responses.
pub(super) struct ErrorEnvelope {
    pub(super) error: ProviderError,
}
