//! I15-I21: historical bytes never transfer identity or bypass durable evidence.
use super::baseline_tests::{base, cycle, prepare};
use super::*;
use crate::{ConflictReason, DownloadPlan};

const V1: &[u8] = b"BASE-V1";
const V2: &[u8] = b"BASE-V2";
const V3: &[u8] = b"LOCAL-V3";

fn remote(bytes: &[u8], version: u64) -> RemoteInventory {
    let mut inv = inventory();
    let item = &mut inv.entries[0];
    item.name = "dados.db".into();
    item.version = Some(version);
    item.size = Some(bytes.len() as u64);
    item.md5_checksum = Some(format!("{:x}", md5::Md5::digest(bytes)));
    inv
}
fn provider(inv: &RemoteInventory, bytes: &[u8]) -> Fake {
    let mut source = Fake::new(inv);
    source
        .content
        .insert(inv.entries[0].id.clone(), bytes.to_vec());
    source
}
/// Creates both bases through actual installations, then closes every SQLite
/// connection before copying the DELETE-journal database as a historical fixture.
fn history() -> (Fixture, RemoteInventory, PathBuf) {
    let f = Fixture::new();
    let first = remote(V1, 1);
    assert_eq!(
        cycle(&f, &first, &mut provider(&first, V1))
            .unwrap()
            .downloaded,
        1
    );
    let saved = f.base.join("v1.sqlite3");
    fs::copy(f.db(), &saved).unwrap();
    let current = remote(V2, 2);
    let report = cycle(&f, &current, &mut provider(&current, V2)).unwrap();
    assert_eq!(report.updated, 1);
    assert_eq!(report.preserved_copies.len(), 1);
    assert_eq!(
        fs::read(f.root.join(&report.preserved_copies[0])).unwrap(),
        V1
    );
    assert_eq!(base(&f, &current).unwrap().remote_version, 2);
    (f, current, saved)
}
fn decision(f: &Fixture, inv: &RemoteInventory, expected: PlanAction) -> DownloadPlan {
    let plan = prepare(f, inv).unwrap();
    assert_eq!(plan.entries[0].action, expected);
    let reopened = Store::open(&f.db())
        .unwrap()
        .load("client", &inv.account_id, &inv.root_id, &f.root)
        .unwrap()
        .unwrap();
    assert_eq!(reopened, plan);
    plan
}
/// Captures all root file bytes and identities, including retained versions, to
/// reject preservation that accidentally changes an unasserted backup or inode.
fn files(f: &Fixture) -> HashMap<PathBuf, (Vec<u8>, u64, u64)> {
    fs::read_dir(&f.root)
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            let m = e.metadata().unwrap();
            assert!(m.is_file());
            (
                PathBuf::from(e.file_name()),
                (fs::read(e.path()).unwrap(), m.dev(), m.ino()),
            )
        })
        .collect()
}
fn blocked(f: &Fixture, inv: &RemoteInventory, expected: PlanAction) {
    let old_base = base(f, inv);
    let old_files = files(f);
    for _ in 0..2 {
        decision(f, inv, expected.clone());
        let remote_bytes = [V1, V2, V3]
            .into_iter()
            .find(|bytes| {
                inv.entries[0].md5_checksum.as_deref()
                    == Some(format!("{:x}", md5::Md5::digest(bytes)).as_str())
            })
            .unwrap();
        let mut source = provider(inv, remote_bytes);
        let remote_before = source.content.clone();
        let result = cycle(f, inv, &mut source);
        if inv.entries[0].version.is_none_or(|v| v == 0) {
            assert!(matches!(result, Err(EngineError::InvalidPlan)));
        } else {
            let report = result.unwrap();
            assert_eq!(report.issues.len(), 1);
            assert_eq!(
                report.issues[0].error,
                if expected == PlanAction::DeferredUnknown {
                    EngineError::RemoteChanged
                } else {
                    EngineError::DownloadConflict
                }
            );
            assert!(report.preserved_copies.is_empty());
            assert_eq!(
                (report.downloaded, report.updated, report.recovered),
                (0, 0, 0)
            );
        }
        assert!(source.media_ids.is_empty());
        assert!(source
            .metadata_ids
            .iter()
            .all(|id| id == &inv.entries[0].id));
        assert_eq!(source.content, remote_before);
        assert_eq!(base(f, inv), old_base);
        assert_eq!(files(f), old_files);
    }
}

