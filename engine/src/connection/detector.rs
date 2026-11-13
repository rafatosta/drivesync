//! Validated account context and Accounts-owned sessions for remote observation.
use super::*;
use crate::detectors::remote::{
    Cancellation, RemoteError, RemoteFailure, RemoteResult, RemoteSessionContext,
};
use std::future::Future;

fn account_failure(error: AccountsFailure) -> RemoteFailure {
    let kind = match error.kind {
        AccountsError::NotFound
        | AccountsError::AuthorizationFailed
        | AccountsError::CredentialMissing => RemoteError::AuthorizationRequired,
        AccountsError::IdentityMismatch => RemoteError::InvalidScope,
        AccountsError::UnsupportedProvider => RemoteError::UnsupportedProvider,
        _ => RemoteError::RemoteUnavailable,
    };
    let mut failure = RemoteFailure::new(kind);
    failure.retryable = error.retryable;
    failure
}
impl Engine {
    /// Reads local Accounts metadata only; this does not acquire credentials or perform HTTP.
    pub(crate) fn remote_session_context(
        &self,
        account: &AccountId,
    ) -> RemoteResult<RemoteSessionContext> {
        let account = self
            .inner
            .accounts
            .get(GetAccountRequest {
                account_id: account.clone(),
            })
            .map_err(account_failure)?
            .account;
        Ok(RemoteSessionContext {
            account_id: account.id,
            provider: account.provider,
        })
    }

    /// Provider-specific clients are obtained only by the selected specialist.
    pub(crate) fn google_drive_client(&self) -> GoogleDriveClient {
        self.inner.client.clone()
    }

    /// Runs a specialist query with a fresh, identity-checked Accounts session.
    /// Called only on a blocking detector worker. Cancellation drops HTTP work;
    /// credential transactions already in progress remain owned by Accounts.
    pub(crate) fn with_remote_session<T, F, Q>(
        &self,
        context: &RemoteSessionContext,
        cancellation: &Cancellation,
        query: F,
    ) -> RemoteResult<T>
    where
        T: Send + 'static,
        F: FnOnce(ProviderSessionHandle) -> Q + Send + 'static,
        Q: Future<Output = RemoteResult<T>> + Send + 'static,
    {
        cancellation.check()?;
        let inner = self.inner.clone();
        let context = context.clone();
        let cancel = cancellation.clone();
        self.runtime().block_on(async {
            self.runtime()
                .spawn(async move {
                    let operation = async {
                        let _lease = inner.ownership.gate.clone().read_owned().await;
                        let account = inner
                            .accounts
                            .get(GetAccountRequest {
                                account_id: context.account_id.clone(),
                            })
                            .map_err(account_failure)?
                            .account;
                        if account.provider != context.provider || account.id != context.account_id
                        {
                            return Err(RemoteFailure::new(RemoteError::InvalidScope));
                        }
                        if account.state == AccountState::ReauthorizationRequired {
                            return Err(RemoteFailure::new(RemoteError::AuthorizationRequired));
                        }
                        let session = inner
                            .accounts
                            .acquire_session(AcquireSessionRequest {
                                account_id: context.account_id,
                            })
                            .await
                            .map_err(account_failure)?;
                        query(session.session).await
                    };
                    tokio::select! { biased;
                        _ = cancel.cancelled() => Err(RemoteFailure::new(RemoteError::Cancelled)),
                        result = operation => result,
                    }
                })
                .await
                .map_err(|_| RemoteFailure::new(RemoteError::Internal))?
        })
    }
}
