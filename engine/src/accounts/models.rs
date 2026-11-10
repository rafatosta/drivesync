//! Account metadata, requests, results and sanitized failures.
pub use crate::models::{AccountId, ProviderId, Timestamp};
use serde::{Deserialize, Serialize};
/// Result of an account operation.
pub type AccountsResult<T> = Result<T, AccountsFailure>;
/// Persisted public account metadata; never contains credential material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    pub provider: ProviderId,
    pub provider_user_id: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub state: AccountState,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
/// Last known credential condition, not a network connectivity probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountState {
    Connected,
    ReauthorizationRequired,
    Unavailable,
}
/// Stable failure category without backend diagnostics or private data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountsError {
    InvalidRequest,
    UnsupportedProvider,
    NotFound,
    AlreadyExists,
    IdentityMismatch,
    AuthorizationFailed,
    SecureStorageUnavailable,
    CredentialMissing,
    ProviderUnavailable,
    AccountInUse,
    StorageUnavailable,
    Internal,
}
/// Sanitized error safe for presentation and diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountsFailure {
    pub kind: AccountsError,
    pub retryable: bool,
    pub message: String,
}
impl AccountsFailure {
    /// Constructs a constant diagnostic without incorporating external input.
    pub fn new(kind: AccountsError) -> Self {
        Self {
            kind,
            retryable: matches!(
                kind,
                AccountsError::SecureStorageUnavailable
                    | AccountsError::ProviderUnavailable
                    | AccountsError::StorageUnavailable
            ),
            message: format!("Account operation failed: {kind:?}"),
        }
    }
}
impl std::fmt::Display for AccountsFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for AccountsFailure {}
/// Lookup by stable account identity.
pub struct GetAccountRequest {
    pub account_id: AccountId,
}
/// Account lookup result.
#[derive(Debug)]
pub struct GetAccountResult {
    pub account: Account,
}
/// Optional provider filter; None selects all services.
pub struct ListAccountsRequest {
    pub provider: Option<ProviderId>,
}
/// Accounts in stable ID order.
#[derive(Debug)]
pub struct ListAccountsResult {
    pub accounts: Vec<Account>,
}
/// Local presentation update; None clears the display name.
pub struct UpdateAccountRequest {
    pub account_id: AccountId,
    pub display_name: Option<String>,
}
/// Updated metadata with unchanged external identity.
#[derive(Debug)]
pub struct UpdateAccountResult {
    pub account: Account,
}
/// Snapshot request, with no dependency access.
pub struct AccountsStatusRequest {}
/// Active operations in this instance only.
#[derive(Debug)]
pub struct AccountsStatusResult {
    pub active_operations: Vec<ActiveAccountOperation>,
}
/// Monitored asynchronous operation category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountOperationKind {
    Adding,
    Removing,
    AcquiringSession,
    UpdatingAuthorization,
}
/// Current execution phase; not a completion percentage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountOperationPhase {
    Validating,
    CheckingAssociations,
    AccessingSecureStorage,
    PersistingCredentials,
    PersistingMetadata,
    Finalizing,
}
/// Secret-free, transient activity snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveAccountOperation {
    pub account_id: Option<AccountId>,
    pub request_id: Option<String>,
    pub kind: AccountOperationKind,
    pub phase: AccountOperationPhase,
}

/// Authorized addition; correlation is optional and must not contain secrets.
pub struct AddAccountRequest {
    pub provider: ProviderId,
    pub request_id: Option<String>,
    pub authorization: super::ProviderAuthorizationHandle,
}
/// A repeated verified identity returns created=false without replacing credentials.
#[derive(Debug)]
pub struct AddAccountResult {
    pub account: Account,
    pub created: bool,
}
/// Remove local account data after obtaining external association exclusion.
pub struct RemoveAccountRequest {
    pub account_id: AccountId,
}
/// False means the account was already absent.
#[derive(Debug)]
pub struct RemoveAccountResult {
    pub removed: bool,
}
/// Obtain a provider session for one account.
pub struct AcquireSessionRequest {
    pub account_id: AccountId,
}
/// A session without public credential fields.
#[derive(Debug)]
pub struct AcquireSessionResult {
    pub session: super::ProviderSessionHandle,
}
/// Replace authorization only after verifying the same external identity.
pub struct UpdateAuthorizationRequest {
    pub account_id: AccountId,
    pub authorization: super::ProviderAuthorizationHandle,
}
/// Metadata after successful secure publication.
#[derive(Debug)]
pub struct UpdateAuthorizationResult {
    pub account: Account,
}
