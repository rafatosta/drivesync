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
    std::fs::create_dir(s.0.join(".drivesync")).unwrap();
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
