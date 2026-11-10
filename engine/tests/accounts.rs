//! Public local account contracts, using a fresh private store per test.
use drivesync_engine::accounts::*;
use std::{path::PathBuf, sync::Arc};
struct RefuseRemoval;
impl AccountAssociations for RefuseRemoval {
    fn lock_for_removal(&self, _: &AccountId) -> AccountsResult<Box<dyn RemovalGuard>> {
        Err(AccountsFailure::new(AccountsError::AccountInUse))
    }
}
fn directory() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "accounts-public-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&path).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path
}
#[test]
fn acc_01_02_07_08_s01_s10_s12_local_contracts() {
    let path = directory();
    let config = || AccountsConfig::new(path.clone(), "test.accounts", Arc::new(RefuseRemoval));
    let accounts = Accounts::new(config()).unwrap();
    assert!(accounts
        .list(ListAccountsRequest { provider: None })
        .unwrap()
        .accounts
        .is_empty());
    assert!(accounts
        .status(AccountsStatusRequest {})
        .active_operations
        .is_empty());
    assert_eq!(
        accounts
            .get(GetAccountRequest {
                account_id: AccountId("missing".into())
            })
            .unwrap_err()
            .kind,
        AccountsError::NotFound
    );
    assert!(Accounts::new(config()).is_err());
    drop(accounts);
    let reopened = Accounts::new(config()).unwrap();
    assert!(reopened
        .list(ListAccountsRequest {
            provider: Some(ProviderId("other".into()))
        })
        .unwrap()
        .accounts
        .is_empty());
    drop(reopened);
    std::fs::write(path.join("accounts.json"), b"invalid sensitive content").unwrap();
    let error = Accounts::new(config()).err().unwrap();
    assert_eq!(error.kind, AccountsError::StorageUnavailable);
    assert!(!error.message.contains("sensitive"));
    assert_eq!(
        std::fs::read(path.join("accounts.json")).unwrap(),
        b"invalid sensitive content"
    );
    std::fs::remove_dir_all(path).unwrap();
}
