//! Deterministic credential lifecycle and failure/recovery contracts.
use super::*;
#[tokio::test]
async fn acc_03_04_05_09_12_14_15_18_lifecycle() {
    let fixture = Fixture::new();
    let a = fixture.add("subject-a").await;
    assert_eq!(a.state, AccountState::Connected);
    let repeated = fixture
        .accounts
        .add(fixture.authorization("subject-a"))
        .await
        .unwrap();
    assert!(!repeated.created);
    assert_eq!(a.id, repeated.account.id);
    let b = fixture.add("subject-b").await;
    assert_ne!(a.id, b.id);
    assert_eq!(a.email, b.email);
    let updated = fixture
        .accounts
        .update(UpdateAccountRequest {
            account_id: a.id.clone(),
            display_name: None,
        })
        .unwrap()
        .account;
    assert_eq!(updated.display_name, None);
    assert_eq!(updated.provider_user_id, "subject-a");
    let session = fixture
        .accounts
        .acquire_session(AcquireSessionRequest {
            account_id: a.id.clone(),
        })
        .await
        .unwrap();
    assert!(!format!("{:?}", session.session).contains("secret"));
    let request = fixture.authorization("subject-b");
    assert_eq!(
        fixture
            .accounts
            .update_authorization(UpdateAuthorizationRequest {
                account_id: a.id.clone(),
                authorization: request.authorization
            })
            .await
            .unwrap_err()
            .kind,
        AccountsError::IdentityMismatch
    );
    let request = fixture.authorization("subject-a");
    let replaced = fixture
        .accounts
        .update_authorization(UpdateAuthorizationRequest {
            account_id: a.id.clone(),
            authorization: request.authorization,
        })
        .await
        .unwrap()
        .account;
    assert_eq!(a.id, replaced.id);
    assert_eq!(fixture.vault.entries.lock().unwrap().len(), 2);
    let bytes = std::fs::read(fixture.path.join("accounts.json")).unwrap();
    assert!(!String::from_utf8(bytes).unwrap().contains("secret"));
}

