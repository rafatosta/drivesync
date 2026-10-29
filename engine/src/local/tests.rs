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
fn unreadable_files_are_unknown_and_unreadable_directories_fail_the_complete_scan() {
    use std::os::unix::fs::PermissionsExt;
    let t = Tree::new();
    fs::write(t.0.join("file"), b"preserved").unwrap();
    fs::set_permissions(t.0.join("file"), fs::Permissions::from_mode(0o000)).unwrap();
    let inv = t.scan(|| Ok(()));
    assert_eq!(inv.entries[Path::new("file")], Evidence::Unknown);
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
