//! Credential transactions and transient observability, isolated from GTK.
use super::*;
use credentials::Credentials;
use provider::{Access, Identity};
use std::{
    collections::BTreeMap,
    io::Read,
    sync::atomic::{AtomicU64, Ordering},
};
use storage::{Intent, Record};

#[derive(Default)]
pub(super) struct Activities {
    next: AtomicU64,
    entries: Mutex<BTreeMap<u64, ActiveAccountOperation>>,
}
impl Activities {
    pub fn snapshot(&self) -> Vec<ActiveAccountOperation> {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .cloned()
            .collect()
    }
    fn begin(
        self: &Arc<Self>,
        kind: AccountOperationKind,
        account_id: Option<AccountId>,
        request_id: Option<String>,
    ) -> Activity {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                id,
                ActiveAccountOperation {
                    account_id,
                    request_id,
                    kind,
                    phase: AccountOperationPhase::Validating,
                },
            );
        Activity {
            owner: self.clone(),
            id,
        }
    }
    fn phase(&self, id: u64, phase: AccountOperationPhase, account: Option<&AccountId>) {
        if let Some(entry) = self
            .entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_mut(&id)
        {
            entry.phase = phase;
            if let Some(account) = account {
                entry.account_id = Some(account.clone());
            }
        }
    }
}
/// Caller-owned RAII guard clears status even if its future is dropped.
struct Activity {
    owner: Arc<Activities>,
    id: u64,
}
impl Drop for Activity {
    fn drop(&mut self) {
        self.owner
            .entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.id);
    }
}
fn error(kind: AccountsError) -> AccountsFailure {
    AccountsFailure::new(kind)
}
fn unique() -> AccountsResult<String> {
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|_| error(AccountsError::Internal))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
impl Accounts {
    /// Runs an entire durable unit on a blocking worker. Dropping the caller
    /// does not interrupt an uncertain keyring write; the worker retains the
    /// mutation lock until recovery/publication is finished.
    async fn run<T: Send + 'static>(
        &self,
        kind: AccountOperationKind,
        account: Option<AccountId>,
        request: Option<String>,
        work: impl FnOnce(Self, u64, tokio::runtime::Handle) -> AccountsResult<T> + Send + 'static,
    ) -> AccountsResult<T> {
        let runtime =
            tokio::runtime::Handle::try_current().map_err(|_| error(AccountsError::Internal))?;
        let activity = self.activities.begin(kind, account, request);
        let id = activity.id;
        let owner = self.clone();
        let result = tokio::task::spawn_blocking(move || {
            let _exclusive = owner
                .gate
                .lock()
                .map_err(|_| error(AccountsError::Internal))?;
            owner.recover(id)?;
            work(owner.clone(), id, runtime)
        })
        .await
        .map_err(|_| error(AccountsError::Internal))?;
        drop(activity);
        result
    }
    fn phase(&self, id: u64, phase: AccountOperationPhase, account: Option<&AccountId>) {
        self.activities.phase(id, phase, account);
    }
    fn record(&self, account: &AccountId) -> AccountsResult<Record> {
        self.store
            .lock()
            .map_err(|_| storage::storage_error())?
            .data
            .records
            .get(account)
            .cloned()
            .ok_or_else(|| error(AccountsError::NotFound))
    }
    fn access(&self, provider: &ProviderId) -> AccountsResult<Arc<dyn Access>> {
        if provider.0 != "google" {
            return Err(error(AccountsError::UnsupportedProvider));
        }
        self.google
            .clone()
            .ok_or_else(|| error(AccountsError::UnsupportedProvider))
    }
    /// Writes the nonsecret intent before touching credentials. All local readers
    /// see Unavailable until the transaction is reconciled.
    fn prepare(
        &self,
        before: Option<Record>,
        after: Option<Record>,
        operation: u64,
    ) -> AccountsResult<()> {
        self.phase(operation, AccountOperationPhase::PersistingMetadata, None);
        let mut store = self.store.lock().map_err(|_| storage::storage_error())?;
        let mut data = store.data.clone();
        let mut visible = before
            .as_ref()
            .or(after.as_ref())
            .cloned()
            .ok_or_else(|| error(AccountsError::Internal))?;
        visible.account.state = AccountState::Unavailable;
        data.records.insert(visible.account.id.clone(), visible);
        data.intent = Some(Intent { before, after });
        store.save(data)
    }
    /// Completes a write-ahead intent. Old credentials are deleted only after a
    /// validated replacement exists; failed cleanup retains the durable intent.
    fn recover(&self, operation: u64) -> AccountsResult<()> {
        let intent = self
            .store
            .lock()
            .map_err(|_| storage::storage_error())?
            .data
            .intent
            .clone();
        let Some(intent) = intent else {
            return Ok(());
        };
        let identity = intent
            .before
            .as_ref()
            .or(intent.after.as_ref())
            .ok_or_else(storage::storage_error)?
            .account
            .id
            .clone();
        self.phase(
            operation,
            AccountOperationPhase::AccessingSecureStorage,
            Some(&identity),
        );
        let _association_guard = if intent.after.is_none() {
            Some(self.associations.lock_for_removal(&identity)?)
        } else {
            None
        };
        let selected = if let Some(after) = &intent.after {
            match self.vault.get(&after.key)? {
                Some(value) if value.matches(after) => {
                    if let Some(before) = &intent.before {
                        self.vault.delete(&before.key)?;
                    }
                    Some(after.clone())
                }
                Some(_) => return Err(error(AccountsError::IdentityMismatch)),
                None => intent.before.clone(),
            }
        } else {
            if let Some(before) = &intent.before {
                self.vault.delete(&before.key)?;
            }
            None
        };
        self.phase(
            operation,
            AccountOperationPhase::PersistingMetadata,
            Some(&identity),
        );
        let mut store = self.store.lock().map_err(|_| storage::storage_error())?;
        let mut data = store.data.clone();
        data.records.remove(&identity);
        if let Some(record) = selected {
            data.records.insert(identity, record);
        }
        data.intent = None;
        store.save(data)?;
        self.phase(operation, AccountOperationPhase::Finalizing, None);
        Ok(())
    }
    fn persist(
        &self,
        before: Option<Record>,
        after: Record,
        credentials: Credentials,
        operation: u64,
    ) -> AccountsResult<Account> {
        self.prepare(before, Some(after.clone()), operation)?;
        self.phase(
            operation,
            AccountOperationPhase::PersistingCredentials,
            Some(&after.account.id),
        );
        self.vault.put(&after.key, &credentials)?;
        self.recover(operation)?;
        Ok(after.account)
    }
    fn validate(
        &self,
        provider: &ProviderId,
        authorization: ProviderAuthorizationHandle,
        runtime: &tokio::runtime::Handle,
    ) -> AccountsResult<(Identity, crate::providers::google::GoogleTokenResponse)> {
        if &authorization.provider != provider {
            return Err(error(AccountsError::IdentityMismatch));
        }
        let access = self.access(provider)?;
        let tokens = authorization.tokens;
        if tokens.access_token.is_empty()
            || !tokens.token_type.eq_ignore_ascii_case("Bearer")
            || tokens.expires_in == 0
            || tokens.refresh_token.as_ref().is_none_or(|r| r.is_empty())
        {
            return Err(error(AccountsError::AuthorizationFailed));
        }
        let identity = access.identity(runtime, &tokens.access_token)?;
        if identity.subject.trim().is_empty() {
            return Err(error(AccountsError::AuthorizationFailed));
        }
        Ok((identity, tokens))
    }
    fn material(
        record: &Record,
        tokens: crate::providers::google::GoogleTokenResponse,
    ) -> AccountsResult<Credentials> {
        let duration = i64::try_from(tokens.expires_in)
            .ok()
            .and_then(|v| v.checked_mul(1000))
            .ok_or_else(|| error(AccountsError::AuthorizationFailed))?;
        Ok(Credentials {
            account_id: record.account.id.clone(),
            provider: record.account.provider.clone(),
            subject: record.account.provider_user_id.clone(),
            access: tokens.access_token,
            refresh: tokens.refresh_token,
            expires_at: now()?
                .unix_millis
                .checked_add(duration)
                .ok_or_else(|| error(AccountsError::AuthorizationFailed))?,
        })
    }
    /// Verifies remote identity using Providers, then publishes metadata only
    /// after secure persistence. Repeated identities preserve existing secrets.
    pub async fn add(&self, request: AddAccountRequest) -> AccountsResult<AddAccountResult> {
        self.run(
            AccountOperationKind::Adding,
            None,
            request.request_id,
            move |owner, op, runtime| {
                let (identity, tokens) =
                    owner.validate(&request.provider, request.authorization, &runtime)?;
                let existing = owner
                    .store
                    .lock()
                    .map_err(|_| storage::storage_error())?
                    .data
                    .records
                    .values()
                    .find(|r| {
                        r.account.provider == request.provider
                            && r.account.provider_user_id == identity.subject
                    })
                    .cloned();
                if let Some(record) = existing {
                    owner.phase(
                        op,
                        AccountOperationPhase::AccessingSecureStorage,
                        Some(&record.account.id),
                    );
                    let secret = owner
                        .vault
                        .get(&record.key)?
                        .ok_or_else(|| error(AccountsError::CredentialMissing))?;
                    if !secret.matches(&record) {
                        return Err(error(AccountsError::IdentityMismatch));
                    }
                    return Ok(AddAccountResult {
                        account: record.account,
                        created: false,
                    });
                }
                let time = now()?;
                let account = Account {
                    id: AccountId(unique()?),
                    provider: request.provider,
                    provider_user_id: identity.subject,
                    email: identity.email,
                    display_name: identity.name,
                    state: AccountState::Connected,
                    created_at: time,
                    updated_at: time,
                };
                owner.phase(op, AccountOperationPhase::Validating, Some(&account.id));
                let record = Record {
                    key: format!("{}:{}:{}", account.provider.0, account.id.0, unique()?),
                    account,
                };
                let credentials = Self::material(&record, tokens)?;
                Ok(AddAccountResult {
                    account: owner.persist(None, record, credentials, op)?,
                    created: true,
                })
            },
        )
        .await
    }
    /// Replaces credentials for the same verified service identity. A failed
    /// verification leaves the previous credential generation untouched.
    pub async fn update_authorization(
        &self,
        request: UpdateAuthorizationRequest,
    ) -> AccountsResult<UpdateAuthorizationResult> {
        self.run(
            AccountOperationKind::UpdatingAuthorization,
            Some(request.account_id.clone()),
            None,
            move |owner, op, runtime| {
                let before = owner.record(&request.account_id)?;
                let (identity, tokens) =
                    owner.validate(&before.account.provider, request.authorization, &runtime)?;
                if identity.subject != before.account.provider_user_id {
                    return Err(error(AccountsError::IdentityMismatch));
                }
                let mut after = before.clone();
                after.key = format!(
                    "{}:{}:{}",
                    after.account.provider.0,
                    after.account.id.0,
                    unique()?
                );
                after.account.email = identity.email;
                after.account.state = AccountState::Connected;
                after.account.updated_at = now()?;
                let credentials = Self::material(&after, tokens)?;
                Ok(UpdateAuthorizationResult {
                    account: owner.persist(Some(before), after, credentials, op)?,
                })
            },
        )
        .await
    }
    /// Retrieves encapsulated authorization. Expiration delegates renewal to
    /// Providers; rotated credentials are persisted before the new session escapes.
    pub async fn acquire_session(
        &self,
        request: AcquireSessionRequest,
    ) -> AccountsResult<AcquireSessionResult> {
        self.run(
            AccountOperationKind::AcquiringSession,
            Some(request.account_id.clone()),
            None,
            move |owner, op, runtime| {
                let before = owner.record(&request.account_id)?;
                let access = owner.access(&before.account.provider)?;
                owner.phase(
                    op,
                    AccountOperationPhase::AccessingSecureStorage,
                    Some(&request.account_id),
                );
                let Some(mut value) = owner.vault.get(&before.key)? else {
                    owner.mark_reauthorization(&before)?;
                    return Err(error(AccountsError::CredentialMissing));
                };
                if !value.matches(&before) {
                    return Err(error(AccountsError::IdentityMismatch));
                }
                if before.account.state == AccountState::ReauthorizationRequired {
                    return Err(error(AccountsError::AuthorizationFailed));
                }
                if value.expires_at <= now()?.unix_millis {
                    let mut tokens = match access.refresh(&runtime, &value) {
                        Ok(tokens) => tokens,
                        Err(e) => {
                            if e.kind == AccountsError::AuthorizationFailed {
                                owner.mark_reauthorization(&before)?;
                            }
                            return Err(e);
                        }
                    };
                    if tokens.access_token.is_empty()
                        || !tokens.token_type.eq_ignore_ascii_case("Bearer")
                        || tokens.expires_in == 0
                    {
                        return Err(error(AccountsError::AuthorizationFailed));
                    }
                    if tokens.refresh_token.is_none() {
                        tokens.refresh_token = value.refresh.clone();
                    }
                    let identity = access.identity(&runtime, &tokens.access_token)?;
                    if identity.subject != before.account.provider_user_id {
                        return Err(error(AccountsError::IdentityMismatch));
                    }
                    let mut after = before.clone();
                    after.key = format!(
                        "{}:{}:{}",
                        after.account.provider.0,
                        after.account.id.0,
                        unique()?
                    );
                    after.account.updated_at = now()?;
                    after.account.state = AccountState::Connected;
                    value = Self::material(&after, tokens)?;
                    owner.persist(Some(before), after, value.clone(), op)?;
                }
                Ok(AcquireSessionResult {
                    session: ProviderSessionHandle {
                        google: crate::providers::google::GoogleSession::new(
                            request.account_id.0,
                            value.access,
                        ),
                    },
                })
            },
        )
        .await
    }
    fn mark_reauthorization(&self, before: &Record) -> AccountsResult<()> {
        let mut store = self.store.lock().map_err(|_| storage::storage_error())?;
        let mut data = store.data.clone();
        let record = data
            .records
            .get_mut(&before.account.id)
            .ok_or_else(|| error(AccountsError::NotFound))?;
        record.account.state = AccountState::ReauthorizationRequired;
        record.account.updated_at = now()?;
        store.save(data)
    }
    /// Removes only local credentials and metadata while holding the external
    /// connection owner's exclusion lease. No file deletion or remote revocation.
    pub async fn remove(
        &self,
        request: RemoveAccountRequest,
    ) -> AccountsResult<RemoveAccountResult> {
        self.run(
            AccountOperationKind::Removing,
            Some(request.account_id.clone()),
            None,
            move |owner, op, _| {
                let before = match owner.record(&request.account_id) {
                    Ok(r) => r,
                    Err(e) if e.kind == AccountsError::NotFound => {
                        return Ok(RemoveAccountResult { removed: false })
                    }
                    Err(e) => return Err(e),
                };
                owner.phase(
                    op,
                    AccountOperationPhase::CheckingAssociations,
                    Some(&request.account_id),
                );
                let _lease = owner.associations.lock_for_removal(&request.account_id)?;
                owner.prepare(Some(before.clone()), None, op)?;
                owner.phase(
                    op,
                    AccountOperationPhase::AccessingSecureStorage,
                    Some(&request.account_id),
                );
                owner.vault.delete(&before.key)?;
                // Finish under the existing association guard (do not recursively lock it).
                owner.phase(op, AccountOperationPhase::PersistingMetadata, None);
                let mut store = owner.store.lock().map_err(|_| storage::storage_error())?;
                let mut data = store.data.clone();
                data.records.remove(&request.account_id);
                data.intent = None;
                store.save(data)?;
                owner.phase(op, AccountOperationPhase::Finalizing, None);
                Ok(RemoveAccountResult { removed: true })
            },
        )
        .await
    }
}
