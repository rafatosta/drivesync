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
        bulk_population: Default::default(),
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

fn observation(version: u64, fingerprint: &str) -> crate::ResolutionObservation {
    crate::ResolutionObservation {
        relative_path: PathBuf::from("item.txt"),
        remote: crate::RemoteState::Present {
            id: "file".into(),
            parent_id: "root".into(),
            name: "item.txt".into(),
            version,
        },
        local: crate::LocalState::Present {
            fingerprint: fingerprint.into(),
        },
    }
}

#[test]
fn stale_resolution_is_aborted_audited_idempotent_and_survives_reopen() {
    use crate::{LocalState, RemoteState, StaleResolutionReason as S};
    let fixture = Fixture::new();
    let account = AccountId("account".into());
    let local = Path::new("/files");
    let account_scope = scope(&account, local);
    let mut history = ConflictHistory::open(&fixture.db()).unwrap();
    history
        .record_pending(&account_scope, &reconciliation(), &Cancellation::new())
        .unwrap();
    let id = history.records(&account_scope).unwrap()[0]
        .conflict_id
        .clone();

    let mut check = |current| {
        history
            .revalidate_resolution(&account_scope, &id, &current)
            .unwrap()
    };
    assert_eq!(check(observation(3, "hash:1:2")), Ok(()));
    assert_eq!(
        check(observation(4, "hash:1:2")),
        Err(S::RemoteRevisionChanged)
    );
    assert_eq!(
        check(observation(4, "hash:1:2")),
        Err(S::RemoteRevisionChanged)
    );
    assert_eq!(check(observation(3, "other:1:2")), Err(S::LocalChanged));
    let mut moved = observation(3, "hash:1:2");
    moved.remote = RemoteState::Unknown;
    assert_eq!(check(moved), Err(S::EvidenceUnavailable));
    let mut missing = observation(3, "hash:1:2");
    missing.local = LocalState::Missing;
    assert_eq!(check(missing), Err(S::LocalMissing));

    let reopened = ConflictHistory::open(&fixture.db()).unwrap();
    let records = reopened.records(&account_scope).unwrap();
    assert_eq!(records[0].status, crate::ConflictStatus::Pending);
    assert_eq!(records[0].outcomes.len(), 4);
    let aborted = reopened
        .events(&account_scope)
        .unwrap()
        .into_iter()
        .filter(|event| {
            matches!(
                event.kind,
                crate::ConflictHistoryKind::ResolutionAborted { .. }
            )
        })
        .count();
    assert_eq!(aborted, 4);
}

#[test]
fn resolution_for_unknown_or_settled_conflict_is_rejected_without_events() {
    let fixture = Fixture::new();
    let account = AccountId("account".into());
    let local = Path::new("/files");
    let account_scope = scope(&account, local);
    let mut history = ConflictHistory::open(&fixture.db()).unwrap();
    history
        .record_pending(&account_scope, &reconciliation(), &Cancellation::new())
        .unwrap();
    let id = history.records(&account_scope).unwrap()[0]
        .conflict_id
        .clone();
    let mut settled = reconciliation();
    settled.entries[0].kind = ChangeKind::Equivalent;
    history
        .record_pending(&account_scope, &settled, &Cancellation::new())
        .unwrap();
    let events = history.events(&account_scope).unwrap().len();
    for target in [id.as_str(), "unknown"] {
        assert_eq!(
            history
                .revalidate_resolution(&account_scope, target, &observation(3, "hash:1:2"))
                .unwrap(),
            Err(crate::StaleResolutionReason::NotPending)
        );
    }
    assert_eq!(history.events(&account_scope).unwrap().len(), events);

    let other = AccountId("other".into());
    assert_eq!(
        history
            .revalidate_resolution(&scope(&other, local), &id, &observation(3, "hash:1:2"))
            .unwrap(),
        Err(crate::StaleResolutionReason::NotPending)
    );
}

/// Access problems remain pending offline, across reopen and repeated observation.
#[test]
fn c22_c25_access_problems_remain_pending_after_restart_and_replay() {
    let fixture = Fixture::new();
    let account = AccountId("account".into());
    let local_file = fixture.0.join("item.txt");
    fs::write(&local_file, b"local work").unwrap();
    let account_scope = scope(&account, &fixture.0);
    for kind in [
        ChangeKind::Unknown,
        ChangeKind::RemoteUnavailable,
        ChangeKind::LocalChangedRemoteUnavailable,
    ] {
        let mut observation = reconciliation();
        observation.plan.local_root = fixture.0.clone();
        observation.entries[0].kind = kind.clone();
        let mut history = ConflictHistory::open(&fixture.db()).unwrap();
        history
            .record_pending(&account_scope, &observation, &Cancellation::new())
            .unwrap();
        let count = history.events(&account_scope).unwrap().len();
        drop(history);
        let mut reopened = ConflictHistory::open(&fixture.db()).unwrap();
        let records = reopened.records(&account_scope).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].status, crate::ConflictStatus::Pending);
        assert_eq!(records[0].diagnosis, kind);
        assert_eq!(records[0].local_size, Some(4));
        assert_eq!(records[0].local_modified_unix, Some(1));
        assert_eq!(records[0].remote_size, Some(4));
        assert!(records[0].outcomes.is_empty());
        reopened
            .record_pending(&account_scope, &observation, &Cancellation::new())
            .unwrap();
        assert_eq!(reopened.events(&account_scope).unwrap().len(), count);
        assert_eq!(fs::read(&local_file).unwrap(), b"local work");
    }
}

#[test]
fn old_conflict_format_is_rejected_without_resetting_evidence() {
    let fixture = Fixture::new();
    let history = ConflictHistory::open(&fixture.db()).unwrap();
    history.0 .0.pragma_update(None, "user_version", 1).unwrap();
    drop(history);
    let bytes = fs::read(fixture.db()).unwrap();
    assert!(ConflictHistory::open(&fixture.db()).is_err());
    assert_eq!(fs::read(fixture.db()).unwrap(), bytes);
}
