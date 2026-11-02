//! Deterministic quiescence, offline discovery and descriptor safety oracles.
use super::{inventory::scan_with_wait, *};
use crate::{download::filesystem::Root, Cancellation, LocalObservation};
use std::{fs, path::Path};
struct Tree(std::path::PathBuf);
impl Tree {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "drivesync-local-{}",
            oauth2::CsrfToken::new_random().secret()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn scan(&self, wait: impl FnOnce() -> Result<(), crate::EngineError>) -> Inventory {
        scan_with_wait(&Root::open(&self.0).unwrap(), &Cancellation::new(), wait).unwrap()
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn monitor_discovery_skips_symlinks_and_does_not_hold_the_sync_lock() {
    let t = Tree::new();
    fs::create_dir(t.0.join("nested")).unwrap();
    fs::create_dir(t.0.join(".drivesync")).unwrap();
    fs::create_dir(t.0.join("writing.part")).unwrap();
    std::os::unix::fs::symlink("/", t.0.join("outside")).unwrap();
    let cancel = Cancellation::new();
    let tree = MonitorTree::discover(&t.0, &cancel).unwrap();
    let paths: Vec<_> = tree
        .directories()
        .iter()
        .map(|dir| dir.relative())
        .collect();
    assert_eq!(paths, [Path::new(""), Path::new("nested")]);
    assert!(Root::open(&t.0).is_ok());
    assert_eq!(tree.validate(&cancel), Ok(()));
}

#[test]
fn monitor_discovery_detects_replaced_directories_and_cancellation() {
    let t = Tree::new();
    fs::create_dir(t.0.join("nested")).unwrap();
    let cancel = Cancellation::new();
    let tree = MonitorTree::discover(&t.0, &cancel).unwrap();
    fs::rename(t.0.join("nested"), t.0.join("old")).unwrap();
    fs::create_dir(t.0.join("nested")).unwrap();
    assert_eq!(tree.validate(&cancel), Err(MonitorError::Changed));
    cancel.cancel();
    assert!(matches!(
        MonitorTree::discover(&t.0, &cancel),
        Err(MonitorError::Cancelled)
    ));
}

#[test]
fn full_scan_recovers_offline_changes_without_events_and_rejects_temporary_patterns() {
    let t = Tree::new();
    fs::create_dir(t.0.join("nested")).unwrap();
    fs::write(t.0.join("nested/file"), b"before").unwrap();
    for name in [
        ".settings",
        "legitimate.tmp",
        ".editor.swp",
        "a.part",
        "a.crdownload",
        "x~",
        "~$book.xlsx",
        ".~lock.book#",
        ".goutputstream-123",
    ] {
        fs::write(t.0.join(name), b"bytes").unwrap();
    }
    fs::create_dir(t.0.join(".drivesync")).unwrap();
    fs::write(t.0.join(".drivesync/token"), b"internal").unwrap();
    let first = t.scan(|| Ok(()));
    assert!(!first.entries.contains_key(Path::new(".drivesync")));
    for name in [".settings", "legitimate.tmp"] {
        assert!(matches!(
            first.entries[Path::new(name)],
            Evidence::Stable { .. }
        ));
    }
    for name in [
        ".editor.swp",
        "a.part",
        "a.crdownload",
        "x~",
        "~$book.xlsx",
        ".~lock.book#",
        ".goutputstream-123",
    ] {
        assert_eq!(first.entries[Path::new(name)], Evidence::Temporary);
    }
    fs::write(t.0.join("nested/file"), b"offline edit").unwrap();
    fs::write(t.0.join("new"), b"new").unwrap();
    let second = t.scan(|| Ok(()));
    assert_ne!(
        first.entries[Path::new("nested/file")],
        second.entries[Path::new("nested/file")]
    );
    assert!(matches!(
        second.observe(Path::new("new")),
        LocalObservation::Verified(_)
    ));
    fs::remove_file(t.0.join("nested/file")).unwrap();
    assert_eq!(
        t.scan(|| Ok(())).observe(Path::new("nested/file")),
        LocalObservation::Absent
    );
}
#[test]
fn writes_and_equal_byte_replacements_during_quiescence_are_deferred() {
    for replace in [false, true] {
        let t = Tree::new();
        fs::write(t.0.join("file"), b"old").unwrap();
        let inv = t.scan(|| {
            if replace {
                fs::write(t.0.join("replacement"), b"old").unwrap();
                fs::rename(t.0.join("replacement"), t.0.join("file")).unwrap();
            } else {
                fs::write(t.0.join("file"), b"new content").unwrap();
            }
            Ok(())
        });
        assert_eq!(inv.entries[Path::new("file")], Evidence::Writing);
        assert_eq!(inv.observe(Path::new("file")), LocalObservation::Unknown);
        assert!(matches!(
            t.scan(|| Ok(())).entries[Path::new("file")],
            Evidence::Stable { .. }
        ));
    }
}
#[test]
fn symlink_descendants_and_cancelled_scans_are_not_absence() {
    let t = Tree::new();
    std::os::unix::fs::symlink("/tmp", t.0.join("link")).unwrap();
    let inv = t.scan(|| Ok(()));
    assert_eq!(inv.entries[Path::new("link")], Evidence::Unsupported);
    assert_eq!(
        inv.observe(Path::new("link/missing")),
        LocalObservation::Unknown
    );
    fs::write(t.0.join("file"), b"bytes").unwrap();
    let cancel = Cancellation::new();
    assert_eq!(
        scan_with_wait(&Root::open(&t.0).unwrap(), &cancel, || {
            cancel.cancel();
            Ok(())
        }),
        Err(crate::EngineError::Authentication(
            crate::AuthError::Cancelled
        ))
    );
}

#[test]
fn sync018_e03_unreadable_files_remain_unknown_to_planner_with_access_diagnosis() {
    use std::os::unix::fs::PermissionsExt;
    let t = Tree::new();
    fs::write(t.0.join("file"), b"preserved").unwrap();
    fs::set_permissions(t.0.join("file"), fs::Permissions::from_mode(0o000)).unwrap();
    let inv = t.scan(|| Ok(()));
    assert_eq!(inv.entries[Path::new("file")], Evidence::AccessDenied);
    assert_eq!(inv.observe(Path::new("file")), LocalObservation::Unknown);
    fs::set_permissions(t.0.join("file"), fs::Permissions::from_mode(0o600)).unwrap();
    fs::create_dir(t.0.join("locked")).unwrap();
    fs::write(t.0.join("locked/child"), b"preserved child").unwrap();
    fs::set_permissions(t.0.join("locked"), fs::Permissions::from_mode(0o000)).unwrap();
    let root = Root::open(&t.0).unwrap();
    assert!(scan_with_wait(&root, &Cancellation::new(), || Ok(())).is_err());
    fs::set_permissions(t.0.join("locked"), fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        fs::read(t.0.join("locked/child")).unwrap(),
        b"preserved child"
    );
}

#[test]
fn sync020_n05_aliases_and_special_files_are_never_upload_candidates() {
    let outside = Tree::new();
    let t = Tree::new();
    fs::write(outside.0.join("private"), b"private bytes").unwrap();
    fs::hard_link(outside.0.join("private"), t.0.join("alias")).unwrap();
    std::os::unix::fs::symlink(outside.0.join("private"), t.0.join("link")).unwrap();
    rustix::fs::mknodat(
        rustix::fs::CWD,
        t.0.join("pipe"),
        rustix::fs::FileType::Fifo,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        0,
    )
    .unwrap();
    for _ in 0..2 {
        let inv = t.scan(|| Ok(()));
        for name in ["alias", "link", "pipe"] {
            assert_eq!(inv.entries[Path::new(name)], Evidence::Unsupported);
            assert_eq!(
                inv.observe(Path::new(name)),
                LocalObservation::SymlinkOrSpecial
            );
        }
    }
    assert_eq!(
        fs::read(outside.0.join("private")).unwrap(),
        b"private bytes"
    );
    fs::remove_file(t.0.join("alias")).unwrap();
    fs::write(t.0.join("alias"), b"independent copy").unwrap();
    assert!(matches!(
        t.scan(|| Ok(())).entries[Path::new("alias")],
        Evidence::Stable { .. }
    ));
}

#[test]
fn sync020_e01_exclusive_flock_defers_and_shared_flock_allows_reading() {
    let t = Tree::new();
    fs::write(t.0.join("file"), b"preserved").unwrap();
    let writer = fs::File::open(t.0.join("file")).unwrap();
    rustix::fs::flock(
        &writer,
        rustix::fs::FlockOperation::NonBlockingLockExclusive,
    )
    .unwrap();
    for _ in 0..2 {
        let inv = t.scan(|| Ok(()));
        assert_eq!(inv.entries[Path::new("file")], Evidence::Writing);
        assert_eq!(inv.observe(Path::new("file")), LocalObservation::Unknown);
    }
    rustix::fs::flock(&writer, rustix::fs::FlockOperation::NonBlockingLockShared).unwrap();
    assert!(matches!(
        t.scan(|| Ok(())).entries[Path::new("file")],
        Evidence::Stable { .. }
    ));
    drop(writer);
    assert_eq!(fs::read(t.0.join("file")).unwrap(), b"preserved");
}

#[test]
fn sync020_n05_link_creation_and_symlink_swap_during_wait_fail_closed() {
    for alias in [false, true] {
        let outside = Tree::new();
        let t = Tree::new();
        fs::write(outside.0.join("private"), b"private").unwrap();
        fs::write(t.0.join("file"), b"local").unwrap();
        let inv = t.scan(|| {
            if alias {
                fs::hard_link(t.0.join("file"), outside.0.join("alias")).unwrap();
            } else {
                fs::remove_file(t.0.join("file")).unwrap();
                std::os::unix::fs::symlink(outside.0.join("private"), t.0.join("file")).unwrap();
            }
            Ok(())
        });
        assert_eq!(inv.observe(Path::new("file")), LocalObservation::Unknown);
        assert_eq!(fs::read(outside.0.join("private")).unwrap(), b"private");
    }
}

#[test]
fn sync020_n05_descriptor_chain_refuses_parent_swaps_and_traversal() {
    let t = Tree::new();
    let outside = Tree::new();
    fs::create_dir(t.0.join("nested")).unwrap();
    fs::write(t.0.join("nested/file"), b"original").unwrap();
    fs::write(outside.0.join("file"), b"private").unwrap();
    let root = Root::open(&t.0).unwrap();
    let (parent, _) = root.parent(Path::new("nested/file")).unwrap();
    let id = crate::download::filesystem::identity(&parent).unwrap();
    fs::rename(t.0.join("nested"), t.0.join("saved")).unwrap();
    std::os::unix::fs::symlink(&outside.0, t.0.join("nested")).unwrap();
    assert!(root.validate_parent(Path::new("nested/file"), id).is_err());
    assert!(stamp(&root, Path::new("nested/file")).is_err());
    for path in ["../file", "/file", "nested/../../file"] {
        assert!(root.parent(Path::new(path)).is_err());
    }
    assert_eq!(fs::read(outside.0.join("file")).unwrap(), b"private");
}
