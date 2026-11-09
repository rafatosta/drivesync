//! HTTP client entry point of the Google provider.

use super::{FileGetOptions, FileId, GoogleFile, GoogleResult, GoogleSession};

const DRIVE_BASE: &str = "https://www.googleapis.com/drive/v3";
const UPLOAD_BASE: &str = "https://www.googleapis.com/upload/drive/v3";

/// Client for the Google Drive REST API v3.
///
/// Holds no credentials: every call receives a [`GoogleSession`].
#[derive(Debug, Clone)]
pub struct GoogleDriveClient {
    #[allow(dead_code)]
    drive_base: String,
    #[allow(dead_code)]
    upload_base: String,
}

impl GoogleDriveClient {
    /// Creates a client targeting the official Google endpoints.
    pub fn new() -> Self {
        Self {
            drive_base: DRIVE_BASE.to_owned(),
            upload_base: UPLOAD_BASE.to_owned(),
        }
    }

    /// Creates a client targeting arbitrary base URLs.
    ///
    /// Test-only (`test-endpoints` feature): production code must never accept
    /// caller-controlled endpoints.
    #[cfg(feature = "test-endpoints")]
    pub fn with_base_urls(drive_base: impl Into<String>, upload_base: impl Into<String>) -> Self {
        Self {
            drive_base: drive_base.into(),
            upload_base: upload_base.into(),
        }
    }

    /// `GET /files/{fileId}`: reads file metadata.
    ///
    /// Blocking call; callers must not invoke it from the GTK main thread.
    // RED: behaviour pending.
    pub fn files_get(
        &self,
        _session: &GoogleSession,
        _file_id: &FileId,
        _options: FileGetOptions,
    ) -> GoogleResult<GoogleFile> {
        unimplemented!("files_get (RED)")
    }
}

impl Default for GoogleDriveClient {
    fn default() -> Self {
        Self::new()
    }
}
