use super::*;
use std::{
    os::unix::fs::symlink,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
pub(in crate::detectors) struct Sandbox(pub PathBuf);
impl Sandbox {
    pub(in crate::detectors) fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "drivesync-sources-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn scanner_preserves_bytes_hashes_and_ignores_internal_artifacts_without_following_links() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let s = Sandbox::new();
    std::fs::create_dir(s.0.join("nested")).unwrap();
    std::fs::write(s.0.join("nested/file"), b"abc").unwrap();
    std::fs::write(s.0.join("nested/.~lock.file#"), b"editor lock").unwrap();
    std::fs::create_dir(s.0.join(".drivesync")).unwrap();
    let trash =
        s.0.join(".drivesync-trash-0123456789abcdef0123456789abcdef");
    std::fs::create_dir(&trash).unwrap();
    std::fs::write(trash.join("retained"), b"hidden").unwrap();
    std::fs::write(s.0.join(".drivesync/secret"), b"hidden").unwrap();
    std::fs::write(s.0.join(".drivesync-transfer-private"), b"hidden").unwrap();
    let bytes = PathBuf::from(OsString::from_vec(vec![0xff]));
    std::fs::write(s.0.join(&bytes), b"bytes").unwrap();
    symlink("/etc", s.0.join("link")).unwrap();
    let scan = Scanner::open(s.0.clone(), SyncRootId("scope".into()))
        .unwrap()
        .scan(&CancellationToken::new())
        .unwrap();
    assert_eq!(scan.coverage, ObservationCoverage::Complete);
    assert!(
        matches!(&scan.entries[&PathBuf::from("nested/file")].capabilities, Evidence::Known(c) if c.can_rename == Evidence::Known(true))
    );
    assert_eq!(scan.entries.len(), 4);
    assert!(scan.entries.contains_key(&bytes));
    assert_eq!(
        scan.entries[&PathBuf::from("link")].kind,
        Evidence::Known(FileKind::Symlink)
    );
    assert!(!scan.entries.contains_key(&PathBuf::from("link/passwd")));
    assert_eq!(
        scan.entries[&PathBuf::from("nested/file")].checksum,
        Evidence::Known(Checksum {
            algorithm: "sha256".into(),
            value: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into()
        })
    );
    assert!(scan
        .entries
        .values()
        .all(|o| o.event.as_ref().unwrap().kind == ObservationKind::Snapshot));
    assert_eq!(std::fs::read(s.0.join("nested/file")).unwrap(), b"abc");
}
#[test]
fn cancellation_limits_and_replaced_roots_never_report_complete() {
    let s = Sandbox::new();
    std::fs::write(s.0.join("one"), b"a").unwrap();
    std::fs::write(s.0.join("two"), b"b").unwrap();
    let mut scanner = Scanner::open(s.0.clone(), SyncRootId("scope".into())).unwrap();
    scanner.max_items = 1;
    let scan = scanner.scan(&CancellationToken::new()).unwrap();
    assert_eq!(scan.coverage, ObservationCoverage::Partial);
    assert_eq!(scan.issues[0].kind, ScanIssueKind::LimitReached);
    scanner.expand_limit();
    let retry = scanner.scan(&CancellationToken::new()).unwrap();
    assert_eq!(retry.coverage, ObservationCoverage::Complete);
    assert_eq!(retry.entries.len(), 2);
    let token = CancellationToken::new();
    token.cancel();
    assert!(matches!(scanner.scan(&token), Err(ScanError::Cancelled)));
    let moved = s.0.join("moved");
    let original = s.0.join("root");
    std::fs::create_dir(&original).unwrap();
    let scanner = Scanner::open(original.clone(), SyncRootId("scope".into())).unwrap();
    std::fs::rename(&original, &moved).unwrap();
    std::fs::create_dir(&original).unwrap();
    assert!(matches!(
        scanner.scan(&CancellationToken::new()),
        Err(ScanError::RootChanged)
    ));
}

#[test]
fn denied_content_is_explicit_and_cannot_be_a_complete_snapshot() {
    use std::os::unix::fs::PermissionsExt;
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let s = Sandbox::new();
    std::fs::write(s.0.join("denied"), b"private").unwrap();
    std::fs::set_permissions(s.0.join("denied"), std::fs::Permissions::from_mode(0o000)).unwrap();
    let scan = Scanner::open(s.0.clone(), SyncRootId("scope".into()))
        .unwrap()
        .scan(&CancellationToken::new())
        .unwrap();
    assert_eq!(scan.coverage, ObservationCoverage::Partial);
    assert_eq!(
        scan.entries[&PathBuf::from("denied")].checksum,
        Evidence::PermissionDenied
    );
}

#[test]
#[ignore = "creates 100001 real local files to measure soft-budget recovery and memory; no Drive access"]
fn stage11_large_tree_budget_recovers_without_false_absence() {
    let s = Sandbox::new();
    let count = 100001usize;
    let create = std::time::Instant::now();
    for n in 0..count {
        std::fs::write(s.0.join(format!("file-{n:06}")), []).unwrap();
    }
    println!("created {count} files in {:?}", create.elapsed());
    let mut scanner = Scanner::open(s.0.clone(), SyncRootId("large-tree".into())).unwrap();
    let started = std::time::Instant::now();
    let partial = scanner.scan(&CancellationToken::new()).unwrap();
    assert_eq!(partial.coverage, ObservationCoverage::Partial);
    assert!(partial
        .issues
        .iter()
        .any(|i| i.kind == ScanIssueKind::LimitReached));
    println!(
        "soft limit: {} entries in {:?}",
        partial.entries.len(),
        started.elapsed()
    );
    drop(partial);
    assert!(scanner.expand_limit());
    let started = std::time::Instant::now();
    let complete = scanner.scan(&CancellationToken::new()).unwrap();
    assert_eq!(complete.coverage, ObservationCoverage::Complete);
    assert_eq!(complete.entries.len(), count);
    println!(
        "recovered: {count} entries in {:?}; {}",
        started.elapsed(),
        std::fs::read_to_string("/proc/self/status")
            .unwrap()
            .lines()
            .filter(|l| l.starts_with("VmHWM:") || l.starts_with("VmRSS:"))
            .collect::<Vec<_>>()
            .join("; ")
    );
}

#[test]
fn hashes_are_reused_only_for_stable_native_file_evidence() {
    let sandbox = Sandbox::new();
    let path = sandbox.0.join("file");
    let key = StdPath::new("file");
    let cancel = CancellationToken::new();
    std::fs::write(&path, b"abc").unwrap();
    let scanner = Scanner::open(sandbox.0.clone(), SyncRootId("cache".into())).unwrap();
    let scan = || scanner.scan(&cancel).unwrap().entries[key].checksum.clone();
    let cached = || scanner.hashes.borrow().get(key).unwrap().clone();

    let old = scan();
    let initial = cached();
    assert_eq!(scan(), old);
    assert_eq!(cached().checked, initial.checked);

    let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
    std::thread::sleep(Duration::from_millis(5));
    std::fs::write(&path, b"xyz").unwrap();
    File::open(&path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(mtime))
        .unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), mtime);
    assert_ne!(scan(), old);
    assert!(cached().checked > initial.checked);

    let before = cached();
    std::fs::write(sandbox.0.join("incoming"), b"xyz").unwrap();
    std::fs::rename(sandbox.0.join("incoming"), &path).unwrap();
    scan();
    assert_ne!(cached().stamp.1, before.stamp.1);

    scanner.hashes.borrow_mut().get_mut(key).unwrap().checked =
        Instant::now() - HASH_AUDIT_INTERVAL;
    scan();
    assert!(cached().checked.elapsed() < HASH_AUDIT_INTERVAL);
}

