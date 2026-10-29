//! I01-I09 acceptance with isolated real files/SQLite and ID-aware remote media.
use super::baseline_tests::{base, cycle, cycle_client, prepare, prepare_client};
use super::*;
use crate::{ConflictReason, LocalObservation};
use std::fs::{File, FileTimes};
use std::time::{Duration, SystemTime};

const V1: &[u8] = b"BASE-V1";
const V2: &[u8] = b"BASE-V2";

fn inventory_with(bytes: &[u8]) -> RemoteInventory {
    let mut inv = inventory();
    inv.entries[0].name = "dados.db".into();
    inv.entries[0].size = Some(bytes.len() as u64);
    inv.entries[0].md5_checksum = Some(format!("{:x}", md5::Md5::digest(bytes)));
    inv
}
fn provider(inv: &RemoteInventory, bytes: &[u8]) -> Fake {
    let mut source = Fake::new(inv);
    source
        .content
        .insert(inv.entries[0].id.clone(), bytes.to_vec());
    source
}
fn installed() -> (Fixture, RemoteInventory) {
    let f = Fixture::new();
    let inv = inventory_with(V1);
    let report = cycle(&f, &inv, &mut provider(&inv, V1)).unwrap();
    assert_eq!(report.downloaded, 1);
    assert!(report.issues.is_empty());
    (f, inv)
}
fn file_identity(path: &std::path::Path) -> (u64, u64) {
    let m = fs::metadata(path).unwrap();
    (m.dev(), m.ino())
}
/// Checks the serialized diagnostic independently from the in-memory plan.
fn assert_action(f: &Fixture, inv: &RemoteInventory, expected: PlanAction) {
    let plan = prepare(f, inv).unwrap();
    assert_eq!(plan.entries[0].action, expected);
    let conn = rusqlite::Connection::open(f.db()).unwrap();
    let json: String = conn
        .query_row(
            "SELECT plan FROM observations WHERE file=?1",
            [&inv.entries[0].id],
            |r| r.get(0),
        )
        .unwrap();
    let entry: crate::PlanEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(entry.remote_id, inv.entries[0].id);
    assert_eq!(entry.action, expected);
}

#[test]
fn i01_continuity_reopens_without_media_or_identity_changes() {
    let (f, inv) = installed();
    let confirmed = base(&f, &inv).unwrap();
    let id = file_identity(&f.root.join("dados.db"));
    for _ in 0..2 {
        assert_action(&f, &inv, PlanAction::NoOp);
        let mut source = provider(&inv, V1);
        let report = cycle(&f, &inv, &mut source).unwrap();
        assert_eq!(report.unchanged, 1);
        assert!(report.issues.is_empty());
        assert!(report.preserved_copies.is_empty());
        assert!(source.media_ids.is_empty());
        assert!(source.metadata_ids.iter().all(|id| id == "file"));
        assert_eq!(file_identity(&f.root.join("dados.db")), id);
        assert_eq!(fs::read(f.root.join("dados.db")).unwrap(), V1);
        assert_eq!(base(&f, &inv), Some(confirmed.clone()));
    }
}

#[test]
fn i02_content_edit_retains_identity_and_confirmed_base() {
    let (f, inv) = installed();
    let confirmed = base(&f, &inv).unwrap();
    let id = file_identity(&f.root.join("dados.db"));
    fs::write(f.root.join("dados.db"), b"LOCAL-V3").unwrap();
    assert_eq!(file_identity(&f.root.join("dados.db")), id);
    for _ in 0..2 {
        assert_action(&f, &inv, PlanAction::Blocked(ConflictReason::LocalChanged));
        let mut source = provider(&inv, V1);
        assert_eq!(
            cycle(&f, &inv, &mut source).unwrap().issues[0].error,
            EngineError::DownloadConflict
        );
        assert!(source.media_ids.is_empty());
        assert_eq!(base(&f, &inv), Some(confirmed.clone()));
        assert_eq!(fs::read(f.root.join("dados.db")).unwrap(), b"LOCAL-V3");
    }
}

