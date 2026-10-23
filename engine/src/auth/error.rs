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
    AccountStorage,
    Internal,
}
impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
                "Could not access the desktop credential vault. Unlock your keyring and try again."
            }
            Self::AccountStorage => "Could not read or update the account list. Please try again.",
            Self::Internal => {
                "Account connection could not be completed. Please restart DriveSync."
            }
        })
    }
}
impl std::error::Error for AuthError {}
