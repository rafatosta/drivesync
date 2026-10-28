//! I25-I28: real journal failure boundaries, verified replay and stale events.
use super::baseline_tests::{base, cycle, prepare};
use super::concurrency_tests::{execute_plan, fake, files, installed, journal, remote, V1, V2};
use super::*;

/// Rejects only final confirmation, after the actual filesystem effect. Every
/// connection closes before replay; no successful journal is rewritten as pending.
fn unconfirmed_effect(f: &Fixture, inv: &RemoteInventory, source: &mut Fake) -> Operation {
    let plan = prepare(f, inv).unwrap();
    let conn = rusqlite::Connection::open(f.db()).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_confirm BEFORE UPDATE ON downloads WHEN json_extract(NEW.record,'$.phase.Installed') IS NOT NULL OR json_extract(NEW.record,'$.phase.Updated') IS NOT NULL BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert_eq!(
        execute_plan(f, inv, &plan, source, &Cancellation::new(), |_| {}).unwrap_err(),
        EngineError::StateStorage
    );
    conn.execute_batch("DROP TRIGGER fail_confirm").unwrap();
    drop(conn);
    assert!(base(f, inv).is_none());
    let op = journal(f, inv).pop().unwrap();
    assert!(matches!(
        op.phase,
        Phase::Ready { .. } | Phase::UpdateReady { .. }
    ));
    op
}

#[test]
fn i26_actual_effect_without_confirmation_replays_only_the_verified_identity() {
    for update in [false, true] {
        for substitution in 0..3 {
            let (f, _) = if update {
                installed()
            } else {
                (Fixture::new(), remote(V1, 1))
            };
            let next = remote(V2, 2);
            let mut source = fake(&next, V2);
            let op = unconfirmed_effect(&f, &next, &mut source);
            let expected = match &op.phase {
                Phase::Ready { identity, .. } | Phase::UpdateReady { identity, .. } => *identity,
                _ => unreachable!(),
            };
            let path = f.root.join("dados.db");
            let effect = fs::metadata(&path).unwrap();
            assert_eq!(
                (effect.dev(), effect.ino()),
                (expected.device, expected.inode)
            );
            assert_eq!(fs::read(&path).unwrap(), V2);
            if substitution != 0 {
                fs::rename(&path, f.root.join("retained-effect")).unwrap();
                fs::write(
                    &path,
                    if substitution == 1 {
                        V2
                    } else {
                        &b"USER-DATA"[..]
                    },
                )
                .unwrap();
                let substitute = fs::metadata(&path).unwrap();
                assert_ne!(
                    (substitute.dev(), substitute.ino()),
                    (expected.device, expected.inode)
                );
            }
            let retained = files(&f);
            let pending = serde_json::to_value(journal(&f, &next)).unwrap();
            for iteration in 0..2 {
                let report = cycle(&f, &next, &mut source).unwrap();
                if substitution == 0 {
                    assert!(report.issues.is_empty());
                    assert_eq!(report.recovered, usize::from(iteration == 0));
                    assert_eq!(report.unchanged, usize::from(iteration != 0));
                    let confirmed = base(&f, &next).unwrap();
                    assert_eq!(confirmed.remote_version, 2);
                    assert_eq!(
                        Some(confirmed.local_fingerprint),
                        filesystem::fingerprint(
                            &f.root,
                            std::path::Path::new("dados.db"),
                            &Cancellation::new()
                        )
                        .unwrap()
                    );
                } else {
                    assert_eq!(report.issues[0].error, EngineError::DownloadConflict);
                    assert_eq!(
                        (report.recovered, report.updated, report.downloaded),
                        (0, 0, 0)
                    );
                    assert!(base(&f, &next).is_none());
                    assert_eq!(serde_json::to_value(journal(&f, &next)).unwrap(), pending);
                    assert_eq!(fs::read(f.root.join("retained-effect")).unwrap(), V2);
                }
                assert_eq!(files(&f), retained);
                assert_eq!(source.media_ids, ["file"]);
                if update {
                    let Phase::UpdateReady { backup, .. } = &op.phase else {
                        unreachable!()
                    };
                    assert_eq!(fs::read(f.root.join(backup)).unwrap(), V1);
                }
            }
        }
    }
}