#[test]
fn i15_historical_bytes_at_a_newer_revision_preserve_the_current_version() {
    let (f, current, _) = history();
    let old_base = base(&f, &current).unwrap();
    let old_files = files(&f);
    let historical = remote(V1, 3);
    decision(&f, &historical, PlanAction::Download);
    assert_eq!(base(&f, &historical), Some(old_base));
    let mut source = provider(&historical, V1);
    let report = cycle(&f, &historical, &mut source).unwrap();
    assert!(report.issues.is_empty());
    assert_eq!(report.updated, 1);
    assert_eq!(report.preserved_copies.len(), 1);
    assert_eq!(
        fs::read(f.root.join(&report.preserved_copies[0])).unwrap(),
        V2
    );
    assert_eq!(fs::read(f.root.join("dados.db")).unwrap(), V1);
    for (name, expected) in &old_files {
        if name != std::path::Path::new("dados.db") {
            assert_eq!(&files(&f)[name], expected);
        }
    }
    let confirmed = base(&f, &historical).unwrap();
    assert_eq!(confirmed.remote_version, 3);
    assert_eq!(
        Some(confirmed.local_fingerprint.clone()),
        filesystem::fingerprint(
            &f.root,
            std::path::Path::new("dados.db"),
            &Cancellation::new()
        )
        .unwrap()
    );
    let after = files(&f);
    for _ in 0..2 {
        decision(&f, &historical, PlanAction::NoOp);
        assert_eq!(cycle(&f, &historical, &mut source).unwrap().unchanged, 1);
        assert_eq!(base(&f, &historical), Some(confirmed.clone()));
        assert_eq!(files(&f), after);
        assert_eq!(source.media_ids, ["file"]);
    }
}

#[test]
fn i16_historical_bytes_in_a_new_remote_object_never_inherit_base() {
    let (f, current, _) = history();
    let old_base = base(&f, &current).unwrap();
    let mut historical = remote(V1, 3);
    historical.entries[0].id = "backup-object".into();
    blocked(
        &f,
        &historical,
        PlanAction::Blocked(ConflictReason::ExistingLocalData),
    );
    assert!(base(&f, &historical).is_none());
    assert_eq!(base(&f, &current), Some(old_base));
}

#[test]
fn i17_local_history_distinguishes_content_from_identity_without_rewinding_base() {
    for replace in [false, true] {
        let (f, current, _) = history();
        let path = f.root.join("dados.db");
        let before = fs::metadata(&path).unwrap();
        if replace {
            fs::rename(&path, f.root.join("retained-v2")).unwrap();
        }
        fs::write(&path, V1).unwrap();
        let after = fs::metadata(&path).unwrap();
        assert_eq!(
            (before.dev(), before.ino()) != (after.dev(), after.ino()),
            replace
        );
        blocked(
            &f,
            &current,
            PlanAction::Blocked(if replace {
                ConflictReason::LocalIdentityChanged
            } else {
                ConflictReason::LocalChanged
            }),
        );
        assert_eq!(base(&f, &current).unwrap().remote_version, 2);
        if replace {
            assert_eq!(fs::read(f.root.join("retained-v2")).unwrap(), V2);
        }
    }
}

#[test]
fn i18_regressive_zero_and_missing_revisions_are_not_historical_authority() {
    for version in [Some(1), Some(0), None] {
        let (f, current, _) = history();
        let confirmed = base(&f, &current).unwrap();
        let mut historical = remote(V1, 1);
        historical.entries[0].version = version;
        blocked(&f, &historical, PlanAction::DeferredUnknown);
        assert_eq!(base(&f, &current), Some(confirmed));
    }
}

#[test]
fn i19_historical_and_independent_edits_conflict_against_the_last_confirmed_base() {
    for remote_history in [false, true] {
        for replace in [false, true] {
            for offline in [false, true] {
                let (f, current, _) = history();
                let confirmed = base(&f, &current).unwrap();
                // An offline interval has no observations. The online variant
                // additionally observes the still-current version before edits.
                if !offline {
                    assert_eq!(
                        cycle(&f, &current, &mut provider(&current, V2))
                            .unwrap()
                            .unchanged,
                        1
                    );
                }
                let path = f.root.join("dados.db");
                if replace {
                    fs::rename(&path, f.root.join("retained-v2")).unwrap();
                }
                fs::write(&path, if remote_history { V3 } else { V1 }).unwrap();
                let next = remote(if remote_history { V1 } else { V3 }, 3);
                blocked(
                    &f,
                    &next,
                    PlanAction::Blocked(if replace {
                        ConflictReason::ConcurrentIdentityChanges
                    } else {
                        ConflictReason::ConcurrentChanges
                    }),
                );
                assert_eq!(base(&f, &current), Some(confirmed));
            }
        }
    }
}

