//! N08/P06 acceptance with the feed double, real isolated files and SQLite.
use super::*;
use crate::{BulkPolicy, BulkReview, ConfirmedBase, ConfirmedLocation, ReconciliationEntry};

fn many(count: usize) -> RemoteInventory {
    let mut inv = inventory();
    inv.entries = (0..count)
        .map(|i| {
            let mut item = inventory().entries.remove(0);
            item.id = format!("file-{i}");
            item.name = format!("file-{i}.txt");
            item
        })
        .collect();
    inv
}
fn setup(count: usize) -> (Fixture, Feed, Fake) {
    let f = Fixture::new();
    let inv = many(count);
    let mut feed = Feed {
        inventory: inv.clone(),
        events: vec![],
        offline: false,
    };
    let mut remote = Fake::new(&inv);
    assert_eq!(
        observed_cycle(&f, &mut feed, &mut remote)
            .unwrap()
            .downloaded,
        count
    );
    (f, feed, remote)
}
fn review(result: Result<DownloadReport, EngineError>) -> BulkReview {
    match result {
        Err(EngineError::BulkReviewRequired(review)) => review,
        other => panic!("expected review: {other:?}"),
    }
}
fn saved(f: &Fixture, feed: &Feed) -> Reconciliation {
    Reconciliations::open(&f.db().with_file_name("reconciliation.sqlite3"))
        .unwrap()
        .load(&CheckpointScope {
            client: "client",
            account: &feed.inventory.account_id,
            remote: "root",
            local: &f.root,
        })
        .unwrap()
        .unwrap()
        .1
}

#[test]
fn n08_p06_missing_batch_blocks_before_effects_survives_restart_and_requires_explicit_retry() {
    let (f, mut feed, mut remote) = setup(50);
    for i in 0..12 {
        fs::remove_file(f.root.join(format!("file-{i}.txt"))).unwrap();
    }
    let calls = remote.media_calls;
    let pending = review(observed_cycle(&f, &mut feed, &mut remote));
    assert_eq!(pending.impacts()[0].affected, 12);
    assert_eq!(pending.impacts()[0].population, 50);
    let record = saved(&f, &feed);
    let bases = record.bases.clone();
    assert_eq!(record.bases.len(), 50);
    assert_eq!(remote.media_calls, calls);
    assert_eq!(remote.content.len(), 50);
    assert_eq!(fs::read(f.root.join("file-49.txt")).unwrap(), b"abc");
    // Reopening all stores and repeating the scan cannot authorize restoration.
    assert_eq!(review(observed_cycle(&f, &mut feed, &mut remote)), pending);
    assert_eq!(saved(&f, &feed).bases, bases);
    assert_eq!(remote.media_calls, calls);
    assert_eq!(
        crate::coordinator::bulk_gate("client", &record, Some(&pending)),
        Ok(())
    );
    assert!(matches!(
        crate::coordinator::bulk_gate("another-client", &record, Some(&pending)),
        Err(EngineError::BulkReviewRequired(_))
    ));
    let result = observed_cycle_reviewed(&f, &mut feed, &mut remote, Some(&pending)).unwrap();
    assert_eq!(result.downloaded, 12);
    for i in 0..50 {
        assert_eq!(
            fs::read(f.root.join(format!("file-{i}.txt"))).unwrap(),
            b"abc"
        );
    }
    let calls = remote.media_calls;
    let repeated = observed_cycle(&f, &mut feed, &mut remote).unwrap();
    assert_eq!(repeated.downloaded + repeated.updated, 0);
    assert_eq!(remote.media_calls, calls);
}

#[test]
fn n08_offline_and_changed_evidence_never_reuse_a_proposal() {
    let (f, mut feed, mut remote) = setup(50);
    for i in 0..12 {
        fs::remove_file(f.root.join(format!("file-{i}.txt"))).unwrap();
    }
    let pending = review(observed_cycle(&f, &mut feed, &mut remote));
    let before = saved(&f, &feed);
    let calls = remote.media_calls;
    feed.offline = true;
    assert_eq!(
        observed_cycle_reviewed(&f, &mut feed, &mut remote, Some(&pending)).unwrap_err(),
        EngineError::Authentication(crate::AuthError::Network)
    );
    assert_eq!(saved(&f, &feed), before);
    feed.offline = false;
    fs::write(f.root.join("file-49.txt"), b"concurrent work").unwrap();
    let fresh = review(observed_cycle_reviewed(
        &f,
        &mut feed,
        &mut remote,
        Some(&pending),
    ));
    assert_ne!(fresh, pending);
    assert_eq!(remote.media_calls, calls);
    assert_eq!(
        fs::read(f.root.join("file-49.txt")).unwrap(),
        b"concurrent work"
    );
    // The same physical root cannot dispatch a second cycle under the first lock.
    let _lock = Root::open(&f.root).unwrap();
    assert_eq!(
        observed_cycle_reviewed(&f, &mut feed, &mut remote, Some(&fresh)).unwrap_err(),
        EngineError::SyncBusy
    );
}