#[test]
fn i03_i04_replacements_keep_identity_diagnostic_despite_equal_metadata_or_bytes() {
    for bytes in [V1, &b"DIFF-V1"[..]] {
        for remote_updated in [false, true] {
            let (f, inv) = installed();
            let confirmed = base(&f, &inv).unwrap();
            let path = f.root.join("dados.db");
            let original = f.root.join("original");
            let times =
                FileTimes::new().set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(123456));
            File::open(&path).unwrap().set_times(times).unwrap();
            fs::rename(&path, &original).unwrap();
            fs::write(&path, bytes).unwrap();
            File::open(&path).unwrap().set_times(times).unwrap();
            assert_ne!(file_identity(&path), file_identity(&original));
            let a = fs::metadata(&path).unwrap();
            let b = fs::metadata(&original).unwrap();
            assert_eq!(
                (a.len(), a.modified().unwrap()),
                (b.len(), b.modified().unwrap())
            );
            let mut next = inventory_with(if remote_updated { V2 } else { V1 });
            next.entries[0].version = Some(if remote_updated { 2 } else { 1 });
            for _ in 0..2 {
                let reason = if remote_updated {
                    ConflictReason::ConcurrentIdentityChanges
                } else {
                    ConflictReason::LocalIdentityChanged
                };
                assert_action(&f, &next, PlanAction::Blocked(reason));
                let mut source = provider(&next, if remote_updated { V2 } else { V1 });
                let remote_before = source.content.clone();
                assert_eq!(
                    cycle(&f, &next, &mut source).unwrap().issues[0].error,
                    EngineError::DownloadConflict
                );
                assert!(source.media_ids.is_empty());
                assert_eq!(source.content, remote_before);
                assert_eq!(fs::read(&path).unwrap(), bytes);
                assert_eq!(fs::read(&original).unwrap(), V1);
                assert_eq!(base(&f, &next), Some(confirmed.clone()));
            }
        }
    }
}

#[test]
fn i05_new_remote_id_never_inherits_the_old_installation() {
    for bytes in [V1, V2] {
        let (f, old) = installed();
        let confirmed = base(&f, &old).unwrap();
        let mut next = inventory_with(bytes);
        next.entries[0].id = "replacement".into();
        for _ in 0..2 {
            assert_action(
                &f,
                &next,
                PlanAction::Blocked(ConflictReason::ExistingLocalData),
            );
            let mut source = provider(&next, bytes);
            let remote_before = source.content.clone();
            assert_eq!(
                cycle(&f, &next, &mut source).unwrap().issues[0].error,
                EngineError::DownloadConflict
            );
            assert!(source.media_ids.is_empty());
            assert!(source.metadata_ids.iter().all(|id| id == "replacement"));
            assert_eq!(source.content, remote_before);
            assert!(base(&f, &next).is_none());
            assert_eq!(base(&f, &old), Some(confirmed.clone()));
            assert_eq!(fs::read(f.root.join("dados.db")).unwrap(), V1);
        }
    }
}

#[test]
fn i06_same_remote_id_updates_only_after_durable_installation() {
    let (f, old) = installed();
    let confirmed = base(&f, &old).unwrap();
    let mut next = inventory_with(V2);
    next.entries[0].version = Some(2);
    assert_action(&f, &next, PlanAction::Download);
    assert_eq!(base(&f, &old), Some(confirmed));
    let mut source = provider(&next, V2);
    let report = cycle(&f, &next, &mut source).unwrap();
    assert_eq!(report.updated, 1);
    assert!(report.issues.is_empty());
    assert_eq!(report.preserved_copies.len(), 1);
    assert_eq!(
        fs::read(f.root.join(&report.preserved_copies[0])).unwrap(),
        V1
    );
    assert_eq!(fs::read(f.root.join("dados.db")).unwrap(), V2);
    let new_base = base(&f, &next).unwrap();
    assert_eq!(new_base.remote_version, 2);
    assert_eq!(
        new_base.local_fingerprint,
        filesystem::fingerprint(
            &f.root,
            std::path::Path::new("dados.db"),
            &Cancellation::new()
        )
        .unwrap()
        .unwrap()
    );
    assert_eq!(source.media_ids, ["file"]);
    for _ in 0..2 {
        assert_action(&f, &next, PlanAction::NoOp);
        assert_eq!(cycle(&f, &next, &mut source).unwrap().unchanged, 1);
        assert_eq!(source.media_ids, ["file"]);
        assert_eq!(base(&f, &next), Some(new_base.clone()));
    }
}

#[test]
fn i08_each_scope_component_requires_its_own_base() {
    for variant in 0..4 {
        let (mut f, old) = installed();
        let original_root = f.root.clone();
        let confirmed = base(&f, &old).unwrap();
        let mut inv = old.clone();
        let client = if variant == 0 {
            "other-client"
        } else {
            "client"
        };
        match variant {
            1 => inv.account_id = AccountId("other-account".into()),
            2 => {
                inv.root_id = "other-root".into();
                inv.entries[0].parent_id = inv.root_id.clone();
            }
            3 => {
                f.root = f.base.join("other-local");
                fs::create_dir(&f.root).unwrap();
                fs::write(f.root.join("dados.db"), V1).unwrap();
            }
            _ => {}
        }
        for _ in 0..2 {
            assert!(Store::open(&f.db())
                .unwrap()
                .confirmed_bases(client, &inv.account_id, &inv.root_id, &f.root)
                .unwrap()
                .is_empty());
            let mut source = provider(&inv, V1);
            let plan = prepare_client(&f, &inv, client).unwrap();
            assert_eq!(
                plan.entries[0].action,
                PlanAction::Blocked(ConflictReason::ExistingLocalData)
            );
            let persisted = Store::open(&f.db())
                .unwrap()
                .load(client, &inv.account_id, &inv.root_id, &f.root)
                .unwrap()
                .unwrap();
            assert_eq!(persisted, plan);
            let remote_before = source.content.clone();
            let report = cycle_client(&f, &inv, &mut source, client).unwrap();
            assert_eq!(source.content, remote_before);
            assert_eq!(report.issues[0].error, EngineError::DownloadConflict);
            assert!(source.media_ids.is_empty());
            assert_eq!(fs::read(f.root.join("dados.db")).unwrap(), V1);
            let bases = Store::open(&f.db())
                .unwrap()
                .confirmed_bases("client", &old.account_id, &old.root_id, &original_root)
                .unwrap();
            assert_eq!(bases["file"], confirmed);
        }
    }
}

