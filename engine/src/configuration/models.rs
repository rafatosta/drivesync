//! Data models for configured Google Drive and local folders.
//!
//! These records are metadata and do not assert that authentication is valid
//! or synchronization is running.
use crate::AccountId;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Remote folder identity and display name obtained from Google Drive.
///
/// This type holds metadata only. A saved folder does not imply current access
/// to its remote contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriveFolder {
    /// Provider folder ID, subject to internal folder identifier validation.
    pub id: String,
    /// Nonempty Drive folder name; control characters are rejected.
    pub name: String,
}
impl DriveFolder {
    /// Checks whether the remote folder metadata can be safely accepted.
    ///
    /// Requires an allowed identifier and a nonempty, bounded name without
    /// control characters.
    pub(crate) fn valid(&self) -> bool {
        valid_id(&self.id)
            && !self.name.is_empty()
            && self.name.len() <= 4096
            && !self.name.chars().any(char::is_control)
    }
}
/// Validates a Drive identifier before embedding it in URL paths or queries.
///
/// Only ASCII letters, digits, underscore and hyphen are accepted, with a
/// maximum of 255 bytes. This also rejects query/path injection characters.
pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 255
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
/// Persisted association of an account, a Drive folder and a local directory.
///
/// `local` is the canonical local path used to access the directory; the
/// `local_display` path represents its resolved host origin when a Flatpak
/// document-portal alias is involved. The latter supports safe overlap checks.
/// This structure describes configuration, not a running synchronization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FolderConfiguration {
    /// Account that owns this folder mapping.
    pub account_id: AccountId,
    /// Selected remote Drive folder.
    pub remote: DriveFolder,
    /// Canonical path for local filesystem access.
    pub local: PathBuf,
    /// Host-origin path used to detect aliases and overlaps.
    pub local_display: PathBuf,
}