#[test]
fn partial_scans_do_not_carry_hash_evidence_into_recovery() {
    let sandbox = Sandbox::new();
    std::fs::write(sandbox.0.join("a"), b"abc").unwrap();
    std::fs::write(sandbox.0.join("b"), b"xyz").unwrap();
    let cancel = CancellationToken::new();
    let mut scanner = Scanner::open(sandbox.0.clone(), SyncRootId("cache".into())).unwrap();
    assert_eq!(
        scanner.scan(&cancel).unwrap().coverage,
        ObservationCoverage::Complete
    );
    assert_eq!(scanner.hashes.borrow().len(), 2);
    scanner.max_items = 1;
    assert_eq!(
        scanner.scan(&cancel).unwrap().coverage,
        ObservationCoverage::Partial
    );
    assert!(scanner.hashes.borrow().is_empty());
}

#[test]
fn native_events_recheck_dirty_files_and_rebuild_on_directory_changes() {
    let sandbox = Sandbox::new();
    std::fs::create_dir(sandbox.0.join("folder")).unwrap();
    std::fs::write(sandbox.0.join("folder/a"), b"before").unwrap();
    std::fs::write(sandbox.0.join("b"), b"stable").unwrap();
    let cancel = CancellationToken::new();
    let scanner = Scanner::open(sandbox.0.clone(), SyncRootId("watched".into())).unwrap();
    let initial = scanner.poll(None, &cancel).unwrap();
    let started = || scanner.watcher.borrow().as_ref().unwrap().started;
    let original_watch = started();
    let untouched = scanner.hashes.borrow()[StdPath::new("b")].checked;
    let again = scanner.poll(Some(&initial.entries), &cancel).unwrap();
    assert_eq!(started(), original_watch);
    assert_eq!(again.entries, initial.entries);

    std::fs::write(sandbox.0.join("folder/a"), b"temporary").unwrap();
    std::fs::write(sandbox.0.join("folder/a"), b"after").unwrap();
    let updated = scanner.poll(Some(&again.entries), &cancel).unwrap();
    assert_eq!(started(), original_watch);
    assert_eq!(
        scanner.hashes.borrow()[StdPath::new("b")].checked,
        untouched
    );
    assert_eq!(
        updated.entries[StdPath::new("b")],
        again.entries[StdPath::new("b")]
    );
    assert_eq!(
        updated.entries[StdPath::new("folder/a")].checksum,
        Evidence::Known(Checksum {
            algorithm: "sha256".into(),
            value: format!("{:x}", Sha256::digest(b"after")),
        })
    );

    std::fs::rename(sandbox.0.join("folder/a"), sandbox.0.join("moved")).unwrap();
    let moved = scanner.poll(Some(&updated.entries), &cancel).unwrap();
    assert_eq!(started(), original_watch);
    assert_eq!(moved.coverage, ObservationCoverage::Complete);
    assert!(moved.entries.contains_key(StdPath::new("moved")));
    assert!(!moved.entries.contains_key(StdPath::new("folder/a")));

    std::fs::create_dir(sandbox.0.join("new-directory")).unwrap();
    let changed_tree = scanner.poll(Some(&moved.entries), &cancel).unwrap();
    assert!(started() > original_watch);
    assert!(changed_tree
        .entries
        .contains_key(StdPath::new("new-directory")));
}
