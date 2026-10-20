//! Minimal account registry. Credential ownership stays inside auth.
use crate::{AccountId, AccountSummary, AuthError};
use std::{collections::HashMap, sync::Mutex};

#[derive(Default)]
pub(crate) struct Accounts(Mutex<HashMap<AccountId, AccountSummary>>);
impl Accounts {
    pub fn record(&self, account: AccountSummary) -> Result<(), AuthError> {
        self.0
            .lock()
            .map_err(|_| AuthError::Internal)?
            .insert(account.id.clone(), account);
        Ok(())
    }
    pub fn list(&self) -> Result<Vec<AccountSummary>, AuthError> {
        let mut accounts: Vec<_> = self
            .0
            .lock()
            .map_err(|_| AuthError::Internal)?
            .values()
            .cloned()
            .collect();
        accounts.sort_by(|a, b| a.id.0.cmp(&b.id.0));
        Ok(accounts)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn same_subject_updates_profile_without_affecting_other_accounts() {
        let accounts = Accounts::default();
        let profile = |subject: &str, email: &str| AccountSummary {
            id: AccountId::from_subject(subject),
            provider_subject: subject.into(),
            email: Some(email.into()),
        };
        accounts.record(profile("a", "old@example.com")).unwrap();
        accounts.record(profile("b", "b@example.com")).unwrap();
        accounts.record(profile("a", "new@example.com")).unwrap();
        let list = accounts.list().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].email.as_deref(), Some("new@example.com"));
        assert_eq!(list[1].email.as_deref(), Some("b@example.com"));
    }
}