#[test]
fn n08_p06_4800_of_5000_and_subfolder_impact_are_not_diluted_or_duplicated() {
    let (f, feed, _) = setup(1);
    let mut record = saved(&f, &feed);
    let template = Store::open(&f.db())
        .unwrap()
        .confirmed_bases("client", &feed.inventory.account_id, "root", &f.root)
        .unwrap()
        .into_values()
        .next()
        .unwrap();
    record.bulk_population.clear();
    record.entries.clear();
    for i in 0..5000 {
        let id = format!("id-{i}");
        let path = PathBuf::from(format!("docs/file-{i}.txt"));
        record.bulk_population.insert(
            id.clone(),
            ConfirmedBase {
                location: Some(ConfirmedLocation {
                    relative_path: path.clone(),
                    remote_name: format!("file-{i}.txt"),
                    remote_parent: "docs".into(),
                }),
                ..template.clone()
            },
        );
        let entry = ReconciliationEntry {
            remote_id: Some(id),
            relative_path: Some(path),
            kind: if i < 4800 {
                ChangeKind::LocalMissing
            } else {
                ChangeKind::Unchanged
            },
        };
        record.entries.push(entry.clone());
        record.entries.push(entry);
    }
    let review = crate::planner::bulk_impact("client", &record, BulkPolicy::default())
        .unwrap()
        .unwrap();
    assert!(review
        .impacts()
        .iter()
        .all(|i| i.affected == 4800 && i.population == 5000));
    for entry in &mut record.entries {
        entry.kind = ChangeKind::Unchanged;
    }
    for i in 0..12 {
        let id = format!("small-{i}");
        let path = PathBuf::from(format!("small/file-{i}.txt"));
        record.bulk_population.insert(
            id.clone(),
            ConfirmedBase {
                location: Some(ConfirmedLocation {
                    relative_path: path.clone(),
                    remote_name: format!("file-{i}.txt"),
                    remote_parent: "small".into(),
                }),
                ..template.clone()
            },
        );
        record.entries.push(ReconciliationEntry {
            remote_id: Some(id),
            relative_path: Some(path),
            kind: ChangeKind::RemoteDeleted,
        });
    }
    let review = crate::planner::bulk_impact("client", &record, BulkPolicy::default())
        .unwrap()
        .unwrap();
    assert_eq!(review.impacts().len(), 1);
    assert_eq!(review.impacts()[0].folder, PathBuf::from("small"));
    assert_eq!(review.impacts()[0].affected, 12);
    // Thresholds can evolve without rewriting aggregation, but cannot disable it.
    assert_eq!(
        crate::planner::bulk_impact(
            "client",
            &record,
            BulkPolicy {
                absolute: 0,
                ..BulkPolicy::default()
            }
        ),
        Err(EngineError::InvalidPlan)
    );
}

#[test]
fn n08_small_changes_continue_but_bulk_edits_require_review_and_preserve_prior_bytes() {
    let (f, mut feed, mut remote) = setup(50);
    for i in 0..12 {
        let mut item = remote.entries[&format!("file-{i}")].clone();
        item.version = Some(2);
        item.md5_checksum = Some("d16fb36f0911f878998c136191af705e".into());
        remote.content.insert(item.id.clone(), b"xyz".to_vec());
        remote.entries.insert(item.id.clone(), item.clone());
        feed.events.push(Change {
            id: item.id.clone(),
            removed: false,
            confirmed_trashed: false,
            item: Some(item),
        });
    }
    let calls = remote.media_calls;
    let pending = review(observed_cycle(&f, &mut feed, &mut remote));
    assert_eq!(remote.media_calls, calls);
    for i in 0..50 {
        assert_eq!(
            fs::read(f.root.join(format!("file-{i}.txt"))).unwrap(),
            b"abc"
        );
    }
    let updated = observed_cycle_reviewed(&f, &mut feed, &mut remote, Some(&pending)).unwrap();
    assert_eq!(updated.updated, 12);
    assert_eq!(updated.preserved_copies.len(), 12);
    for copy in updated.preserved_copies {
        assert_eq!(fs::read(f.root.join(copy)).unwrap(), b"abc");
    }
    fs::remove_file(f.root.join("file-49.txt")).unwrap();
    assert_eq!(
        observed_cycle(&f, &mut feed, &mut remote)
            .unwrap()
            .downloaded,
        1
    );
}

