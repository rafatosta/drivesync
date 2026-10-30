//! Persistence, cancellation, isolation and rollback oracles for observation records.
use super::*;
use crate::{AccountId, ChangeKind, LocalObservation};
use std::{collections::BTreeMap, fs, path::PathBuf};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir()
            .join(format!(
                "drivesync-reconciliation-{}",
                oauth2::CsrfToken::new_random().secret()
            ))
            .join("observations.sqlite3");
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.0.parent().unwrap());
    }
}
fn record() -> Reconciliation {
    let remote = RemoteInventory {
        account_id: AccountId("account".into()),
        root_id: "root".into(),
        entries: vec![],
    };
    let plan =
        crate::planner::plan(&remote, Path::new("/files"), |_| LocalObservation::Unknown).unwrap();
    let local = Inventory {
        entries: BTreeMap::from([(PathBuf::from("new.txt"), Evidence::Unknown)]),
    };
    Reconciliation::new(local, remote, plan, HashMap::new(), vec![], None)
}
fn scope(account: &AccountId) -> CheckpointScope<'_> {
    CheckpointScope {
        client: "client",
        account,
        remote: "root",
        local: Path::new("/files"),
    }
}
#[test]
fn reopens_full_evidence_isolates_scopes_and_rejects_stale_workers() {
    let f = Fixture::new();
    let account = AccountId("account".into());
    let scope = scope(&account);
    let mut store = Reconciliations::open(&f.0).unwrap();
    let record = record();
    store
        .save(&scope, None, &record, &Cancellation::new())
        .unwrap();
    let (revision, loaded) = Reconciliations::open(&f.0)
        .unwrap()
        .load(&scope)
        .unwrap()
        .unwrap();
    assert_eq!(loaded, record);
    assert_eq!(loaded.entries[0].kind, ChangeKind::Unknown);
    let other = AccountId("other".into());
    for scope in [
        CheckpointScope {
            client: "other",
            ..scope
        },
        CheckpointScope {
            account: &other,
            ..scope
        },
        CheckpointScope {
            remote: "other",
            ..scope
        },
        CheckpointScope {
            local: Path::new("/other"),
            ..scope
        },
    ] {
        assert!(store.load(&scope).unwrap().is_none());
    }
    store
        .save(&scope, Some(&revision), &record, &Cancellation::new())
        .unwrap();
    assert_eq!(
        store.save(&scope, Some(&revision), &record, &Cancellation::new()),
        Err(EngineError::SyncBusy)
    );
    assert_eq!(store.load(&scope).unwrap().unwrap().1, record);
}
#[test]
fn failed_commit_and_cancellation_retain_the_previous_record() {
    let f = Fixture::new();
    let account = AccountId("account".into());
    let scope = scope(&account);
    let mut store = Reconciliations::open(&f.0).unwrap();
    let old = record();
    store
        .save(&scope, None, &old, &Cancellation::new())
        .unwrap();
    let (revision, _) = store.load(&scope).unwrap().unwrap();
    let next = Reconciliation::new(
        Inventory {
            entries: BTreeMap::new(),
        },
        old.remote.clone(),
        old.plan.clone(),
        HashMap::new(),
        vec![],
        Some(&old),
    );
    assert_eq!(next.entries[0].kind, ChangeKind::LocalMissing);
    let cancel = Cancellation::new();
    cancel.cancel();
    assert!(store.save(&scope, Some(&revision), &next, &cancel).is_err());
    store.0.0.execute_batch("CREATE TRIGGER refuse BEFORE UPDATE ON snapshots BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert_eq!(
        store.save(&scope, Some(&revision), &next, &Cancellation::new()),
        Err(EngineError::StateStorage)
    );
    assert_eq!(
        Reconciliations::open(&f.0)
            .unwrap()
            .load(&scope)
            .unwrap()
            .unwrap(),
        (revision.clone(), old)
    );
    store.0 .0.execute_batch("DROP TRIGGER refuse").unwrap();
    store
        .save(&scope, Some(&revision), &next, &Cancellation::new())
        .unwrap();
    assert_eq!(
        Reconciliations::open(&f.0)
            .unwrap()
            .load(&scope)
            .unwrap()
            .unwrap()
            .1,
        next
    );
}
#[test]
fn corrupt_incompatible_foreign_and_unsafe_evidence_are_not_reset() {
    let f = Fixture::new();
    let account = AccountId("account".into());
    let scope = scope(&account);
    let mut store = Reconciliations::open(&f.0).unwrap();
    let mut record = record();
    record
        .local
        .entries
        .insert(PathBuf::from("../outside"), Evidence::Unknown);
    assert_eq!(
        store.save(&scope, None, &record, &Cancellation::new()),
        Err(EngineError::InvalidPlan)
    );
    assert!(store.load(&scope).unwrap().is_none());
    let record = super::tests::record();
    store
        .save(&scope, None, &record, &Cancellation::new())
        .unwrap();
    store
        .0
         .0
        .execute("UPDATE snapshots SET record=?1", ["invalid JSON"])
        .unwrap();
    assert_eq!(store.load(&scope), Err(EngineError::StateStorage));
    assert_eq!(
        store
            .0
             .0
            .query_row("SELECT record FROM snapshots", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "invalid JSON"
    );
    store.0 .0.pragma_update(None, "user_version", 99).unwrap();
    drop(store);
    assert!(Reconciliations::open(&f.0).is_err());
    assert_eq!(
        rusqlite::Connection::open(&f.0)
            .unwrap()
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        99
    );
    let foreign = f.0.with_file_name("foreign.sqlite3");
    drop(Store::open(&foreign).unwrap());
    assert!(Reconciliations::open(&foreign).is_err());
    let link = f.0.with_file_name("link");
    std::os::unix::fs::symlink(&f.0, &link).unwrap();
    assert!(Reconciliations::open(&link).is_err());
}
#[test]
fn historical_remote_absence_never_becomes_a_new_upload_candidate() {
    let mut record = record();
    record.history = vec![Notice {
        deleted: false,
        id: "old".into(),
        path: Some(PathBuf::from("new.txt")),
    }];
    let a = crate::changes::classify(&record.local, &record.plan, &record.history, &record.bases);
    let b = crate::changes::classify(&record.local, &record.plan, &record.history, &record.bases);
    assert_eq!(a, b);
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].remote_id.as_deref(), Some("old"));
    assert_eq!(a[0].kind, ChangeKind::RemoteUnavailable);
}