#[test]
fn i20_known_historical_content_without_provenance_has_only_observable_diagnostics() {
    let (f, current, saved) = history();
    let conn = rusqlite::Connection::open(saved).unwrap();
    let record: String = conn
        .query_row("SELECT record FROM downloads", [], |r| r.get(0))
        .unwrap();
    let historical: Operation = serde_json::from_str(&record).unwrap();
    let Phase::Installed { sha256, .. } = historical.phase else {
        panic!("expected confirmed V1")
    };
    assert_eq!(sha256, format!("{:x}", sha2::Sha256::digest(V1)));
    // Knowing that these bytes were installed before is not provenance for the
    // present change. The input model makes no assertion of restoration intent.
    let next = remote(V1, 3);
    let plan = decision(&f, &next, PlanAction::Download);
    assert_eq!(plan.entries[0].remote_id, "file");
    assert_eq!(plan.entries[0].expected_remote_version, Some(3));
    fs::write(f.root.join("dados.db"), V1).unwrap();
    blocked(
        &f,
        &current,
        PlanAction::Blocked(ConflictReason::LocalChanged),
    );
}

#[test]
fn i21_restored_or_missing_state_never_adopts_divergent_current_files() {
    for variant in 0..4 {
        let (f, current, saved) = history();
        match variant {
            0 => {
                fs::copy(saved, f.db()).unwrap();
            }
            1 => {
                fs::write(f.root.join("dados.db"), V1).unwrap();
            }
            2 => {
                fs::rename(f.root.join("dados.db"), f.root.join("retained-v2")).unwrap();
                fs::write(f.root.join("dados.db"), V1).unwrap();
            }
            _ => {
                fs::remove_file(f.db()).unwrap();
            }
        }
        let expected = match variant {
            0 => ConflictReason::ConcurrentIdentityChanges,
            1 => ConflictReason::LocalChanged,
            2 => ConflictReason::LocalIdentityChanged,
            _ => ConflictReason::ExistingLocalData,
        };
        blocked(&f, &current, PlanAction::Blocked(expected));
        if variant == 3 {
            assert!(base(&f, &current).is_none());
        } else {
            assert_eq!(
                base(&f, &current).unwrap().remote_version,
                if variant == 0 { 1 } else { 2 }
            );
        }
    }
}

#[test]
fn i21_joint_local_rollback_without_an_independent_anchor_looks_like_remote_change() {
    let (f, current, saved) = history();
    let backup = fs::read_dir(&f.root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.file_name().unwrap().to_str().unwrap().ends_with(".bak"))
        .unwrap();
    // Restore the actual V1 inode as well as the old journal. Retain V2 outside
    // the root as test evidence, not an independent input available to the engine.
    let retained = f.base.join("retained-v2");
    fs::rename(f.root.join("dados.db"), &retained).unwrap();
    fs::rename(backup, f.root.join("dados.db")).unwrap();
    fs::copy(saved, f.db()).unwrap();
    let restored = base(&f, &current).unwrap();
    assert_eq!(restored.remote_version, 1);
    assert_eq!(
        Some(restored.local_fingerprint),
        filesystem::fingerprint(
            &f.root,
            std::path::Path::new("dados.db"),
            &Cancellation::new()
        )
        .unwrap()
    );
    // There is no local rollback signal in mutually consistent B/L. Current R2
    // still permits an ordinary safe update; no claim of rollback detection.
    decision(&f, &current, PlanAction::Download);
    let mut source = provider(&current, V2);
    let report = cycle(&f, &current, &mut source).unwrap();
    assert!(report.issues.is_empty());
    assert_eq!(report.updated, 1);
    assert_eq!(
        fs::read(f.root.join(&report.preserved_copies[0])).unwrap(),
        V1
    );
    assert_eq!(fs::read(&retained).unwrap(), V2);
    assert_eq!(base(&f, &current).unwrap().remote_version, 2);
    let after = files(&f);
    assert_eq!(cycle(&f, &current, &mut source).unwrap().unchanged, 1);
    assert_eq!(source.media_ids, ["file"]);
    assert_eq!(files(&f), after);
}
