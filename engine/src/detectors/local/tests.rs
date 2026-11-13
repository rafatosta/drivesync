//! Physical collector oracles use only isolated temporary files and explicit races.
use super::{collector::*, models::*};
use crate::models::{AccountId, LocalEntryKind, LocalRelativePath, MetadataValue, SyncId};
use std::{
    fs,
    os::unix::{
        ffi::OsStrExt,
        fs::{symlink, MetadataExt, PermissionsExt},
        net::UnixListener,
    },
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = std::env::temp_dir();
        loop {
            let path = base.join(format!(
                "drivesync-collector-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("temporary fixture creation failed: {error}"),
            }
        }
    }
    fn scope(&self) -> LocalDetectorScope {
        LocalDetectorScope {
            account_id: AccountId("test-account".into()),
            sync_id: SyncId("test-sync".into()),
            local_root: self.0.clone(),
        }
    }
    fn collector(&self) -> FilesystemMetadataCollector {
        FilesystemMetadataCollector::new(self.scope()).unwrap()
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
fn collects_real_file_metadata_and_preserves_root_scope() {
    // LOC-FS-01/03: expectations come from the fixture and independent stat.
    let tree = Tree::new();
    fs::write(tree.0.join("file"), b"known content").unwrap();
    let expected = fs::symlink_metadata(tree.0.join("file")).unwrap();
    let collector = tree.collector();
    assert_eq!(collector.scope(), tree.scope());
    let result = collector.collect(request(b"file")).unwrap();
    assert_eq!(result.coverage, LocalScanCoverage::Complete);
    let file = result.file.unwrap();
    assert_eq!(file.relative_path.as_bytes(), b"file");
    assert_eq!(file.local_id, None);
    assert_eq!(file.kind, MetadataValue::Known(LocalEntryKind::Regular));
    assert_eq!(file.storage.size_bytes, MetadataValue::Known(13));
    assert_eq!(file.storage.inode, MetadataValue::Known(expected.ino()));
    assert_eq!(file.owner.uid, MetadataValue::Known(expected.uid()));
    assert_eq!(file.owner.gid, MetadataValue::Known(expected.gid()));
    assert_eq!(
        file.times.modified_at,
        MetadataValue::Known(crate::models::LocalTimestamp {
            unix_seconds: expected.mtime(),
            nanoseconds: expected.mtime_nsec() as u32
        })
    );
    assert_eq!(
        file.times.changed_at,
        MetadataValue::Known(crate::models::LocalTimestamp {
            unix_seconds: expected.ctime(),
            nanoseconds: expected.ctime_nsec() as u32
        })
    );
    assert_eq!(
        file.mode,
        MetadataValue::Known(crate::models::LocalFileMode {
            raw_mode: expected.mode(),
            permission_bits: (expected.mode() & 0o7777) as u16
        })
    );
    assert_eq!(file.validate(), Ok(()));
    assert_eq!(fs::read(tree.0.join("file")).unwrap(), b"known content");
}

#[test]
fn collects_directories_hardlinks_and_special_entries_without_content_io() {
    // LOC-FS-03/05: O_PATH must not wait for a FIFO reader/writer.
    let tree = Tree::new();
    fs::create_dir(tree.0.join("directory")).unwrap();
    fs::write(tree.0.join("original"), b"x").unwrap();
    fs::hard_link(tree.0.join("original"), tree.0.join("alias")).unwrap();
    let fifo = std::ffi::CString::new(tree.0.join("fifo").as_os_str().as_bytes()).unwrap();
    // SAFETY: the CString is a valid fixture pathname; no descriptor is borrowed.
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    let _listener = UnixListener::bind(tree.0.join("socket")).unwrap();
    let collector = tree.collector();
    for (name, kind) in [
        (b"directory".as_slice(), LocalEntryKind::Directory),
        (b"fifo", LocalEntryKind::Fifo),
        (b"socket", LocalEntryKind::Socket),
    ] {
        let result = collector.collect(request(name)).unwrap();
        assert_eq!(result.coverage, LocalScanCoverage::Complete);
        assert_eq!(result.file.unwrap().kind, MetadataValue::Known(kind));
    }
    let result = collector.collect(request(b"alias")).unwrap().file.unwrap();
    assert_eq!(result.storage.hard_link_count, MetadataValue::Known(2));
    assert_eq!(result.local_id, None);
}

#[test]
fn symlinks_and_non_utf8_names_remain_literal_even_outside_the_root() {
    // LOC-FS-04: a broken target cannot provide invented target metadata.
    let tree = Tree::new();
    let name = std::ffi::OsStr::from_bytes(b"link-\xff");
    let target = std::ffi::OsStr::from_bytes(b"/nonexistent/../outside-\xfe");
    symlink(target, tree.0.join(name)).unwrap();
    let result = tree.collector().collect(request(name.as_bytes())).unwrap();
    assert_eq!(result.coverage, LocalScanCoverage::Complete);
    let file = result.file.unwrap();
    assert_eq!(file.relative_path.as_bytes(), name.as_bytes());
    assert_eq!(file.kind, MetadataValue::Known(LocalEntryKind::Symlink));
    assert_eq!(
        file.symlink.target,
        MetadataValue::Known(target.as_bytes().to_vec())
    );
    assert_eq!(
        file.storage.size_bytes,
        MetadataValue::Known(target.as_bytes().len() as u64)
    );
}

#[test]
fn missing_entries_and_reserved_metadata_never_become_confirmed_absence() {
    // LOC-FS-07.
    let tree = Tree::new();
    let collector = tree.collector();
    let missing = collector.collect(request(b"missing")).unwrap();
    assert_eq!(missing.coverage, LocalScanCoverage::Partial);
    assert!(missing.file.is_none());
    for path in [b".drivesync".as_slice(), b"folder/.drivesync/metadata"] {
        assert_eq!(
            collector.collect(request(path)).unwrap_err().kind,
            LocalDetectorError::InvalidScope
        );
    }
}

#[test]
fn rejects_invalid_scope_and_symbolic_root_ancestors() {
    // LOC-FS-02.
    let tree = Tree::new();
    for scope in [
        LocalDetectorScope {
            local_root: PathBuf::from("relative"),
            ..tree.scope()
        },
        LocalDetectorScope {
            account_id: AccountId(" ".into()),
            ..tree.scope()
        },
        LocalDetectorScope {
            sync_id: SyncId("".into()),
            ..tree.scope()
        },
        LocalDetectorScope {
            local_root: tree.0.join(".drivesync"),
            ..tree.scope()
        },
    ] {
        assert_eq!(
            FilesystemMetadataCollector::new(scope).err().unwrap().kind,
            LocalDetectorError::InvalidScope
        );
    }
    let outside = Tree::new();
    symlink(&outside.0, tree.0.join("linked")).unwrap();
    for root in [
        tree.0.join("linked"),
        tree.0.join("linked/child"),
        tree.0.join("missing"),
    ] {
        assert_eq!(
            FilesystemMetadataCollector::new(LocalDetectorScope {
                local_root: root,
                ..tree.scope()
            })
            .err()
            .unwrap()
            .kind,
            LocalDetectorError::RootUnavailable
        );
    }
}

#[test]
fn cancellation_is_shared_but_new_tokens_are_independent() {
    // LOC-FS-08 / LOC-A-07: cancel before even opening the now-missing root.
    let tree = Tree::new();
    let collector = tree.collector();
    let cancel = LocalCancellation::new();
    let copy = cancel.clone();
    assert!(!cancel.is_cancelled());
    cancel.cancel();
    assert!(copy.is_cancelled());
    assert!(!LocalCancellation::new().is_cancelled());
    fs::remove_dir(&tree.0).unwrap();
    let error = collector
        .collect(LocalMetadataRequest {
            cancellation: copy,
            ..request(b"file")
        })
        .unwrap_err();
    assert_eq!(error.kind, LocalDetectorError::Cancelled);
    fs::create_dir(&tree.0).unwrap();
}

#[test]
fn root_replacement_is_rejected_and_scope_query_does_not_touch_disk() {
    // LOC-FS-01/09.
    let tree = Tree::new();
    fs::create_dir(tree.0.join("root")).unwrap();
    let scope = LocalDetectorScope {
        local_root: tree.0.join("root"),
        ..tree.scope()
    };
    let collector = FilesystemMetadataCollector::new(scope.clone()).unwrap();
    fs::rename(tree.0.join("root"), tree.0.join("old-root")).unwrap();
    fs::create_dir(tree.0.join("root")).unwrap();
    fs::write(tree.0.join("root/file"), b"replacement").unwrap();
    assert_eq!(collector.scope(), scope);
    assert_eq!(
        collector.collect(request(b"file")).unwrap_err().kind,
        LocalDetectorError::RootUnavailable
    );
}

#[test]
fn relative_symbolic_ancestor_is_rejected_without_reading_its_target() {
    // LOC-FS-10.
    let tree = Tree::new();
    let outside = Tree::new();
    fs::write(outside.0.join("private"), b"secret").unwrap();
    symlink(&outside.0, tree.0.join("link")).unwrap();
    assert_eq!(
        tree.collector()
            .collect(request(b"link/private"))
            .unwrap_err()
            .kind,
        LocalDetectorError::PartialObservation
    );
    assert_eq!(fs::read(outside.0.join("private")).unwrap(), b"secret");
}

#[test]
fn permission_denial_is_real_but_file_read_permission_is_not_required() {
    // LOC-FS-06: never infer permission failure only from POSIX mode bits.
    let tree = Tree::new();
    fs::write(tree.0.join("unreadable"), b"metadata only").unwrap();
    fs::set_permissions(tree.0.join("unreadable"), fs::Permissions::from_mode(0o0)).unwrap();
    let read_error = fs::read(tree.0.join("unreadable")).unwrap_err();
    assert_eq!(read_error.kind(), std::io::ErrorKind::PermissionDenied);
    let result = tree.collector().collect(request(b"unreadable")).unwrap();
    assert_eq!(result.coverage, LocalScanCoverage::Complete);
    fs::create_dir(tree.0.join("blocked")).unwrap();
    fs::write(tree.0.join("blocked/file"), b"private").unwrap();
    fs::set_permissions(tree.0.join("blocked"), fs::Permissions::from_mode(0o0)).unwrap();
    let denied = tree.collector().collect(request(b"blocked/file"));
    let proof = fs::symlink_metadata(tree.0.join("blocked/file"));
    fs::set_permissions(tree.0.join("blocked"), fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        proof.unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    let error = denied.unwrap_err();
    assert_eq!(error.kind, LocalDetectorError::PermissionDenied);
    assert!(!error
        .message
        .contains(&tree.0.to_string_lossy().to_string()));
    assert!(error.progress.is_none());
}

#[test]
fn literal_link_target_larger_than_initial_buffer_is_not_truncated() {
    // LOC-FS-12.
    let tree = Tree::new();
    let target = "x".repeat(1024);
    symlink(&target, tree.0.join("long-link")).unwrap();
    assert_eq!(
        tree.collector()
            .collect(request(b"long-link"))
            .unwrap()
            .file
            .unwrap()
            .symlink
            .target,
        MetadataValue::Known(target.into_bytes())
    );
}

#[test]
fn concurrent_roots_and_requests_remain_isolated() {
    // LOC-FS-01/13; no shared mutable per-request metadata.
    let first = Tree::new();
    let second = Tree::new();
    fs::write(first.0.join("file"), b"one").unwrap();
    fs::write(second.0.join("file"), b"longer").unwrap();
    let a = first.collector();
    let b = second.collector();
    std::thread::scope(|threads| {
        let mut handles = Vec::new();
        for _ in 0..8 {
            handles.push(threads.spawn(|| a.collect(request(b"file")).unwrap()));
            handles.push(threads.spawn(|| b.collect(request(b"file")).unwrap()));
        }
        for (index, handle) in handles.into_iter().enumerate() {
            assert_eq!(
                handle.join().unwrap().file.unwrap().storage.size_bytes,
                MetadataValue::Known(if index % 2 == 0 { 3 } else { 6 })
            );
        }
    });
}

#[test]
fn cancellation_after_open_prevents_a_metadata_result() {
    // LOC-FS-08: exact safe point, not timing-dependent cancellation.
    let tree = Tree::new();
    fs::write(tree.0.join("file"), b"x").unwrap();
    let collector = tree.collector();
    let req = request(b"file");
    let cancel = req.cancellation.clone();
    let result = collector.collect_with(req, |stage| {
        if stage == CollectionStage::Opened {
            cancel.cancel();
        }
    });
    assert_eq!(result.unwrap_err().kind, LocalDetectorError::Cancelled);
}

#[test]
fn replaced_target_with_identical_bytes_is_still_inconclusive() {
    // LOC-FS-11: descriptor identity, not names/content similarity.
    let tree = Tree::new();
    fs::write(tree.0.join("file"), b"same").unwrap();
    let result = tree
        .collector()
        .collect_with(request(b"file"), |stage| {
            if stage == CollectionStage::Opened {
                fs::rename(tree.0.join("file"), tree.0.join("old")).unwrap();
                fs::write(tree.0.join("file"), b"same").unwrap();
            }
        })
        .unwrap();
    assert!(result.file.is_none());
    assert_eq!(result.coverage, LocalScanCoverage::Partial);
}

#[test]
fn edits_after_observation_do_not_publish_a_complete_snapshot() {
    // LOC-FS-11: size change is deterministic even on coarse timestamp filesystems.
    let tree = Tree::new();
    fs::write(tree.0.join("file"), b"before").unwrap();
    let result = tree
        .collector()
        .collect_with(request(b"file"), |stage| {
            if stage == CollectionStage::Observed {
                fs::write(tree.0.join("file"), b"after, with more bytes").unwrap();
            }
        })
        .unwrap();
    assert!(result.file.is_none());
    assert_eq!(result.coverage, LocalScanCoverage::Partial);
}

#[test]
fn moving_a_parent_outside_the_root_invalidates_held_directory_evidence() {
    // LOC-FS-10: an open directory fd alone does not establish current ancestry.
    let tree = Tree::new();
    fs::create_dir_all(tree.0.join("root/sub")).unwrap();
    fs::write(tree.0.join("root/sub/file"), b"x").unwrap();
    let collector = FilesystemMetadataCollector::new(LocalDetectorScope {
        local_root: tree.0.join("root"),
        ..tree.scope()
    })
    .unwrap();
    let result = collector.collect_with(request(b"sub/file"), |stage| {
        if stage == CollectionStage::Opened {
            fs::rename(tree.0.join("root/sub"), tree.0.join("moved-out")).unwrap();
            symlink(tree.0.join("moved-out"), tree.0.join("root/sub")).unwrap();
        }
    });
    assert_eq!(
        result.unwrap_err().kind,
        LocalDetectorError::PartialObservation
    );
    assert_eq!(fs::read(tree.0.join("moved-out/file")).unwrap(), b"x");
}

#[test]
fn replacing_an_absolute_ancestor_during_collection_does_not_adopt_a_new_root() {
    // LOC-FS-09: absolute ancestry is revalidated as well as the final root inode.
    let tree = Tree::new();
    fs::create_dir_all(tree.0.join("parent/root")).unwrap();
    fs::write(tree.0.join("parent/root/file"), b"x").unwrap();
    let collector = FilesystemMetadataCollector::new(LocalDetectorScope {
        local_root: tree.0.join("parent/root"),
        ..tree.scope()
    })
    .unwrap();
    let result = collector.collect_with(request(b"file"), |stage| {
        if stage == CollectionStage::Opened {
            fs::rename(tree.0.join("parent"), tree.0.join("old-parent")).unwrap();
            fs::create_dir_all(tree.0.join("parent/root")).unwrap();
        }
    });
    assert_eq!(
        result.unwrap_err().kind,
        LocalDetectorError::RootUnavailable
    );
}
