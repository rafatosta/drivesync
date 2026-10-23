//! Integrity checks for saved account and folder metadata.
//!
//! Index validation does not contact Google, the secret store or the local
//! filesystem. Runtime checks for overlapping local directories are handled
//! by the repository when configuring folders.
use super::models::Index;
use crate::{AccountId, AuthError};
use std::collections::HashSet;

/// Validates version, client IDs, account identity and folder references.
///
/// Rejects unsupported index versions, duplicate accounts or folder mappings,
/// malformed identities, invalid remote-folder metadata and nonabsolute paths.
/// This does not verify current authentication or filesystem availability.
///
/// # Errors
///
/// Returns [`AuthError::AccountStorage`] for any invalid index.
pub(super) fn validate(index: &Index) -> Result<(), AuthError> {
    if index.version != 2 {
        return Err(AuthError::AccountStorage);
    }
    for (client, profiles) in &index.clients {
        crate::auth::OAuthConfig {
            client_id: client.clone(),
            client_secret: None,
        }
        .validate()
        .map_err(|_| AuthError::AccountStorage)?;
        let mut ids = HashSet::new();
        for account in profiles {
            if account.provider_subject.is_empty()
                || account.provider_subject.len() > 255
                || account.provider_subject.chars().any(char::is_control)
                || account.id != AccountId::from_subject(&account.provider_subject)
                || !ids.insert(&account.id)
                || account
                    .email
                    .as_ref()
                    .is_some_and(|e| e.len() > 1024 || e.chars().any(char::is_control))
            {
                return Err(AuthError::AccountStorage);
            }
        }
    }
    for (client, folders) in &index.folders {
        let mut ids = HashSet::new();
        for folder in folders {
            if !index
                .clients
                .get(client)
                .is_some_and(|profiles| profiles.iter().any(|p| p.id == folder.account_id))
                || !folder.remote.valid()
                || !folder.local.is_absolute()
                || !folder.local_display.is_absolute()
                || !ids.insert(&folder.account_id)
            {
                return Err(AuthError::AccountStorage);
            }
        }
    }
    for (client, descriptions) in &index.descriptions {
        for (id, description) in descriptions {
            if !index
                .clients
                .get(client)
                .is_some_and(|profiles| profiles.iter().any(|p| &p.id.0 == id))
                || description.is_empty()
                || description.trim() != description
                || description.chars().count() > 128
                || description.chars().any(char::is_control)
            {
                return Err(AuthError::AccountStorage);
            }
        }
    }
    Ok(())
}
