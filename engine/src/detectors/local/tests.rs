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

#[test]
fn native_statx_preserves_supported_flags_and_birth_time_without_inference() {
    // LOC-FS-14: native values are obtained separately from model normalization.
    use super::metadata::{LinuxMetadataReader, MetadataReader};
    let tree = Tree::new();
    fs::write(tree.0.join("file"), b"actual metadata").unwrap();
    let descriptor = fs::File::open(tree.0.join("file")).unwrap();
    let native = LinuxMetadataReader.statx(&descriptor).unwrap();
    let result = tree.collector().collect(request(b"file")).unwrap();
    let file = result.file.unwrap();
    assert_eq!(
        file.flags.statx_attributes,
        MetadataValue::Known(native.stx_attributes)
    );
    assert_eq!(
        file.flags.statx_attributes_mask,
        MetadataValue::Known(native.stx_attributes_mask)
    );
    if native.stx_mask & libc::STATX_BTIME != 0 {
        assert_eq!(
            file.times.created_at,
            MetadataValue::Known(crate::models::LocalTimestamp {
                unix_seconds: native.stx_btime.tv_sec,
                nanoseconds: native.stx_btime.tv_nsec
            })
        );
    } else {
        assert_eq!(file.times.created_at, MetadataValue::Unavailable);
    }
}

struct FailedStatx(i32);
impl super::metadata::MetadataReader for FailedStatx {
    fn statx(&self, _: &fs::File) -> std::io::Result<libc::statx> {
        Err(std::io::Error::from_raw_os_error(self.0))
    }
}

#[test]
fn failed_statx_falls_back_to_real_descriptor_metadata_with_explicit_optional_states() {
    // LOC-FS-14: only statx fails; navigation/fstat and symlink handling remain real.
    let tree = Tree::new();
    fs::write(tree.0.join("file"), b"fallback").unwrap();
    let collector = tree.collector();
    for (errno, state) in [
        (libc::ENOSYS, MetadataValue::Unsupported),
        (libc::EOPNOTSUPP, MetadataValue::Unsupported),
        (libc::EPERM, MetadataValue::PermissionDenied),
        (libc::EIO, MetadataValue::Unavailable),
    ] {
        let result = collector
            .collect_using(request(b"file"), |_| {}, &FailedStatx(errno))
            .unwrap();
        assert_eq!(result.coverage, LocalScanCoverage::Complete);
        let file = result.file.unwrap();
        assert_eq!(file.kind, MetadataValue::Known(LocalEntryKind::Regular));
        assert_eq!(file.storage.size_bytes, MetadataValue::Known(8));
        assert_eq!(file.flags.statx_attributes, state);
        assert_eq!(
            file.flags.statx_attributes_mask,
            file.flags.statx_attributes
        );
        assert!(matches!(
            (errno, file.times.created_at),
            (libc::ENOSYS | libc::EOPNOTSUPP, MetadataValue::Unsupported)
                | (libc::EPERM, MetadataValue::PermissionDenied)
                | (libc::EIO, MetadataValue::Unavailable)
        ));
    }
}

struct MissingStatxBit(u32);
impl super::metadata::MetadataReader for MissingStatxBit {
    fn statx(&self, file: &fs::File) -> std::io::Result<libc::statx> {
        let mut stat = super::metadata::LinuxMetadataReader.statx(file)?;
        stat.stx_mask &= !self.0;
        // Deliberately nonzero: missing-mask data must not be treated as evidence.
        stat.stx_uid = 7654;
        Ok(stat)
    }
}