#[test]
fn inaccessible_ancestor_does_not_turn_previous_descendants_into_missing_files() {
    let mut previous = record();
    previous.local.entries = BTreeMap::from([(PathBuf::from("nested/file"), Evidence::Unknown)]);
    let next = Reconciliation::new(
        Inventory {
            entries: BTreeMap::from([(PathBuf::from("nested"), Evidence::Unsupported)]),
        },
        previous.remote.clone(),
        previous.plan.clone(),
        HashMap::new(),
        vec![],
        Some(&previous),
    );
    assert_eq!(
        next.entries
            .iter()
            .find(|e| e.relative_path.as_deref() == Some(Path::new("nested/file")))
            .unwrap()
            .kind,
        ChangeKind::Unknown
    );
}

#[test]
fn protective_population_cannot_omit_current_confirmed_evidence() {
    let f = Fixture::new();
    let account = AccountId("account".into());
    let scope = scope(&account);
    let mut store = Reconciliations::open(&f.0).unwrap();
    let mut record = record();
    let base = ConfirmedBase {
        remote_version: 1,
        local_fingerprint: format!("{}:1:2", "a".repeat(64)),
        location: None,
    };
    record.bases.insert("file".into(), base.clone());
    assert_eq!(
        store.save(&scope, None, &record, &Cancellation::new()),
        Err(EngineError::InvalidPlan)
    );
    record.bulk_population.insert("file".into(), base);
    store
        .save(&scope, None, &record, &Cancellation::new())
        .unwrap();
    record.bulk_population.clear();
    store
        .0
         .0
        .execute(
            "UPDATE snapshots SET record=?1",
            [serde_json::to_string(&record).unwrap()],
        )
        .unwrap();
    assert_eq!(store.load(&scope), Err(EngineError::StateStorage));
}
