//! Desktop keyring adapter for OAuth credentials.
//!
//! Credentials are scoped by OAuth client and account identity. Missing vault
//! entries are distinct from inaccessible or failed secure storage.
use super::{models::Credential, ports::CredentialStore, AuthError, VaultFailure, VaultOperation};
use crate::AccountId;
use std::io::Write;

/// Production credential-store implementation backed by the system keyring.
pub(super) struct DesktopVault;
impl DesktopVault {
    fn entry(client: &str, id: &AccountId) -> Result<keyring::Entry, AuthError> {
        keyring::Entry::new(&format!("com.rtosta.DriveSync.Devel.oauth.{client}"), &id.0)
            .map_err(|error| keyring_failure(VaultOperation::Entry, error))
    }
}
impl CredentialStore for DesktopVault {
    fn load(&self, client: &str, id: &AccountId) -> Result<Option<Credential>, AuthError> {
        recover_read(|| match Self::entry(client, id)?.get_password() {
            Ok(value) => serde_json::from_str(&value)
                .map(Some)
                .map_err(|_| report_failure(VaultOperation::Decode, VaultFailure::InvalidData)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(keyring_failure(VaultOperation::Read, error)),
        })
    }
    fn save(&self, client: &str, id: &AccountId, value: &Credential) -> Result<(), AuthError> {
        let serialized = serde_json::to_string(value)
            .map_err(|_| report_failure(VaultOperation::Encode, VaultFailure::InvalidData))?;
        Self::entry(client, id)?
            .set_password(&serialized)
            .map_err(|error| keyring_failure(VaultOperation::Write, error))
    }
    fn delete(&self, client: &str, id: &AccountId) -> Result<(), AuthError> {
        match Self::entry(client, id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(keyring_failure(VaultOperation::Delete, error)),
        }
    }
}

/// Reopens a failed encrypted read once, using a fresh keyring entry/session.
///
/// Repeated reads of a fixed synthetic secret reproduce intermittent DH crypto
/// errors against oo7 on Fedora 45. Reading is idempotent; no remote operation or
/// vault write is replayed. Decode errors, absence, permissions and unknown
/// failures are never retried. The caller still verifies credential identity.
fn recover_read<T>(mut read: impl FnMut() -> Result<T, AuthError>) -> Result<T, AuthError> {
    let result = read();
    if matches!(
        result,
        Err(AuthError::CredentialStorage {
            operation: VaultOperation::Read,
            failure: VaultFailure::Crypto,
        })
    ) {
        let result = read();
        if result.is_ok() {
            let _ = writeln!(
                std::io::stderr(),
                "DriveSync credential-vault operation=Read recovery=FreshSession"
            );
        }
        result
    } else {
        result
    }
}

/// Emits only fixed operation/category names. Provider errors may contain secrets
/// or private entry attributes and must never be formatted, even in debug logs.
fn report_failure(operation: VaultOperation, failure: VaultFailure) -> AuthError {
    let error = AuthError::CredentialStorage { operation, failure };
    let _ = writeln!(
        std::io::stderr(),
        "DriveSync credential-vault operation={operation:?} category={failure:?}"
    );
    error
}

fn keyring_failure(operation: VaultOperation, error: keyring::Error) -> AuthError {
    report_failure(operation, classify(&error))
}

/// Classifies concrete types and allowlisted D-Bus names, never message text.
fn classify(error: &keyring::Error) -> VaultFailure {
    use keyring::Error;
    match error {
        Error::PlatformFailure(source) | Error::NoStorageAccess(source) => {
            if let Some(error) = source.downcast_ref::<dbus_secret_service::Error>() {
                return match error {
                    dbus_secret_service::Error::Locked => VaultFailure::Locked,
                    dbus_secret_service::Error::Prompt => VaultFailure::PromptDismissed,
                    dbus_secret_service::Error::Unavailable => VaultFailure::Unavailable,
                    dbus_secret_service::Error::Crypto(_) => VaultFailure::Crypto,
                    dbus_secret_service::Error::Dbus(error) => dbus_failure(error.name()),
                    dbus_secret_service::Error::Parse
                    | dbus_secret_service::Error::Path(_)
                    | dbus_secret_service::Error::UnsupportedSecretFormat => VaultFailure::Protocol,
                    _ => VaultFailure::Platform,
                };
            }
            VaultFailure::Platform
        }
        Error::BadEncoding(_) => VaultFailure::InvalidData,
        Error::Ambiguous(_) => VaultFailure::Ambiguous,
        Error::TooLong(_, _) | Error::Invalid(_, _) => VaultFailure::InvalidAttribute,
        _ => VaultFailure::Platform,
    }
}

fn dbus_failure(name: Option<&str>) -> VaultFailure {
    match name {
        Some("org.freedesktop.Secret.Error.IsLocked") => VaultFailure::Locked,
        Some("org.freedesktop.Secret.Error.NoSession") => VaultFailure::InvalidSession,
        Some("org.freedesktop.DBus.Error.AccessDenied")
        | Some("org.freedesktop.DBus.Error.AuthFailed") => VaultFailure::AccessDenied,
        Some("org.freedesktop.DBus.Error.NoReply") | Some("org.freedesktop.DBus.Error.Timeout") => {
            VaultFailure::Timeout
        }
        Some("org.freedesktop.DBus.Error.ServiceUnknown")
        | Some("org.freedesktop.DBus.Error.NameHasNoOwner")
        | Some("org.freedesktop.DBus.Error.Disconnected") => VaultFailure::Unavailable,
        _ => VaultFailure::Platform,
    }
}

#[cfg(test)]
mod tests;