#[test]
fn missing_statx_basic_mask_bits_produce_partial_metadata_instead_of_invented_values() {
    // LOC-FS-14 / LOC-MOD-02.
    let tree = Tree::new();
    fs::write(tree.0.join("file"), b"x").unwrap();
    let collector = tree.collector();
    for bit in [
        libc::STATX_TYPE,
        libc::STATX_MODE,
        libc::STATX_UID,
        libc::STATX_GID,
        libc::STATX_MTIME,
        libc::STATX_CTIME,
        libc::STATX_SIZE,
        libc::STATX_INO,
        libc::STATX_NLINK,
    ] {
        let result = collector
            .collect_using(request(b"file"), |_| {}, &MissingStatxBit(bit))
            .unwrap();
        assert_eq!(result.coverage, LocalScanCoverage::Partial);
        let file = result.file.unwrap();
        assert_eq!(file.validate(), Ok(()));
        match bit {
            libc::STATX_TYPE => {
                assert_eq!(file.kind, MetadataValue::Unavailable);
                assert_eq!(file.mode, MetadataValue::Unavailable);
                assert_eq!(file.symlink.target, MetadataValue::NotCollected);
            }
            libc::STATX_MODE => assert_eq!(file.mode, MetadataValue::Unavailable),
            libc::STATX_UID => assert_eq!(file.owner.uid, MetadataValue::Unavailable),
            libc::STATX_GID => assert_eq!(file.owner.gid, MetadataValue::Unavailable),
            libc::STATX_MTIME => assert_eq!(file.times.modified_at, MetadataValue::Unavailable),
            libc::STATX_CTIME => assert_eq!(file.times.changed_at, MetadataValue::Unavailable),
            libc::STATX_SIZE => assert_eq!(file.storage.size_bytes, MetadataValue::Unavailable),
            libc::STATX_INO => assert_eq!(file.storage.inode, MetadataValue::Unavailable),
            libc::STATX_NLINK => {
                assert_eq!(file.storage.hard_link_count, MetadataValue::Unavailable)
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn missing_optional_birth_time_does_not_degrade_complete_basic_collection() {
    // LOC-MOD-01: NotCollected/Unsupported are not invented from a missing mask bit.
    let tree = Tree::new();
    fs::write(tree.0.join("file"), b"x").unwrap();
    let result = tree
        .collector()
        .collect_using(
            request(b"file"),
            |_| {},
            &MissingStatxBit(libc::STATX_BTIME),
        )
        .unwrap();
    assert_eq!(result.coverage, LocalScanCoverage::Complete);
    assert_eq!(
        result.file.unwrap().times.created_at,
        MetadataValue::Unavailable
    );
}

struct FailedLink(i32);
impl super::metadata::MetadataReader for FailedLink {
    fn statx(&self, file: &fs::File) -> std::io::Result<libc::statx> {
        super::metadata::LinuxMetadataReader.statx(file)
    }
    fn read_link(
        &self,
        _: &fs::File,
        _: &LocalCancellation,
    ) -> LocalDetectorResult<std::io::Result<Vec<u8>>> {
        Ok(Err(std::io::Error::from_raw_os_error(self.0)))
    }
}

#[test]
fn failed_literal_target_read_keeps_partial_symlink_metadata_without_fabricating_target() {
    // LOC-FS-15: syscall failure at the target boundary, not a fake collector.
    let tree = Tree::new();
    symlink("../outside", tree.0.join("link")).unwrap();
    let collector = tree.collector();
    for (errno, expected) in [
        (libc::EACCES, MetadataValue::PermissionDenied),
        (libc::ENOSYS, MetadataValue::Unsupported),
        (libc::EOVERFLOW, MetadataValue::Unavailable),
        (libc::EIO, MetadataValue::Unavailable),
    ] {
        let result = collector
            .collect_using(request(b"link"), |_| {}, &FailedLink(errno))
            .unwrap();
        assert_eq!(result.coverage, LocalScanCoverage::Partial);
        let file = result.file.unwrap();
        assert_eq!(file.kind, MetadataValue::Known(LocalEntryKind::Symlink));
        assert_eq!(file.symlink.target, expected);
        assert_eq!(file.validate(), Ok(()));
    }
}

struct CancelAfterStatx(LocalCancellation);
impl super::metadata::MetadataReader for CancelAfterStatx {
    fn statx(&self, file: &fs::File) -> std::io::Result<libc::statx> {
        let result = super::metadata::LinuxMetadataReader.statx(file);
        self.0.cancel();
        result
    }
    fn read_link(
        &self,
        _: &fs::File,
        _: &LocalCancellation,
    ) -> LocalDetectorResult<std::io::Result<Vec<u8>>> {
        panic!("cancelled collection must not perform the next I/O operation");
    }
}

#[test]
fn cancellation_between_statx_and_readlink_prevents_the_next_io_operation() {
    // LOC-A-03 / LOC-FS-08.
    let tree = Tree::new();
    symlink("../outside", tree.0.join("link")).unwrap();
    let req = request(b"link");
    let backend = CancelAfterStatx(req.cancellation.clone());
    assert_eq!(
        tree.collector()
            .collect_using(req, |_| {}, &backend)
            .unwrap_err()
            .kind,
        LocalDetectorError::Cancelled
    );
}

#[test]
fn possibly_truncated_readlink_is_bounded_and_never_claimed_known() {
    // LOC-FS-15: emulate only the readlink count, including an always-full buffer.
    let mut capacities = Vec::new();
    let result = super::filesystem::read_link_with(&LocalCancellation::new(), |bytes| {
        capacities.push(bytes.len());
        Ok(bytes.len())
    })
    .unwrap()
    .unwrap_err();
    assert_eq!(result.raw_os_error(), Some(libc::EOVERFLOW));
    assert_eq!(
        capacities,
        [128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536]
    );
}

#[test]
fn cancellation_during_link_read_does_not_trigger_a_buffer_retry() {
    // LOC-FS-08/15.
    let cancel = LocalCancellation::new();
    let mut calls = 0;
    let result = super::filesystem::read_link_with(&cancel, |bytes| {
        calls += 1;
        cancel.cancel();
        Ok(bytes.len())
    });
    assert_eq!(result.unwrap_err().kind, LocalDetectorError::Cancelled);
    assert_eq!(calls, 1);
}

struct InvalidNativeTime(bool);
impl super::metadata::MetadataReader for InvalidNativeTime {
    fn statx(&self, file: &fs::File) -> std::io::Result<libc::statx> {
        let mut native = super::metadata::LinuxMetadataReader.statx(file)?;
        native.stx_mtime.tv_nsec = u32::MAX;
        if self.0 {
            native.stx_mask &= !libc::STATX_MTIME;
        }
        Ok(native)
    }
}

#[test]
fn invalid_known_native_metadata_is_typed_but_unavailable_masked_fields_are_not_used() {
    // LOC-FS-14: masked bytes cannot cause a false known value or invalid timestamp.
    let tree = Tree::new();
    fs::write(tree.0.join("file"), b"x").unwrap();
    let collector = tree.collector();
    assert_eq!(
        collector
            .collect_using(request(b"file"), |_| {}, &InvalidNativeTime(false))
            .unwrap_err()
            .kind,
        LocalDetectorError::ModelInvalid
    );
    let result = collector
        .collect_using(request(b"file"), |_| {}, &InvalidNativeTime(true))
        .unwrap();
    assert_eq!(result.coverage, LocalScanCoverage::Partial);
    assert_eq!(
        result.file.unwrap().times.modified_at,
        MetadataValue::Unavailable
    );
}
