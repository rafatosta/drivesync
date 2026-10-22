//! Desktop keyring adapter for OAuth credentials.
//!
//! Credentials are scoped by OAuth client and account identity. Missing vault
//! entries are distinct from inaccessible or failed secure storage.
use super::{models::Credential, ports::CredentialStore, AuthError};
use crate::AccountId;

/// Production credential-store implementation backed by the system keyring.
pub(super) struct DesktopVault;
impl DesktopVault {
    fn entry(client: &str, id: &AccountId) -> Result<keyring::Entry, AuthError> {
        keyring::Entry::new(&format!("com.rtosta.DriveSync.Devel.oauth.{client}"), &id.0)
            .map_err(|_| AuthError::SecureStorage)
    }
}
impl CredentialStore for DesktopVault {
    fn load(&self, client: &str, id: &AccountId) -> Result<Option<Credential>, AuthError> {
        match Self::entry(client, id)?.get_password() {
            Ok(value) => serde_json::from_str(&value)
                .map(Some)
                .map_err(|_| AuthError::SecureStorage),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(AuthError::SecureStorage),
        }
    }
    fn save(&self, client: &str, id: &AccountId, value: &Credential) -> Result<(), AuthError> {
        let serialized = serde_json::to_string(value).map_err(|_| AuthError::SecureStorage)?;
        Self::entry(client, id)?
            .set_password(&serialized)
            .map_err(|_| AuthError::SecureStorage)
    }
    fn delete(&self, client: &str, id: &AccountId) -> Result<(), AuthError> {
        match Self::entry(client, id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(AuthError::SecureStorage),
        }
    }
}