use crate::providers::google::GoogleTokenResponse;
use credentials::{Credentials, Vault};
use provider::{Access, Identity};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Condvar,
    },
};
#[derive(Default)]
struct Barrier {
    arrived: tokio::sync::Notify,
    state: Mutex<bool>,
    release: Condvar,
}
impl Barrier {
    fn wait(&self) {
        self.arrived.notify_one();
        let mut open = self.state.lock().unwrap();
        while !*open {
            open = self.release.wait(open).unwrap();
        }
    }
    fn open(&self) {
        *self.state.lock().unwrap() = true;
        self.release.notify_all();
    }
}
#[derive(Default)]
struct FakeVault {
    put_barrier: Mutex<Option<Arc<Barrier>>>,
    entries: Mutex<BTreeMap<String, Credentials>>,
    fail_get: AtomicBool,
    fail_put: AtomicBool,
    fail_delete: AtomicBool,
    barrier: Mutex<Option<Arc<Barrier>>>,
}
impl Vault for FakeVault {
    fn get(&self, key: &str) -> AccountsResult<Option<Credentials>> {
        let barrier = self.barrier.lock().unwrap().take();
        if let Some(b) = barrier {
            b.wait();
        }
        if self.fail_get.load(Ordering::SeqCst) {
            return Err(AccountsFailure::new(
                AccountsError::SecureStorageUnavailable,
            ));
        }
        Ok(self.entries.lock().unwrap().get(key).cloned())
    }
    fn put(&self, key: &str, value: &Credentials) -> AccountsResult<()> {
        let barrier = self.put_barrier.lock().unwrap().take();
        if let Some(b) = barrier {
            b.wait();
        }
        if self.fail_put.load(Ordering::SeqCst) {
            return Err(AccountsFailure::new(
                AccountsError::SecureStorageUnavailable,
            ));
        }
        self.entries
            .lock()
            .unwrap()
            .insert(key.into(), value.clone());
        Ok(())
    }
    fn delete(&self, key: &str) -> AccountsResult<()> {
        if self.fail_delete.load(Ordering::SeqCst) {
            return Err(AccountsFailure::new(
                AccountsError::SecureStorageUnavailable,
            ));
        }
        self.entries.lock().unwrap().remove(key);
        Ok(())
    }
}
#[derive(Default)]
struct FakeAccess {
    fail: AtomicBool,
    refresh_fail: AtomicBool,
    calls: AtomicUsize,
    barrier: Mutex<Option<Arc<Barrier>>>,
}
impl Access for FakeAccess {
    fn identity(&self, _: &tokio::runtime::Handle, access: &str) -> AccountsResult<Identity> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let barrier = self.barrier.lock().unwrap().take();
        if let Some(b) = barrier {
            b.wait();
        }
        if self.fail.load(Ordering::SeqCst) {
            return Err(AccountsFailure::new(AccountsError::ProviderUnavailable));
        }
        Ok(Identity {
            subject: access.strip_prefix("secret:").unwrap().into(),
            email: Some("shared@example.invalid".into()),
            name: Some("Presentation".into()),
        })
    }
    fn refresh(
        &self,
        _: &tokio::runtime::Handle,
        value: &Credentials,
    ) -> AccountsResult<GoogleTokenResponse> {
        if self.refresh_fail.load(Ordering::SeqCst) {
            return Err(AccountsFailure::new(AccountsError::AuthorizationFailed));
        }
        Ok(GoogleTokenResponse {
            access_token: format!("secret:{}", value.subject),
            token_type: "Bearer".into(),
            expires_in: 3600,
            ..Default::default()
        })
    }
}
struct Lease;
impl RemovalGuard for Lease {}
#[derive(Default)]
struct Associations {
    in_use: AtomicBool,
    barrier: Mutex<Option<Arc<Barrier>>>,
}
impl AccountAssociations for Associations {
    fn lock_for_removal(&self, _: &AccountId) -> AccountsResult<Box<dyn RemovalGuard>> {
        let barrier = self.barrier.lock().unwrap().take();
        if let Some(b) = barrier {
            b.wait();
        }
        if self.in_use.load(Ordering::SeqCst) {
            Err(AccountsFailure::new(AccountsError::AccountInUse))
        } else {
            Ok(Box::new(Lease))
        }
    }
}
struct Fixture {
    path: PathBuf,
    accounts: Accounts,
    vault: Arc<FakeVault>,
    access: Arc<FakeAccess>,
    associations: Arc<Associations>,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "accounts-unit-{}-{}-{}",
            std::process::id(),
            now().unwrap().unix_millis,
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let vault = Arc::new(FakeVault::default());
        let access = Arc::new(FakeAccess::default());
        let associations = Arc::new(Associations::default());
        let mut accounts = Accounts::new(AccountsConfig::new(
            path.clone(),
            "test.accounts",
            associations.clone(),
        ))
        .unwrap();
        accounts.vault = vault.clone();
        accounts.google = Some(access.clone());
        Self {
            path,
            accounts,
            vault,
            access,
            associations,
        }
    }
    fn authorization(&self, subject: &str) -> AddAccountRequest {
        AddAccountRequest {
            provider: ProviderId("google".into()),
            request_id: None,
            authorization: ProviderAuthorizationHandle::google(GoogleTokenResponse {
                access_token: format!("secret:{subject}"),
                token_type: "Bearer".into(),
                expires_in: 3600,
                refresh_token: Some(format!("refresh-secret:{subject}")),
                ..Default::default()
            }),
        }
    }
    async fn add(&self, subject: &str) -> Account {
        self.accounts
            .add(self.authorization(subject))
            .await
            .unwrap()
            .account
    }
    fn state(&self, id: &AccountId) -> AccountState {
        self.accounts
            .get(GetAccountRequest {
                account_id: id.clone(),
            })
            .unwrap()
            .account
            .state
    }
    fn key(&self, id: &AccountId) -> String {
        self.accounts.store.lock().unwrap().data.records[id]
            .key
            .clone()
    }
    fn empty_status(&self) {
        assert!(self
            .accounts
            .status(AccountsStatusRequest {})
            .active_operations
            .is_empty());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[tokio::test]
async fn acc_06_17_20_reject_invalid_and_unsupported_without_effects() {
    let f = Fixture::new();
    let mut request = f.authorization("a");
    request.authorization.tokens.access_token.clear();
    assert_eq!(
        f.accounts.add(request).await.unwrap_err().kind,
        AccountsError::AuthorizationFailed
    );
    f.access.fail.store(true, Ordering::SeqCst);
    assert_eq!(
        f.accounts.add(f.authorization("a")).await.unwrap_err().kind,
        AccountsError::ProviderUnavailable
    );
    f.access.fail.store(false, Ordering::SeqCst);
    let mut request = f.authorization("a");
    request.provider = ProviderId("example-no-oauth".into());
    request.authorization.provider = request.provider.clone();
    assert_eq!(
        f.accounts.add(request).await.unwrap_err().kind,
        AccountsError::UnsupportedProvider
    );
    assert!(f
        .accounts
        .list(ListAccountsRequest { provider: None })
        .unwrap()
        .accounts
        .is_empty());
    assert!(f.vault.entries.lock().unwrap().is_empty());
    f.empty_status();
}
#[tokio::test]
async fn acc_10_11_19_remove_in_use_repeat_and_recover() {
    let mut f = Fixture::new();
    let a = f.add("a").await;
    let sentinel = f.path.join("user-file");
    std::fs::write(&sentinel, b"preserve").unwrap();
    f.associations.in_use.store(true, Ordering::SeqCst);
    assert_eq!(
        f.accounts
            .remove(RemoveAccountRequest {
                account_id: a.id.clone()
            })
            .await
            .unwrap_err()
            .kind,
        AccountsError::AccountInUse
    );
    assert_eq!(f.state(&a.id), AccountState::Connected);
    assert_eq!(f.vault.entries.lock().unwrap().len(), 1);
    f.associations.in_use.store(false, Ordering::SeqCst);
    f.vault.fail_delete.store(true, Ordering::SeqCst);
    assert_eq!(
        f.accounts
            .remove(RemoveAccountRequest {
                account_id: a.id.clone()
            })
            .await
            .unwrap_err()
            .kind,
        AccountsError::SecureStorageUnavailable
    );
    assert_eq!(f.state(&a.id), AccountState::Unavailable);
    assert!(f.accounts.store.lock().unwrap().data.intent.is_some());
    f.reopen();
    f.vault.fail_delete.store(false, Ordering::SeqCst);
    // The next operation first finishes the interrupted removal.
    assert!(
        !f.accounts
            .remove(RemoveAccountRequest {
                account_id: a.id.clone()
            })
            .await
            .unwrap()
            .removed
    );
    assert!(f.vault.entries.lock().unwrap().is_empty());
    assert_eq!(std::fs::read(sentinel).unwrap(), b"preserve");
    let b = f.add("b").await;
    assert!(
        f.accounts
            .remove(RemoveAccountRequest { account_id: b.id })
            .await
            .unwrap()
            .removed
    );
    f.empty_status();
}
#[tokio::test]
async fn acc_13_refresh_preserves_refresh_token_and_identity() {
    let f = Fixture::new();
    let a = f.add("a").await;
    let key = f.key(&a.id);
    f.vault
        .entries
        .lock()
        .unwrap()
        .get_mut(&key)
        .unwrap()
        .expires_at = 0;
    let session = f
        .accounts
        .acquire_session(AcquireSessionRequest {
            account_id: a.id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(session.session.google().account_id, a.id.0);
    assert_eq!(f.vault.entries.lock().unwrap().len(), 1);
    let key = f.key(&a.id);
    assert_eq!(
        f.vault.entries.lock().unwrap()[&key].refresh.as_deref(),
        Some("refresh-secret:a")
    );
    f.vault
        .entries
        .lock()
        .unwrap()
        .get_mut(&key)
        .unwrap()
        .expires_at = 0;
    f.access.refresh_fail.store(true, Ordering::SeqCst);
    assert_eq!(
        f.accounts
            .acquire_session(AcquireSessionRequest {
                account_id: a.id.clone()
            })
            .await
            .unwrap_err()
            .kind,
        AccountsError::AuthorizationFailed
    );
    assert_eq!(f.state(&a.id), AccountState::ReauthorizationRequired);
    let request = f.authorization("a");
    f.accounts
        .update_authorization(UpdateAuthorizationRequest {
            account_id: a.id.clone(),
            authorization: request.authorization,
        })
        .await
        .unwrap();
    assert_eq!(f.state(&a.id), AccountState::Connected);
}
#[tokio::test]
async fn acc_16_write_failure_leaves_recoverable_intent() {
    let f = Fixture::new();
    f.vault.fail_put.store(true, Ordering::SeqCst);
    assert_eq!(
        f.accounts.add(f.authorization("a")).await.unwrap_err().kind,
        AccountsError::SecureStorageUnavailable
    );
    let accounts = f
        .accounts
        .list(ListAccountsRequest { provider: None })
        .unwrap()
        .accounts;
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].state, AccountState::Unavailable);
    assert!(f.accounts.store.lock().unwrap().data.intent.is_some());
    f.vault.fail_put.store(false, Ordering::SeqCst);
    let a = f.add("a").await;
    f.vault.fail_put.store(true, Ordering::SeqCst);
    let request = f.authorization("a");
    assert!(f
        .accounts
        .update_authorization(UpdateAuthorizationRequest {
            account_id: a.id.clone(),
            authorization: request.authorization
        })
        .await
        .is_err());
    f.vault.fail_put.store(false, Ordering::SeqCst);
    f.accounts
        .acquire_session(AcquireSessionRequest {
            account_id: a.id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(f.state(&a.id), AccountState::Connected);
    assert_eq!(f.vault.entries.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn acc_s02_s03_s07_s08_s09_s11_controlled_activity_and_cancellation() {
    let f = Fixture::new();
    let a = f.add("a").await;
    let barrier = Arc::new(Barrier::default());
    *f.access.barrier.lock().unwrap() = Some(barrier.clone());
    let owner = f.accounts.clone();
    let mut request = f.authorization("b");
    request.request_id = Some("correlation-b".into());
    let pending = tokio::spawn(async move { owner.add(request).await });
    barrier.arrived.notified().await;
    let status = f.accounts.status(AccountsStatusRequest {});
    assert_eq!(status.active_operations.len(), 1);
    assert_eq!(
        status.active_operations[0].request_id.as_deref(),
        Some("correlation-b")
    );
    assert_eq!(status.active_operations[0].account_id, None);
    assert_eq!(
        status.active_operations[0].phase,
        AccountOperationPhase::Validating
    );
    // A different request is visible independently even while waiting for the worker.
    let owner = f.accounts.clone();
    let id = a.id.clone();
    let other = tokio::spawn(async move {
        owner
            .acquire_session(AcquireSessionRequest { account_id: id })
            .await
    });
    tokio::task::yield_now().await;
    assert_eq!(
        f.accounts
            .status(AccountsStatusRequest {})
            .active_operations
            .len(),
        2
    );
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    assert_eq!(
        f.accounts
            .status(AccountsStatusRequest {})
            .active_operations
            .len(),
        1
    );
    barrier.open();
    other.await.unwrap().unwrap();
    f.empty_status();
    let barrier = Arc::new(Barrier::default());
    *f.vault.barrier.lock().unwrap() = Some(barrier.clone());
    f.vault.fail_get.store(true, Ordering::SeqCst);
    let owner = f.accounts.clone();
    let id = a.id.clone();
    let pending = tokio::spawn(async move {
        owner
            .acquire_session(AcquireSessionRequest { account_id: id })
            .await
    });
    barrier.arrived.notified().await;
    let status = f.accounts.status(AccountsStatusRequest {});
    assert_eq!(
        status.active_operations[0].phase,
        AccountOperationPhase::AccessingSecureStorage
    );
    assert_eq!(status.active_operations[0].account_id.as_ref(), Some(&a.id));
    assert_eq!(f.state(&a.id), AccountState::Connected);
    barrier.open();
    assert_eq!(
        pending.await.unwrap().unwrap_err().kind,
        AccountsError::SecureStorageUnavailable
    );
    f.empty_status();
    assert_eq!(f.state(&a.id), AccountState::Connected);
}

impl Fixture {
    fn reopen(&mut self) {
        let placeholder = Accounts::new(AccountsConfig::new(
            self.path.join("placeholder"),
            "test.placeholder",
            self.associations.clone(),
        ))
        .unwrap();
        drop(std::mem::replace(&mut self.accounts, placeholder));
        let mut accounts = Accounts::new(AccountsConfig::new(
            self.path.clone(),
            "test.accounts",
            self.associations.clone(),
        ))
        .unwrap();
        accounts.vault = self.vault.clone();
        accounts.google = Some(self.access.clone());
        self.accounts = accounts;
    }
}
#[tokio::test]
async fn acc_16_18_restart_reconciles_both_persistence_boundaries() {
    let mut f = Fixture::new();
    let a = f.add("a").await;
    f.reopen();
    assert_eq!(
        f.accounts
            .get(GetAccountRequest {
                account_id: a.id.clone()
            })
            .unwrap()
            .account,
        a
    );
    let before_key = f.key(&a.id);
    // Write succeeds, but old-generation cleanup fails. The intent must survive reopening.
    f.vault.fail_delete.store(true, Ordering::SeqCst);
    let request = f.authorization("a");
    assert!(f
        .accounts
        .update_authorization(UpdateAuthorizationRequest {
            account_id: a.id.clone(),
            authorization: request.authorization
        })
        .await
        .is_err());
    assert_eq!(f.vault.entries.lock().unwrap().len(), 2);
    assert_eq!(f.state(&a.id), AccountState::Unavailable);
    f.reopen();
    assert_eq!(f.state(&a.id), AccountState::Unavailable);
    f.vault.fail_delete.store(false, Ordering::SeqCst);
    f.accounts
        .acquire_session(AcquireSessionRequest {
            account_id: a.id.clone(),
        })
        .await
        .unwrap();
    assert_ne!(f.key(&a.id), before_key);
    assert_eq!(f.vault.entries.lock().unwrap().len(), 1);
    assert_eq!(f.state(&a.id), AccountState::Connected);
    // Failed new credential creation rolls back to the original account after restart.
    f.vault.fail_put.store(true, Ordering::SeqCst);
    assert!(f.accounts.add(f.authorization("b")).await.is_err());
    f.reopen();
    f.vault.fail_put.store(false, Ordering::SeqCst);
    f.accounts
        .acquire_session(AcquireSessionRequest {
            account_id: a.id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(
        f.accounts
            .list(ListAccountsRequest { provider: None })
            .unwrap()
            .accounts
            .len(),
        1
    );
}
#[tokio::test]
async fn acc_12_missing_and_mismatched_credentials_fail_closed() {
    let f = Fixture::new();
    let a = f.add("a").await;
    let b = f.add("b").await;
    let key_a = f.key(&a.id);
    let key_b = f.key(&b.id);
    let other = f.vault.entries.lock().unwrap()[&key_b].clone();
    let original = f
        .vault
        .entries
        .lock()
        .unwrap()
        .insert(key_a.clone(), other)
        .unwrap();
    assert_eq!(
        f.accounts
            .acquire_session(AcquireSessionRequest {
                account_id: a.id.clone()
            })
            .await
            .unwrap_err()
            .kind,
        AccountsError::IdentityMismatch
    );
    f.vault
        .entries
        .lock()
        .unwrap()
        .insert(key_a.clone(), original);
    f.vault.entries.lock().unwrap().remove(&key_a);
    assert_eq!(
        f.accounts
            .acquire_session(AcquireSessionRequest {
                account_id: a.id.clone()
            })
            .await
            .unwrap_err()
            .kind,
        AccountsError::CredentialMissing
    );
    assert_eq!(f.state(&a.id), AccountState::ReauthorizationRequired);
    assert_eq!(f.state(&b.id), AccountState::Connected);
}
#[tokio::test]
async fn acc_s04_s06_remove_and_reauthorization_phases() {
    let f = Fixture::new();
    let a = f.add("a").await;
    let barrier = Arc::new(Barrier::default());
    *f.access.barrier.lock().unwrap() = Some(barrier.clone());
    let owner = f.accounts.clone();
    let request = f.authorization("a");
    let id = a.id.clone();
    let pending = tokio::spawn(async move {
        owner
            .update_authorization(UpdateAuthorizationRequest {
                account_id: id,
                authorization: request.authorization,
            })
            .await
    });
    barrier.arrived.notified().await;
    let status = f.accounts.status(AccountsStatusRequest {});
    assert_eq!(
        status.active_operations[0].kind,
        AccountOperationKind::UpdatingAuthorization
    );
    assert_eq!(status.active_operations[0].account_id.as_ref(), Some(&a.id));
    barrier.open();
    pending.await.unwrap().unwrap();
    f.empty_status();
    let barrier = Arc::new(Barrier::default());
    *f.associations.barrier.lock().unwrap() = Some(barrier.clone());
    f.associations.in_use.store(true, Ordering::SeqCst);
    let owner = f.accounts.clone();
    let id = a.id.clone();
    let pending =
        tokio::spawn(async move { owner.remove(RemoveAccountRequest { account_id: id }).await });
    barrier.arrived.notified().await;
    let status = f.accounts.status(AccountsStatusRequest {});
    assert_eq!(
        status.active_operations[0].kind,
        AccountOperationKind::Removing
    );
    assert_eq!(
        status.active_operations[0].phase,
        AccountOperationPhase::CheckingAssociations
    );
    barrier.open();
    assert_eq!(
        pending.await.unwrap().unwrap_err().kind,
        AccountsError::AccountInUse
    );
    f.empty_status();
}
#[tokio::test]
async fn acc_09_local_updates_do_not_call_dependencies() {
    let f = Fixture::new();
    let a = f.add("a").await;
    let calls = f.access.calls.load(Ordering::SeqCst);
    f.vault.fail_get.store(true, Ordering::SeqCst);
    let updated = f
        .accounts
        .update(UpdateAccountRequest {
            account_id: a.id.clone(),
            display_name: Some("New name".into()),
        })
        .unwrap()
        .account;
    assert_eq!(updated.email, a.email);
    assert_eq!(updated.id, a.id);
    assert_eq!(updated.created_at, a.created_at);
    assert_eq!(
        f.accounts
            .update(UpdateAccountRequest {
                account_id: a.id.clone(),
                display_name: Some(" ".into())
            })
            .unwrap_err()
            .kind,
        AccountsError::InvalidRequest
    );
    assert_eq!(
        f.accounts
            .list(ListAccountsRequest {
                provider: Some(ProviderId("google".into()))
            })
            .unwrap()
            .accounts
            .len(),
        1
    );
    assert!(f
        .accounts
        .list(ListAccountsRequest {
            provider: Some(ProviderId("other".into()))
        })
        .unwrap()
        .accounts
        .is_empty());
    assert_eq!(f.access.calls.load(Ordering::SeqCst), calls);
    f.empty_status();
}
/// This opt-in test requires the private bus created by check-vault-isolated.py.
#[tokio::test]
#[ignore = "requires an isolated Secret Service, never the desktop keyring"]
async fn isolated_accounts_vault_roundtrip() {
    let root = std::env::var("DRIVESYNC_TEST_VAULT_ROOT").expect("private vault root required");
    let bus = std::env::var("DBUS_SESSION_BUS_ADDRESS").expect("private bus required");
    assert_eq!(bus, format!("unix:path={root}/bus"));
    let f = Fixture::new();
    let mut owner = f.accounts.clone();
    owner.vault = Arc::new(credentials::Keyring {
        application: format!("test.drivesync.accounts.{}", std::process::id()),
    });
    let a = owner
        .add(f.authorization("isolated-a"))
        .await
        .unwrap()
        .account;
    let result = owner
        .acquire_session(AcquireSessionRequest {
            account_id: a.id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(result.session.google().account_id, a.id.0);
    let request = f.authorization("isolated-a");
    owner
        .update_authorization(UpdateAuthorizationRequest {
            account_id: a.id.clone(),
            authorization: request.authorization,
        })
        .await
        .unwrap();
    let key = f.key(&a.id);
    owner
        .remove(RemoveAccountRequest { account_id: a.id })
        .await
        .unwrap();
    let vault = owner.vault.clone();
    assert!(tokio::task::spawn_blocking(move || vault.get(&key))
        .await
        .unwrap()
        .unwrap()
        .is_none());
}

#[path = "../../tests/google_provider/support.rs"]
#[allow(dead_code)]
mod http_support;
#[tokio::test]
async fn google_adapter_uses_only_existing_provider_operations() {
    use crate::providers::google::{GoogleDriveClient, GoogleOAuthConfig};
    use http_support::{Reply, Server};
    let server = Server::start(vec![
        Reply::json(200, r#"{"sub":"a","email":"verified@example.invalid"}"#),
        Reply::json(
            200,
            r#"{"access_token":"secret:a","token_type":"Bearer","expires_in":3600}"#,
        ),
        Reply::json(200, r#"{"sub":"a"}"#),
    ]);
    let mut f = Fixture::new();
    let client = GoogleDriveClient::new()
        .with_oauth_base(&server.base)
        .unwrap();
    f.accounts.google = Some(Arc::new(provider::GoogleAccess {
        client,
        config: GoogleOAuthConfig {
            client_id: "synthetic-client".into(),
            ..Default::default()
        },
    }));
    let a = f.add("a").await;
    assert_eq!(a.provider_user_id, "a");
    assert_eq!(a.email.as_deref(), Some("verified@example.invalid"));
    let key = f.key(&a.id);
    f.vault
        .entries
        .lock()
        .unwrap()
        .get_mut(&key)
        .unwrap()
        .expires_at = 0;
    f.accounts
        .acquire_session(AcquireSessionRequest { account_id: a.id })
        .await
        .unwrap();
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0].path(), "/v1/userinfo");
    assert_eq!(requests[0].header("authorization"), Some("Bearer secret:a"));
    assert_eq!(requests[1].method, "POST");
    assert_eq!(requests[1].path(), "/token");
    let body = String::from_utf8_lossy(&requests[1].body);
    assert!(body.contains("grant_type=refresh_token"));
    assert!(body.contains("refresh_token=refresh-secret%3Aa"));
    assert_eq!(requests[2].path(), "/v1/userinfo");
}
#[tokio::test]
async fn google_adapter_maps_provider_failures_without_secret_diagnostics() {
    use crate::providers::google::{GoogleDriveClient, GoogleOAuthConfig};
    use http_support::{Reply, Server};
    for (status, body, expected) in [
        (
            401,
            r#"{"error":{"message":"private-secret"}}"#,
            AccountsError::AuthorizationFailed,
        ),
        (503, "private-secret", AccountsError::ProviderUnavailable),
    ] {
        let server = Server::start(vec![Reply::json(status, body)]);
        let mut f = Fixture::new();
        f.accounts.google = Some(Arc::new(provider::GoogleAccess {
            client: GoogleDriveClient::new()
                .with_oauth_base(&server.base)
                .unwrap(),
            config: GoogleOAuthConfig::default(),
        }));
        let failure = f.accounts.add(f.authorization("a")).await.unwrap_err();
        assert_eq!(failure.kind, expected);
        assert!(!failure.message.contains("private-secret"));
        assert!(f
            .accounts
            .list(ListAccountsRequest { provider: None })
            .unwrap()
            .accounts
            .is_empty());
    }
}
#[tokio::test]
async fn repeated_add_missing_secret_marks_only_affected_account_for_reauthorization() {
    let f = Fixture::new();
    let a = f.add("a").await;
    let b = f.add("b").await;
    f.vault.entries.lock().unwrap().remove(&f.key(&a.id));
    assert_eq!(
        f.accounts.add(f.authorization("a")).await.unwrap_err().kind,
        AccountsError::CredentialMissing
    );
    assert_eq!(f.state(&a.id), AccountState::ReauthorizationRequired);
    assert_eq!(f.state(&b.id), AccountState::Connected);
}

#[tokio::test]
async fn acc_s03_metadata_failure_after_secure_write_recovers_after_restart() {
    let mut f = Fixture::new();
    let barrier = Arc::new(Barrier::default());
    *f.vault.put_barrier.lock().unwrap() = Some(barrier.clone());
    let owner = f.accounts.clone();
    let request = f.authorization("a");
    let pending = tokio::spawn(async move { owner.add(request).await });
    barrier.arrived.notified().await;
    let status = f.accounts.status(AccountsStatusRequest {});
    assert_eq!(
        status.active_operations[0].phase,
        AccountOperationPhase::PersistingCredentials
    );
    assert!(status.active_operations[0].account_id.is_some());
    assert!(!format!("{status:?}").contains("secret:"));
    assert_eq!(
        f.accounts
            .list(ListAccountsRequest { provider: None })
            .unwrap()
            .accounts[0]
            .state,
        AccountState::Unavailable
    );
    // The nonsecret intent is already durable. Fail the final metadata publication.
    std::fs::create_dir(f.path.join("accounts.next")).unwrap();
    barrier.open();
    assert_eq!(
        pending.await.unwrap().unwrap_err().kind,
        AccountsError::StorageUnavailable
    );
    assert_eq!(f.vault.entries.lock().unwrap().len(), 1);
    f.empty_status();
    std::fs::remove_dir(f.path.join("accounts.next")).unwrap();
    f.reopen();
    let repeated = f.accounts.add(f.authorization("a")).await.unwrap();
    assert!(!repeated.created);
    assert_eq!(repeated.account.state, AccountState::Connected);
    assert_eq!(f.vault.entries.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn recovery_does_not_restore_connected_when_previous_secret_is_missing() {
    let f = Fixture::new();
    let a = f.add("a").await;
    f.vault.fail_put.store(true, Ordering::SeqCst);
    let request = f.authorization("a");
    assert!(f
        .accounts
        .update_authorization(UpdateAuthorizationRequest {
            account_id: a.id.clone(),
            authorization: request.authorization
        })
        .await
        .is_err());
    f.vault.entries.lock().unwrap().remove(&f.key(&a.id));
    f.vault.fail_put.store(false, Ordering::SeqCst);
    // An unrelated operation runs recovery, which must verify the rollback credential.
    f.add("b").await;
    assert_eq!(f.state(&a.id), AccountState::ReauthorizationRequired);
}
#[test]
fn metadata_rejects_inconsistent_transaction_intent_without_overwriting() {
    let mut f = Fixture::new();
    let mut data = f.accounts.store.lock().unwrap().data.clone();
    data.intent = Some(storage::Intent {
        before: None,
        after: None,
    });
    f.accounts.store.lock().unwrap().save(data).unwrap();
    let placeholder = Accounts::new(AccountsConfig::new(
        f.path.join("placeholder"),
        "test.placeholder",
        f.associations.clone(),
    ))
    .unwrap();
    drop(std::mem::replace(&mut f.accounts, placeholder));
    let before = std::fs::read(f.path.join("accounts.json")).unwrap();
    let result = Accounts::new(AccountsConfig::new(
        f.path.clone(),
        "test.accounts",
        f.associations.clone(),
    ));
    assert!(result.is_err());
    assert_eq!(before, std::fs::read(f.path.join("accounts.json")).unwrap());
}

#[test]
fn storage_rejects_linked_lock_and_staging_files() {
    let f = Fixture::new();
    let victim = f.path.join("preserve-me");
    std::fs::write(&victim, b"unchanged").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&victim, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::hard_link(&victim, f.path.join("accounts.next")).unwrap();
    let store = &mut *f.accounts.store.lock().unwrap();
    assert!(store.save(store.data.clone()).is_err());
    assert_eq!(std::fs::read(&victim).unwrap(), b"unchanged");
}

#[tokio::test]
async fn recovery_status_does_not_mix_another_accounts_identity() {
    let f = Fixture::new();
    f.vault.fail_put.store(true, Ordering::SeqCst);
    assert!(f
        .accounts
        .add(f.authorization("interrupted-a"))
        .await
        .is_err());
    f.vault.fail_put.store(false, Ordering::SeqCst);
    let barrier = Arc::new(Barrier::default());
    *f.access.barrier.lock().unwrap() = Some(barrier.clone());
    let owner = f.accounts.clone();
    let request = f.authorization("new-b");
    let pending = tokio::spawn(async move { owner.add(request).await });
    barrier.arrived.notified().await;
    let status = f.accounts.status(AccountsStatusRequest {});
    // Always release before asserting, so a regression cannot strand the worker.
    barrier.open();
    pending.await.unwrap().unwrap();
    assert_eq!(status.active_operations[0].account_id, None);
    assert_eq!(
        status.active_operations[0].phase,
        AccountOperationPhase::Validating
    );
}
#[tokio::test]
async fn cancellation_during_secure_write_finishes_before_later_session_access() {
    let f = Fixture::new();
    let barrier = Arc::new(Barrier::default());
    *f.vault.put_barrier.lock().unwrap() = Some(barrier.clone());
    let owner = f.accounts.clone();
    let request = f.authorization("a");
    let pending = tokio::spawn(async move { owner.add(request).await });
    barrier.arrived.notified().await;
    let id = f
        .accounts
        .status(AccountsStatusRequest {})
        .active_operations[0]
        .account_id
        .clone()
        .unwrap();
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    f.empty_status();
    barrier.open();
    let session = f
        .accounts
        .acquire_session(AcquireSessionRequest {
            account_id: id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(session.session.google().account_id, id.0);
    assert_eq!(f.state(&id), AccountState::Connected);
    f.empty_status();
    assert!(f.accounts.store.lock().unwrap().data.intent.is_none());
}

#[tokio::test]
async fn acc_08_multiple_provider_metadata_is_readable_without_authentication() {
    let f = Fixture::new();
    let a = f.add("a").await;
    let mut alternative = a.clone();
    alternative.id = AccountId("external-account".into());
    alternative.provider = ProviderId("example-no-oauth".into());
    alternative.state = AccountState::ReauthorizationRequired;
    {
        let mut store = f.accounts.store.lock().unwrap();
        let mut data = store.data.clone();
        data.records.insert(
            alternative.id.clone(),
            storage::Record {
                account: alternative.clone(),
                key: "example-no-oauth:external-account:generation".into(),
            },
        );
        store.save(data).unwrap();
    }
    f.vault.fail_get.store(true, Ordering::SeqCst);
    f.access.fail.store(true, Ordering::SeqCst);
    let result = f
        .accounts
        .list(ListAccountsRequest {
            provider: Some(alternative.provider.clone()),
        })
        .unwrap();
    assert_eq!(result.accounts, vec![alternative]);
    assert_eq!(
        f.accounts
            .list(ListAccountsRequest { provider: None })
            .unwrap()
            .accounts
            .len(),
        2
    );
    assert_eq!(
        f.accounts
            .list(ListAccountsRequest {
                provider: Some(ProviderId("google".into()))
            })
            .unwrap()
            .accounts,
        vec![a]
    );
    f.empty_status();
}

#[tokio::test]
async fn optional_refresh_token_allows_access_and_preserves_existing_renewal() {
    let f = Fixture::new();
    let mut request = f.authorization("access-only");
    request.authorization.tokens.refresh_token = None;
    let access_only = f.accounts.add(request).await.unwrap().account;
    f.accounts
        .acquire_session(AcquireSessionRequest {
            account_id: access_only.id.clone(),
        })
        .await
        .unwrap();
    let key = f.key(&access_only.id);
    f.vault
        .entries
        .lock()
        .unwrap()
        .get_mut(&key)
        .unwrap()
        .expires_at = 0;
    // A provider adapter, rather than Accounts protocol code, determines that
    // an access-only authorization cannot be renewed.
    f.access.refresh_fail.store(true, Ordering::SeqCst);
    assert_eq!(
        f.accounts
            .acquire_session(AcquireSessionRequest {
                account_id: access_only.id.clone()
            })
            .await
            .unwrap_err()
            .kind,
        AccountsError::AuthorizationFailed
    );
    f.access.refresh_fail.store(false, Ordering::SeqCst);
    let a = f.add("a").await;
    let mut request = f.authorization("a");
    request.authorization.tokens.refresh_token = None;
    f.accounts
        .update_authorization(UpdateAuthorizationRequest {
            account_id: a.id.clone(),
            authorization: request.authorization,
        })
        .await
        .unwrap();
    let key = f.key(&a.id);
    assert_eq!(
        f.vault.entries.lock().unwrap()[&key].refresh.as_deref(),
        Some("refresh-secret:a")
    );
}
