//! External consumer of only the public detector collection surface.
use drivesync_engine::{
    detectors::{
        LocalCancellation, LocalDetectorError, LocalDetectorScope, LocalMetadataCollector,
        LocalMetadataRequest, LocalScanCoverage,
    },
    models::{AccountId, LocalEntryKind, LocalRelativePath, MetadataValue, SyncId},
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "drivesync-public-collection-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("fixture creation failed: {e}"),
            }
        }
    }
    fn scope(&self) -> LocalDetectorScope {
        LocalDetectorScope {
            account_id: AccountId("public-test-account".into()),
            sync_id: SyncId("public-test-sync".into()),
            local_root: self.0.clone(),
        }
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn request(path: &[u8]) -> LocalMetadataRequest {
    LocalMetadataRequest {
        relative_path: LocalRelativePath::new(path.to_vec()).unwrap(),
        cancellation: LocalCancellation::new(),
    }
}

#[test]
fn public_consumer_constructs_queries_scope_and_collects_without_internal_traits() {
    let tree = Tree::new();
    fs::write(tree.0.join("file"), b"public collection").unwrap();
    let collector = LocalMetadataCollector::new(tree.scope()).unwrap();
    assert_eq!(collector.scope(), tree.scope());
    let result = collector.collect(request(b"file")).unwrap();
    assert_eq!(result.coverage, LocalScanCoverage::Complete);
    let file = result.file.unwrap();
    assert_eq!(file.kind, MetadataValue::Known(LocalEntryKind::Regular));
    assert_eq!(file.storage.size_bytes, MetadataValue::Known(17));
    assert_eq!(file.relative_path.as_bytes(), b"file");
    assert_eq!(file.local_id, None);
    assert_eq!(file.validate(), Ok(()));
}

#[test]
fn public_consumer_receives_sanitized_typed_errors_and_partial_absence() {
    let tree = Tree::new();
    let invalid = LocalDetectorScope {
        account_id: AccountId(" ".into()),
        ..tree.scope()
    };
    let error = LocalMetadataCollector::new(invalid).err().unwrap();
    assert_eq!(error.kind, LocalDetectorError::InvalidScope);
    assert!(!error.retryable);
    assert!(error.progress.is_none());
    assert_eq!(error.to_string(), "invalid local scope");
    let collector = LocalMetadataCollector::new(tree.scope()).unwrap();
    let result = collector.collect(request(b"missing")).unwrap();
    assert!(result.file.is_none());
    assert_eq!(result.coverage, LocalScanCoverage::Partial);
}

#[test]
fn public_cancellation_can_be_shared_cancelled_and_defaulted_independently() {
    let tree = Tree::new();
    fs::write(tree.0.join("file"), b"x").unwrap();
    let collector = LocalMetadataCollector::new(tree.scope()).unwrap();
    let cancellation = LocalCancellation::new();
    let other = cancellation.clone();
    cancellation.cancel();
    assert!(other.is_cancelled());
    let error = collector
        .collect(LocalMetadataRequest {
            cancellation: other,
            ..request(b"file")
        })
        .unwrap_err();
    assert_eq!(error.kind, LocalDetectorError::Cancelled);
    let fresh = LocalCancellation::default();
    assert!(!fresh.is_cancelled());
    assert!(collector
        .collect(LocalMetadataRequest {
            cancellation: fresh,
            ..request(b"file")
        })
        .unwrap()
        .file
        .is_some());
}

#[test]
fn public_collector_revalidates_root_and_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<LocalMetadataCollector>();
    assert_send_sync::<LocalCancellation>();
    let tree = Tree::new();
    fs::create_dir(tree.0.join("root")).unwrap();
    let scope = LocalDetectorScope {
        local_root: tree.0.join("root"),
        ..tree.scope()
    };
    let collector = LocalMetadataCollector::new(scope.clone()).unwrap();
    fs::rename(tree.0.join("root"), tree.0.join("old-root")).unwrap();
    fs::create_dir(tree.0.join("root")).unwrap();
    fs::write(tree.0.join("root/file"), b"replacement").unwrap();
    assert_eq!(collector.scope(), scope);
    let error = collector.collect(request(b"file")).unwrap_err();
    assert_eq!(error.kind, LocalDetectorError::RootUnavailable);
    assert!(error.retryable);
}
