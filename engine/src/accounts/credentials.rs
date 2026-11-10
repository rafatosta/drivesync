//! Private encrypted keyring access. No plaintext fallback and no desktop prompts.
use super::*;
use secret_service::{blocking::SecretService, EncryptionType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
/// Private credential envelope binds secure material to the account identity.
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Credentials {
    pub account_id: AccountId,
    pub provider: ProviderId,
    pub subject: String,
    pub access: String,
    pub refresh: Option<String>,
    pub expires_at: i64,
}
impl Credentials {
    pub fn matches(&self, record: &storage::Record) -> bool {
        self.account_id == record.account.id
            && self.provider == record.account.provider
            && self.subject == record.account.provider_user_id
            && !self.access.is_empty()
    }
}
pub(super) trait Vault: Send + Sync {
    fn get(&self, key: &str) -> AccountsResult<Option<Credentials>>;
    fn put(&self, key: &str, value: &Credentials) -> AccountsResult<()>;
    fn delete(&self, key: &str) -> AccountsResult<()>;
}
pub(super) struct Keyring {
    pub application: String,
}
fn unavailable() -> AccountsFailure {
    AccountsFailure::new(AccountsError::SecureStorageUnavailable)
}
impl Keyring {
    fn attributes<'a>(&'a self, key: &'a str) -> HashMap<&'a str, &'a str> {
        HashMap::from([
            ("application", self.application.as_str()),
            ("account-credential", key),
        ])
    }
}
impl Vault for Keyring {
    fn get(&self, key: &str) -> AccountsResult<Option<Credentials>> {
        let service = SecretService::connect(EncryptionType::Dh).map_err(|_| unavailable())?;
        let found = service
            .search_items(self.attributes(key))
            .map_err(|_| unavailable())?;
        if !found.locked.is_empty() || found.unlocked.len() > 1 {
            return Err(unavailable());
        }
        found
            .unlocked
            .first()
            .map(|item| {
                let bytes = item.get_secret().map_err(|_| unavailable())?;
                serde_json::from_slice(&bytes)
                    .map_err(|_| AccountsFailure::new(AccountsError::CredentialMissing))
            })
            .transpose()
    }
    fn put(&self, key: &str, value: &Credentials) -> AccountsResult<()> {
        let service = SecretService::connect(EncryptionType::Dh).map_err(|_| unavailable())?;
        let collection = service
            .get_default_collection()
            .map_err(|_| unavailable())?;
        collection.ensure_unlocked().map_err(|_| unavailable())?;
        let bytes = serde_json::to_vec(value).map_err(|_| unavailable())?;
        collection
            .create_item(
                "DriveSync account credentials",
                self.attributes(key),
                &bytes,
                true,
                "application/json",
            )
            .map_err(|_| unavailable())?;
        Ok(())
    }
    fn delete(&self, key: &str) -> AccountsResult<()> {
        let service = SecretService::connect(EncryptionType::Dh).map_err(|_| unavailable())?;
        let found = service
            .search_items(self.attributes(key))
            .map_err(|_| unavailable())?;
        if !found.locked.is_empty() {
            return Err(unavailable());
        }
        for item in found.unlocked {
            item.delete().map_err(|_| unavailable())?;
        }
        Ok(())
    }
}
