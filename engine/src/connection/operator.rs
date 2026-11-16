//! Accounts-owned transfer sessions. Mutating calls are never aborted by dropping
//! a UI future: their worker retains both the runtime owner and account read lease.
use super::*;
use crate::operations::{ExecutionEffect, OperatorError, OperatorFailure, OperatorResult};

pub(crate) struct OperatorAccess {
    pub client: GoogleDriveClient,
    account: AccountId,
    accounts: Option<Accounts>,
    _lease: Option<tokio::sync::OwnedRwLockReadGuard<()>>,
    #[cfg(test)]
    fixed: Option<GoogleSession>,
}
fn denied() -> OperatorFailure {
    OperatorFailure {
        kind: OperatorError::PermissionDenied,
        effect: ExecutionEffect::NotApplied,
        resource: None,
    }
}
impl OperatorAccess {
    /// Refresh through Accounts before each request, retaining an operation-wide
    /// removal lease. Credentials remain private and are never journaled.
    pub async fn session(&self) -> OperatorResult<GoogleSession> {
        #[cfg(test)]
        if let Some(session) = &self.fixed {
            return Ok(session.clone());
        }
        let accounts = self.accounts.as_ref().ok_or_else(denied)?;
        let account = accounts
            .get(GetAccountRequest {
                account_id: self.account.clone(),
            })
            .map_err(|error| {
                eprintln!(
                    "DriveSync Operator: account lookup failed: {:?}",
                    error.kind
                );
                denied()
            })?
            .account;
        if account.provider.0 != "google" || account.state == AccountState::ReauthorizationRequired
        {
            return Err(denied());
        }
        let session = accounts
            .acquire_session(AcquireSessionRequest {
                account_id: self.account.clone(),
            })
            .await
            .map_err(|error| {
                eprintln!(
                    "DriveSync Operator: session acquisition failed: {:?}",
                    error.kind
                );
                OperatorFailure {
                    kind: if error.retryable {
                        OperatorError::Network
                    } else {
                        OperatorError::PermissionDenied
                    },
                    ..denied()
                }
            })?;
        Ok(session.session.google().clone())
    }
    #[cfg(test)]
    pub(crate) fn testing(client: GoogleDriveClient, account: AccountId) -> Self {
        Self {
            client,
            fixed: Some(GoogleSession::new(account.0.clone(), "synthetic-token")),
            account,
            accounts: None,
            _lease: None,
        }
    }
}
impl Engine {
    pub(crate) fn operator_runtime(&self) -> tokio::runtime::Handle {
        self.runtime().handle().clone()
    }
    pub(crate) fn operator_provider(&self, account: &AccountId) -> OperatorResult<ProviderId> {
        self.inner
            .accounts
            .get(GetAccountRequest {
                account_id: account.clone(),
            })
            .map(|r| r.account.provider)
            .map_err(|_| denied())
    }
    pub(crate) async fn operator_access(
        &self,
        account: AccountId,
    ) -> OperatorResult<OperatorAccess> {
        let lease = self.inner.ownership.gate.clone().read_owned().await;
        if self.operator_provider(&account)?.0 != "google" {
            return Err(OperatorFailure {
                kind: OperatorError::UnsupportedProvider,
                ..denied()
            });
        }
        Ok(OperatorAccess {
            client: self.inner.client.clone(),
            account,
            accounts: Some(self.inner.accounts.clone()),
            _lease: Some(lease),
            #[cfg(test)]
            fixed: None,
        })
    }
}