#[test]
fn p06_remote_trash_confirmation_never_authorizes_local_deletion() {
    let (f, mut feed, mut remote) = setup(50);
    let bases = Store::open(&f.db())
        .unwrap()
        .confirmed_bases("client", &feed.inventory.account_id, "root", &f.root)
        .unwrap();
    for i in 0..12 {
        let id = format!("file-{i}");
        remote.entries.remove(&id);
        feed.events.push(Change {
            id,
            removed: true,
            confirmed_trashed: true,
            item: None,
        });
    }
    let calls = remote.media_calls;
    let pending = review(observed_cycle(&f, &mut feed, &mut remote));
    assert_eq!(pending.impacts()[0].affected, 12);
    assert_eq!(remote.media_calls, calls);
    observed_cycle_reviewed(&f, &mut feed, &mut remote, Some(&pending)).unwrap();
    for i in 0..50 {
        assert_eq!(
            fs::read(f.root.join(format!("file-{i}.txt"))).unwrap(),
            b"abc"
        );
    }
    assert_eq!(remote.media_calls, calls);
    assert_eq!(
        Store::open(&f.db())
            .unwrap()
            .confirmed_bases("client", &feed.inventory.account_id, "root", &f.root)
            .unwrap(),
        bases
    );
    assert_eq!(review(observed_cycle(&f, &mut feed, &mut remote)), pending);
}

#[test]
fn n08_failed_transfer_or_corrupt_state_cannot_retain_automatic_consent() {
    let (f, mut feed, mut remote) = setup(50);
    for i in 0..12 {
        fs::remove_file(f.root.join(format!("file-{i}.txt"))).unwrap();
    }
    let pending = review(observed_cycle(&f, &mut feed, &mut remote));
    remote.fail_stream = true;
    let failed = observed_cycle_reviewed(&f, &mut feed, &mut remote, Some(&pending)).unwrap();
    assert_eq!(failed.downloaded + failed.updated, 0);
    assert!(!failed.issues.is_empty());
    let calls = remote.media_calls;
    let fresh = review(observed_cycle(&f, &mut feed, &mut remote));
    assert_eq!(remote.media_calls, calls);
    assert_eq!(fs::read(f.root.join("file-49.txt")).unwrap(), b"abc");
    let db = f.db().with_file_name("reconciliation.sqlite3");
    fs::write(&db, b"invalid preserved evidence").unwrap();
    assert_eq!(
        observed_cycle_reviewed(&f, &mut feed, &mut remote, Some(&fresh)).unwrap_err(),
        EngineError::StateStorage
    );
    assert_eq!(fs::read(db).unwrap(), b"invalid preserved evidence");
    assert_eq!(remote.media_calls, calls);
}

#[test]
fn n08_proposal_is_bound_to_remote_revision_scope_and_current_local_identity() {
    let (f, mut feed, mut remote) = setup(50);
    for i in 0..12 {
        fs::remove_file(f.root.join(format!("file-{i}.txt"))).unwrap();
    }
    let pending = review(observed_cycle(&f, &mut feed, &mut remote));
    let record = saved(&f, &feed);
    for mutation in ["revision", "account", "remote", "path", "base", "identity"] {
        let mut changed = record.clone();
        match mutation {
            "revision" => changed.remote.entries[0].version = Some(2),
            "account" => changed.plan.account_id.0 = "another-account".into(),
            "remote" => changed.plan.remote_root = "another-root".into(),
            "path" => changed.plan.local_root = "/another-root".into(),
            "base" => changed.bases.values_mut().next().unwrap().remote_version += 1,
            "identity" => {
                changed
                    .local
                    .entries
                    .insert("file-49.txt".into(), crate::local::Evidence::Unknown);
            }
            _ => unreachable!(),
        }
        assert!(
            matches!(
                crate::coordinator::bulk_gate("client", &changed, Some(&pending)),
                Err(EngineError::BulkReviewRequired(_))
            ),
            "{mutation}"
        );
    }
    // Input order is not a new observation.
    let mut reordered = record.clone();
    reordered.remote.entries.reverse();
    reordered.plan.entries.reverse();
    reordered.entries.reverse();
    assert_eq!(
        crate::coordinator::bulk_gate("client", &reordered, Some(&pending)),
        Ok(())
    );
}

#[test]
fn n08_stale_approval_aborts_even_when_restored_files_reduce_impact_below_threshold() {
    let (f, mut feed, mut remote) = setup(50);
    for i in 0..12 {
        fs::rename(
            f.root.join(format!("file-{i}.txt")),
            f.base.join(format!("retained-{i}")),
        )
        .unwrap();
    }
    let pending = review(observed_cycle(&f, &mut feed, &mut remote));
    for i in 0..11 {
        fs::rename(
            f.base.join(format!("retained-{i}")),
            f.root.join(format!("file-{i}.txt")),
        )
        .unwrap();
    }
    let calls = remote.media_calls;
    assert_eq!(
        observed_cycle_reviewed(&f, &mut feed, &mut remote, Some(&pending)).unwrap_err(),
        EngineError::BulkReviewChanged
    );
    assert_eq!(remote.media_calls, calls);
    assert!(!f.root.join("file-11.txt").exists());
    assert_eq!(
        observed_cycle(&f, &mut feed, &mut remote)
            .unwrap()
            .downloaded,
        1
    );
    assert_eq!(fs::read(f.base.join("retained-11")).unwrap(), b"abc");
}
