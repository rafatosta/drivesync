//! Sanitized authentication errors exposed to the engine and UI.
//!
//! Messages deliberately omit sensitive provider responses and credentials.

/// Errors that can occur during Google OAuth, secure storage and profile access.
/// These variants are safe to present without embedding tokens or server bodies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthError {
    Configuration,
    Busy,
    Cancelled,
    TimedOut,
    BrowserUnavailable,
    CallbackUnavailable,
    InvalidCallback,
    Denied,
    InvalidCode,
    Network,
    ProviderUnavailable,
    InvalidResponse,
    MissingPermissions,
    IdentityMismatch,
    ReauthorizationRequired,
    SecureStorage,
    /// A sanitized credential-vault failure with its operation and category.
    CredentialStorage {
        operation: VaultOperation,
        failure: VaultFailure,
    },
    AccountStorage,
    Internal,
}
impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Self::CredentialStorage { operation, failure } = self {
            return write!(
                f,
                "{} (credential vault: {operation:?}/{failure:?})",
                failure.message()
            );
        }
        f.write_str(match self {
            Self::Configuration => {
                "Google connection is unavailable in this build. Please install a configured DriveSync release or contact the distributor."
            }
            Self::Busy => "An account connection is already in progress.",
            Self::Cancelled => "Account connection was cancelled.",
            Self::TimedOut => "Google authorization timed out. Please try again.",
            Self::BrowserUnavailable => "Could not open the system browser. Please try again.",
            Self::CallbackUnavailable => "Could not receive the browser's authorization response.",
            Self::InvalidCallback => "The authorization response could not be verified.",
            Self::Denied => "Google authorization was declined.",
            Self::InvalidCode => {
                "The authorization code expired or was rejected. Please connect again."
            }
            Self::Network => "Could not reach Google. Check your connection and try again.",
            Self::ProviderUnavailable => {
                "Google is temporarily unavailable. Please try again later."
            }
            Self::InvalidResponse => "Google returned an unexpected authorization response.",
            Self::MissingPermissions => {
                "Required permissions were not granted. Please connect again."
            }
            Self::IdentityMismatch => "The credential does not belong to the requested account.",
            Self::ReauthorizationRequired => "The account needs to be connected again.",
            Self::SecureStorage => {
                "Could not access the desktop credential vault. Try again; if the problem persists, restart DriveSync."
            }
            Self::CredentialStorage { .. } => unreachable!(),
            Self::AccountStorage => "Could not read or update the account list. Please try again.",
            Self::Internal => {
                "Account connection could not be completed. Please restart DriveSync."
            }
        })
    }
}
impl std::error::Error for AuthError {}

/// Credential operation names safe for user-visible diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultOperation {
    Entry,
    Read,
    Decode,
    Encode,
    Write,
    Delete,
}

/// Allowlisted failure categories; never contain provider text or secrets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultFailure {
    Locked,
    PromptDismissed,
    AccessDenied,
    Unavailable,
    Timeout,
    InvalidSession,
    Crypto,
    InvalidData,
    Ambiguous,
    InvalidAttribute,
    Protocol,
    Platform,
}
impl VaultFailure {
    fn message(self) -> &'static str {
        match self {
            Self::Locked => "The credential vault is locked. Unlock it and try again.",
            Self::PromptDismissed => "Credential vault authorization was cancelled. Try again and complete the authorization prompt.",
            Self::AccessDenied => "Access to the credential vault was denied. Check application permissions and try again.",
            Self::Unavailable => "The credential vault service is unavailable. Restore the desktop credential service and try again.",
            Self::Timeout => "The credential vault did not respond in time. Try again.",
            Self::InvalidSession => "The credential vault session is no longer valid. Try again; if the problem persists, restart DriveSync.",
            Self::Crypto => "The credential vault could not securely exchange credentials. Try again; if the problem persists, report this error.",
            Self::InvalidData => "The saved credential could not be decoded. Existing credentials were preserved; report this error.",
            Self::Ambiguous => "Multiple matching credentials were found. Existing credentials were preserved; report this error.",
            Self::InvalidAttribute => "The credential vault rejected an entry attribute. Report this error.",
            Self::Protocol => "The credential vault returned an incompatible response. Report this error.",
            Self::Platform => "A credential vault operation failed. Try again; if the problem persists, report this error.",
        }
    }
}
