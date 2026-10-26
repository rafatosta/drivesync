use super::*;
use crate::{AccountId, Cancellation};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "drivesync-state-{}",
            oauth2::CsrfToken::new_random().secret()
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn db(&self) -> PathBuf {
        self.0.join("state.sqlite3")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn snapshot() -> (RemoteInventory, DownloadPlan) {
    let inv = RemoteInventory {
        account_id: AccountId("test".into()),
        root_id: "root".into(),
        entries: vec![],
    };
    let plan = crate::planner::plan(&inv, Path::new("/local"), |_| unreachable!()).unwrap();
    (inv, plan)
}
#[test]
fn persists_reopens_and_isolates_scopes() {
    let f = Fixture::new();
    let (inv, plan) = snapshot();
    Store::open(&f.db())
        .unwrap()
        .save("client", &inv, &plan, &Cancellation::new())
        .unwrap();
    let store = Store::open(&f.db()).unwrap();
    assert_eq!(
        store
            .load("client", &inv.account_id, "root", Path::new("/local"))
            .unwrap(),
        Some(plan)
    );
    for (client, account, remote, local) in [
        ("other", "test", "root", "/local"),
        ("client", "other", "root", "/local"),
        ("client", "test", "other", "/local"),
        ("client", "test", "root", "/other"),
    ] {
        assert!(store
            .load(client, &AccountId(account.into()), remote, Path::new(local))
            .unwrap()
            .is_none());
    }
}
#[test]
fn failed_transaction_preserves_previous_generation() {
    let f = Fixture::new();
    let (inv, plan) = snapshot();
    let mut store = Store::open(&f.db()).unwrap();
    store
        .save("client", &inv, &plan, &Cancellation::new())
        .unwrap();
    store
        .0
        .execute_batch(
            "CREATE TRIGGER fail BEFORE UPDATE ON roots BEGIN SELECT RAISE(ABORT,'injected'); END;",
        )
        .unwrap();
    assert!(store
        .save("client", &inv, &plan, &Cancellation::new())
        .is_err());
    let generation: i64 = store
        .0
        .query_row("SELECT generation FROM roots", [], |r| r.get(0))
        .unwrap();
    assert_eq!(generation, 1);
    assert_eq!(
        store
            .load("client", &inv.account_id, "root", Path::new("/local"))
            .unwrap(),
        Some(plan)
    );
}
#[test]
fn rejects_corruption_and_symlinks_without_replacing_data() {
    let f = Fixture::new();
    fs::write(f.db(), b"corrupt database").unwrap();
    assert!(Store::open(&f.db()).is_err());
    assert_eq!(fs::read(f.db()).unwrap(), b"corrupt database");
    let link = f.0.join("link");
    std::os::unix::fs::symlink(f.db(), &link).unwrap();
    assert!(Store::open(&link).is_err());
}

#[test]
fn retains_unobserved_items_without_turning_them_into_current_work() {
    let f = Fixture::new();
    let (mut inv, _) = snapshot();
    inv.entries.push(crate::InventoryEntry {
        id: "file".into(),
        parent_id: "root".into(),
        name: "file.txt".into(),
        mime_type: "text/plain".into(),
        version: Some(1),
        size: Some(3),
        md5_checksum: Some("hash".into()),
        web_view_link: None,
        kind: crate::InventoryKind::Binary,
    });
    let plan = crate::planner::plan(&inv, Path::new("/local"), |_| {
        crate::LocalObservation::Absent
    })
    .unwrap();
    let mut store = Store::open(&f.db()).unwrap();
    store
        .save("client", &inv, &plan, &Cancellation::new())
        .unwrap();
    assert_eq!(
        store
            .load("client", &inv.account_id, "root", Path::new("/local"))
            .unwrap(),
        Some(plan)
    );
    inv.entries.clear();
    let empty = crate::planner::plan(&inv, Path::new("/local"), |_| unreachable!()).unwrap();
    store
        .save("client", &inv, &empty, &Cancellation::new())
        .unwrap();
    assert_eq!(
        store
            .load("client", &inv.account_id, "root", Path::new("/local"))
            .unwrap(),
        Some(empty)
    );
    let retained: i64 = store
        .0
        .query_row("SELECT count(*) FROM observations", [], |r| r.get(0))
        .unwrap();
    assert_eq!(retained, 1);
}

#[test]
fn cancellation_and_foreign_schema_preserve_state_and_private_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let (inv, plan) = snapshot();
    let mut store = Store::open(&f.db()).unwrap();
    assert_eq!(
        fs::metadata(f.db()).unwrap().permissions().mode() & 0o777,
        0o600
    );
    store
        .save("client", &inv, &plan, &Cancellation::new())
        .unwrap();
    let cancel = Cancellation::new();
    cancel.cancel();
    assert!(store.save("client", &inv, &plan, &cancel).is_err());
    assert_eq!(
        store
            .0
            .query_row("SELECT generation FROM roots", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    store.0.pragma_update(None, "user_version", 99).unwrap();
    drop(store);
    assert!(Store::open(&f.db()).is_err());
    assert_eq!(
        Connection::open(f.db())
            .unwrap()
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        99
    );
}