#[test]
fn i09_nonregular_or_unreadable_identity_never_becomes_absence() {
    for directory in [false, true] {
        let (f, inv) = installed();
        let confirmed = base(&f, &inv).unwrap();
        fs::rename(f.root.join("dados.db"), f.root.join("original")).unwrap();
        let outside = f.base.join("outside");
        fs::write(&outside, b"OUTSIDE").unwrap();
        if directory {
            fs::create_dir(f.root.join("dados.db")).unwrap();
        } else {
            std::os::unix::fs::symlink(&outside, f.root.join("dados.db")).unwrap();
        }
        for _ in 0..2 {
            assert_action(
                &f,
                &inv,
                PlanAction::Blocked(ConflictReason::LocalTypeMismatch),
            );
            let mut source = provider(&inv, V1);
            assert_eq!(
                cycle(&f, &inv, &mut source).unwrap().issues[0].error,
                EngineError::DownloadConflict
            );
            assert!(source.media_ids.is_empty());
            assert_eq!(fs::read(&outside).unwrap(), b"OUTSIDE");
            assert_eq!(fs::read(f.root.join("original")).unwrap(), V1);
            assert_eq!(base(&f, &inv), Some(confirmed.clone()));
        }
    }
    let (f, inv) = installed();
    use std::os::unix::fs::PermissionsExt;
    let path = f.root.join("dados.db");
    let permissions = fs::metadata(&path).unwrap().permissions();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
    let mut source = provider(&inv, V1);
    let result = cycle(&f, &inv, &mut source);
    fs::set_permissions(&path, permissions).unwrap();
    assert!(matches!(result, Err(EngineError::DownloadIo)));
    assert!(source.media_ids.is_empty());
    let bases = Store::open(&f.db())
        .unwrap()
        .confirmed_bases("client", &inv.account_id, &inv.root_id, &f.root)
        .unwrap();
    let error = crate::planner::plan_with_bases(&inv, &f.root, &bases, |_, _, _| {
        Err(EngineError::DownloadIo)
    });
    assert!(matches!(error, Err(EngineError::DownloadIo)));
    let unknown = crate::planner::plan_with_bases(&inv, &f.root, &bases, |_, _, _| {
        Ok(LocalObservation::Unknown)
    })
    .unwrap();
    assert_eq!(unknown.entries[0].action, PlanAction::DeferredUnknown);
    assert_eq!(fs::read(f.root.join("dados.db")).unwrap(), V1);
}

#[test]
fn i09_incomplete_persisted_identity_does_not_authorize_media_or_confirmation() {
    let (f, inv) = installed();
    let conn = rusqlite::Connection::open(f.db()).unwrap();
    let json: String = conn
        .query_row("SELECT record FROM downloads", [], |r| r.get(0))
        .unwrap();
    let mut op: Operation = serde_json::from_str(&json).unwrap();
    let Phase::Installed {
        ref mut identity, ..
    } = op.phase
    else {
        panic!("expected installed evidence")
    };
    identity.inode = 0;
    let invalid = serde_json::to_string(&op).unwrap();
    conn.execute("UPDATE downloads SET record=?1", [&invalid])
        .unwrap();
    drop(conn);
    for _ in 0..2 {
        assert_action(&f, &inv, PlanAction::DeferredUnknown);
        let mut source = provider(&inv, V1);
        assert_eq!(
            cycle(&f, &inv, &mut source).unwrap().issues[0].error,
            EngineError::DownloadConflict
        );
        assert!(source.media_ids.is_empty());
        assert_eq!(fs::read(f.root.join("dados.db")).unwrap(), V1);
        let conn = rusqlite::Connection::open(f.db()).unwrap();
        let retained: String = conn
            .query_row("SELECT record FROM downloads", [], |r| r.get(0))
            .unwrap();
        assert_eq!(retained, invalid);
    }
}
