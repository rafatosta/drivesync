//! Public account identity, configuration and status models.
//!
//! Models describe metadata and capabilities, not independently verified
//! credentials or a running synchronization.
use crate::{AuthError, FolderConfiguration};

/// Current high-level engine configuration status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineStatus {
    NotConfigured,
}
/// Stable Google account identity derived from the provider subject.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct AccountId(pub String);
impl AccountId {
    pub(crate) fn from_subject(subject: &str) -> Self {
        Self(format!("google:{subject}"))
    }
}

/// Sanitized identity. Saved/listed metadata alone does not prove authentication.
/// add_account/verify_account confirm it at the time of the operation; no sync
/// root is implied.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountSummary {
    /// Stable identifier derived from the Google provider subject.
    pub id: AccountId,
    /// Provider subject used to detect identity changes.
    pub provider_subject: String,
    /// Verified email, when provided by the identity provider.
    pub email: Option<String>,
}
/// Identifier reserved for future synchronization roots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SyncRootId(pub u64);

/// A saved identity plus the result of verifying it during this startup.
/// `error == None` is the only authenticated outcome; failures retain the profile.
#[derive(Debug, Clone)]
pub struct RestoredAccount {
    /// Cached or freshly verified account identity.
    pub account: AccountSummary,
    /// Startup verification error, if verification was unsuccessful.
    pub error: Option<AuthError>,
    /// Previously configured folder pair, if any.
    pub folder: Option<FolderConfiguration>,
}
