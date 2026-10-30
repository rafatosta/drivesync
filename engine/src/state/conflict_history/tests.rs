use super::*;
use crate::{
    local::{Evidence, Inventory},
    AccountId, ChangeKind, ConflictOutcome, ReconciliationEntry,
};
use std::{collections::BTreeMap, fs, path::PathBuf};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "drivesync-conflict-history-{}",
            oauth2::CsrfToken::new_random().secret()
        ));
        fs::create_dir(&directory).unwrap();
        Self(directory)
    }

    fn db(&self) -> PathBuf {
        self.0.join("conflict-history.sqlite3")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn scope<'a>(account: &'a AccountId, local: &'a std::path::Path) -> CheckpointScope<'a> {
    CheckpointScope {
        client: "client",
        account,
        remote: "root",
        local,
    }
}

fn reconciliation() -> super::super::Reconciliation {
    let remote = crate::RemoteInventory {
        account_id: AccountId("account".into()),
        root_id: "root".into(),
        entries: vec![crate::InventoryEntry {
            id: "file".into(),
            parent_id: "root".into(),
            name: "item.txt".into(),
            mime_type: "text/plain".into(),
            version: Some(3),
            size: Some(4),
            md5_checksum: Some("098f6bcd4621d373cade4e832627b4f6".into()),
            web_view_link: None,
            kind: crate::InventoryKind::Binary,
        }],
    };
    let plan = crate::planner::plan(&remote, Path::new("/files"), |_| {
        crate::LocalObservation::Unknown
    })
    .unwrap();
    super::super::Reconciliation {
        local: Inventory {
            entries: BTreeMap::from([(
                PathBuf::from("item.txt"),
                Evidence::Stable {
                    fingerprint: "hash:1:2".into(),
                    stamp: crate::local::Stamp {
                        device: 1,
                        inode: 2,
                        size: 4,
                        mtime: (1, 0),
                        ctime: (1, 0),
                    },
                },
            )]),
        },
        remote,
        plan,
        bases: Default::default(),
        history: vec![],
        entries: vec![ReconciliationEntry {
            relative_path: Some(PathBuf::from("item.txt")),
            remote_id: Some("file".into()),
            kind: ChangeKind::ConcurrentChanges,
        }],
    }
}

#[test]
fn pending_records_and_audit_events_survive_reopen_and_deduplicate() {
    let fixture = Fixture::new();
    let account = AccountId("account".into());
    let local = Path::new("/files");
    let account_scope = scope(&account, local);
    let record = reconciliation();

    ConflictHistory::open(&fixture.db())
        .unwrap()
        .record_pending(&account_scope, &record, &Cancellation::new())
        .unwrap();
    let mut history = ConflictHistory::open(&fixture.db()).unwrap();
    assert_eq!(history.events(&account_scope).unwrap().len(), 1);
    assert_eq!(
        history.records(&account_scope).unwrap()[0].remote_version,
        Some(3)
    );
    assert_eq!(
        history.records(&account_scope).unwrap()[0]
            .remote_name
            .as_deref(),
        Some("item.txt")
    );
    assert_eq!(
        history.records(&account_scope).unwrap()[0]
            .remote_parent_id
            .as_deref(),
        Some("root")
    );
    assert_eq!(
        history.records(&account_scope).unwrap()[0]
            .local_fingerprint
            .as_deref(),
        Some("hash:1:2")
    );

    history
        .record_pending(&account_scope, &record, &Cancellation::new())
        .unwrap();
    assert_eq!(history.events(&account_scope).unwrap().len(), 1);

    let copy_path = PathBuf::from("DriveSync backup conflict-file.bak");
    history
        .record_outcome(
            &account_scope,
            Some("file"),
            Some(Path::new("item.txt")),
            ConflictOutcome::CopyPreserved {
                copy_path: copy_path.clone(),
                remote_version: 3,
            },
        )
        .unwrap();
    history
        .record_outcome(
            &account_scope,
            Some("file"),
            Some(Path::new("item.txt")),
            ConflictOutcome::RetryBlocked {
                reason: crate::ConflictFailure::RemoteUnavailable,
            },
        )
        .unwrap();
    let reopened = ConflictHistory::open(&fixture.db()).unwrap();
    assert_eq!(reopened.events(&account_scope).unwrap().len(), 3);
    assert_eq!(
        reopened.records(&account_scope).unwrap()[0].outcomes,
        vec![
            ConflictOutcome::CopyPreserved {
                copy_path,
                remote_version: 3,
            },
            ConflictOutcome::RetryBlocked {
                reason: crate::ConflictFailure::RemoteUnavailable,
            }
        ]
    );

    let mut reopened = reopened;
    reopened
        .record_pending(&account_scope, &record, &Cancellation::new())
        .unwrap();
    assert_eq!(reopened.events(&account_scope).unwrap().len(), 3);
    reopened
        .record_outcome(
            &account_scope,
            Some("file"),
            Some(Path::new("item.txt")),
            ConflictOutcome::CopyPreserved {
                copy_path: PathBuf::from("DriveSync backup conflict-file.bak"),
                remote_version: 3,
            },
        )
        .unwrap();
    assert_eq!(reopened.events(&account_scope).unwrap().len(), 3);

    let mut settled = record.clone();
    settled.entries[0].kind = ChangeKind::Equivalent;
    reopened
        .record_pending(&account_scope, &settled, &Cancellation::new())
        .unwrap();
    assert!(reopened.records(&account_scope).unwrap().is_empty());
    reopened
        .record_pending(&account_scope, &record, &Cancellation::new())
        .unwrap();
    assert_eq!(reopened.records(&account_scope).unwrap().len(), 1);
    assert_eq!(reopened.events(&account_scope).unwrap().len(), 5);
}

#[test]
fn scopes_are_isolated_and_failed_event_commit_rolls_back_record() {
    let fixture = Fixture::new();
    let account = AccountId("account".into());
    let local = Path::new("/files");
    let account_scope = scope(&account, local);
    let mut history = ConflictHistory::open(&fixture.db()).unwrap();
    history
        .0
        .0
        .execute_batch(
            "CREATE TRIGGER refuse_event BEFORE INSERT ON events BEGIN SELECT RAISE(ABORT,'injected'); END;",
        )
        .unwrap();
    assert_eq!(
        history.record_pending(&account_scope, &reconciliation(), &Cancellation::new()),
        Err(EngineError::StateStorage)
    );
    assert!(history.records(&account_scope).unwrap().is_empty());
    assert!(history.events(&account_scope).unwrap().is_empty());
    history
        .0
         .0
        .execute_batch("DROP TRIGGER refuse_event")
        .unwrap();
    history
        .record_pending(&account_scope, &reconciliation(), &Cancellation::new())
        .unwrap();

    let other = AccountId("other".into());
    let other_scope = scope(&other, local);
    assert!(history.records(&other_scope).unwrap().is_empty());
    assert!(history.events(&other_scope).unwrap().is_empty());
}
