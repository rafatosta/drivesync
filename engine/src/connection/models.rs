//! Minimal application-facing types; provider credentials never cross this boundary.
use crate::accounts::{Account, AccountId, AccountsError};
use std::path::PathBuf;
/// Bootstrap settings. Missing OAuth setup does not prevent local account use.
#[derive(Clone)]
pub struct EngineConfig {
    pub data_directory: PathBuf,
    pub application_id: String,
    pub google_oauth_file: Option<PathBuf>,
}
/// A sign-in capability belonging to one engine instance, without a public constructor.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct GoogleSignInId(pub(super) String);
impl std::fmt::Debug for GoogleSignInId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GoogleSignInId(..)")
    }
}
/// None adds an account; Some reauthorizes that exact account identity.
#[derive(Default)]
pub struct BeginGoogleSignInRequest {
    pub account_id: Option<AccountId>,
}
/// The only browser-facing data. Never contains tokens or the PKCE verifier.
pub struct BeginGoogleSignInResult {
    pub connection_id: GoogleSignInId,
    pub authorization_url: String,
}
impl std::fmt::Debug for BeginGoogleSignInResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BeginGoogleSignInResult { .. }")
    }
}
/// Consume one pending sign-in attempt.
pub struct FinishGoogleSignInRequest {
    pub connection_id: GoogleSignInId,
}
/// Success is returned only after Accounts securely persists authorization.
#[derive(Debug)]
pub struct FinishGoogleSignInResult {
    pub account: Account,
    pub created: bool,
}
/// Cancel without promising rollback of a commit already started.
pub struct CancelGoogleSignInRequest {
    pub connection_id: GoogleSignInId,
}
/// False means absent, completed, or already committing.
#[derive(Debug)]
pub struct CancelGoogleSignInResult {
    pub cancelled: bool,
}
/// Application-facing error category, independent of provider implementation types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineErrorKind {
    InvalidConfiguration,
    AuthorizationUnavailable,
    AttemptNotFound,
    AlreadyInProgress,
    Cancelled,
    TimedOut,
    AuthorizationDenied,
    InvalidCallback,
    ProviderUnavailable,
    Accounts(AccountsError),
    Internal,
    RemoteFolder(RemoteFolderErrorKind),
}
/// Sanitized error, without backend text, OAuth codes or credential material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineError {
    pub kind: EngineErrorKind,
    pub retryable: bool,
    pub message: String,
}
/// Result of an application request.
pub type EngineResult<T> = Result<T, EngineError>;
impl EngineError {
    pub(super) fn new(kind: EngineErrorKind) -> Self {
        Self {
            kind,
            retryable: matches!(
                kind,
                EngineErrorKind::ProviderUnavailable | EngineErrorKind::TimedOut
            ),
            message: format!("Engine request failed: {kind:?}"),
        }
    }
}
impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for EngineError {}
impl From<crate::accounts::AccountsFailure> for EngineError {
    fn from(value: crate::accounts::AccountsFailure) -> Self {
        let mut error = Self::new(EngineErrorKind::Accounts(value.kind));
        error.retryable = value.retryable;
        error
    }
}

/// Metadata-only folder reference; IDs belong to the selected account's provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFolder {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
}
/// One page of immediate folders; continuation tokens are opaque.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFolderPage {
    pub folders: Vec<RemoteFolder>,
    pub next_page_token: Option<String>,
}
/// A read-only query scoped to one locally registered account.
pub struct ListRemoteFoldersRequest {
    pub account_id: AccountId,
    pub parent_id: String,
    pub page_token: Option<String>,
}
/// Stable remote browsing failures; no server diagnostics or credentials escape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteFolderErrorKind {
    AuthorizationRequired,
    PermissionDenied,
    NotFound,
    Unavailable,
    InvalidResponse,
    InvalidRequest,
}
