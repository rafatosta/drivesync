//! Same-filesystem freedesktop trash. Cross-device moves fail without a destructive fallback.
use super::*;
use crate::inventory::models::{RecoveryRecord, TrashReference};

pub(super) struct TrashDestination {
    destination: Entry,
    pub(super) reference: TrashReference,
}
impl TrashDestination {
    pub(super) fn commit(self, source: &Entry) -> Result<(), OperatorError> {
        rename(source, &self.destination, libc::RENAME_NOREPLACE)?;
        files_sync(&source.parent).map_err(OperatorError::uncertain)?;
        files_sync(&self.destination.parent).map_err(OperatorError::uncertain)
    }
}
pub(super) fn prepare(
    source: &Entry,
    original: &StdPath,
) -> Result<TrashDestination, OperatorError> {
    prepare_to(source, original, &data_home()?)
}

pub(super) fn prepare_replacement(
    source: &Entry,
    original: &StdPath,
) -> Result<Option<TrashDestination>, OperatorError> {
    prepare_replacement_to(source, original, &data_home()?)
}

pub(super) fn prepare_replacement_to(
    source: &Entry,
    original: &StdPath,
    data: &StdPath,
) -> Result<Option<TrashDestination>, OperatorError> {
    prepare_destination(source, original, data, true)
}

fn mount_id(directory: &File) -> Result<u64, OperatorError> {
    let mut stat = std::mem::MaybeUninit::<libc::statx>::zeroed();
    if unsafe {
        libc::statx(
            directory.as_raw_fd(),
            c"".as_ptr(),
            libc::AT_EMPTY_PATH,
            libc::STATX_MNT_ID,
            stat.as_mut_ptr(),
        )
    } != 0
    {
        return Err(error(std::io::Error::last_os_error()));
    }
    let stat = unsafe { stat.assume_init() };
    if stat.stx_mask & libc::STATX_MNT_ID == 0 {
        return Err(OperatorError::new(OperatorErrorKind::Unavailable));
    }
    Ok(stat.stx_mnt_id)
}

fn private_child(parent: &File, name: &str) -> Result<File, OperatorError> {
    let parent_path = PathBuf::from(format!("/proc/self/fd/{}", parent.as_raw_fd()));
    let path = parent_path.join(name);
    let child = CString::new(name).map_err(|_| invalid())?;
    if unsafe { libc::mkdirat(parent.as_raw_fd(), child.as_ptr(), 0o700) } != 0 {
        let error_value = std::io::Error::last_os_error();
        if error_value.kind() != std::io::ErrorKind::AlreadyExists {
            return Err(error(error_value));
        }
    }
    let dir = directory(&path)?;
    let meta = dir.metadata().map_err(error)?;
    if meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
        return Err(OperatorError::new(OperatorErrorKind::PermissionDenied));
    }
    Ok(dir)
}
pub(super) fn prepare_to(
    source: &Entry,
    original: &StdPath,
    data: &StdPath,
) -> Result<TrashDestination, OperatorError> {
    prepare_destination(source, original, data, false)?
        .ok_or_else(|| OperatorError::new(OperatorErrorKind::Unavailable))
}

fn prepare_destination(
    source: &Entry,
    original: &StdPath,
    data: &StdPath,
    retain_on_other_mount: bool,
) -> Result<Option<TrashDestination>, OperatorError> {
    let root = Filesystem::open(data)?;
    let trash = private_child(&root.root, "Trash")?;
    let files = private_child(&trash, "files")?;
    let info = private_child(&trash, "info")?;
    // Flatpak bind mounts can have the same device number and still reject rename.
    if mount_id(&source.parent)? != mount_id(&files)? {
        return if retain_on_other_mount {
            Ok(None)
        } else {
            Err(OperatorError::new(OperatorErrorKind::Unavailable))
        };
    }
    if std::fs::symlink_metadata(source.anchored())
        .map_err(error)?
        .dev()
        != files.metadata().map_err(error)?.dev()
    {
        return Err(OperatorError::new(OperatorErrorKind::Unavailable));
    }
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let key = execution_key(&Id(format!(
        "{}-{}-{}",
        std::process::id(),
        super::super::now().unix_millis,
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )));
    let info_path = PathBuf::from(format!("/proc/self/fd/{}", info.as_raw_fd()))
        .join(format!("{key}.trashinfo"));
    let mut receipt = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(info_path)
        .map_err(error)?;
    let encoded: String = original
        .as_os_str()
        .as_bytes()
        .iter()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"/-_.~".contains(b) {
                (*b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    let seconds = unsafe { libc::time(std::ptr::null_mut()) };
    let mut tm = std::mem::MaybeUninit::<libc::tm>::uninit();
    if unsafe { libc::localtime_r(&seconds, tm.as_mut_ptr()) }.is_null() {
        return Err(invalid());
    }
    let tm = unsafe { tm.assume_init() };
    let text = format!(
        "[Trash Info]\nPath={encoded}\nDeletionDate={:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    );
    receipt.write_all(text.as_bytes()).map_err(error)?;
    receipt.sync_all().map_err(error)?;
    let dest = Entry {
        parent: files,
        name: CString::new(key.clone()).unwrap(),
    };
    let parent_identity = Filesystem {
        root: dest.parent.try_clone().map_err(error)?,
        absolute: data.join("Trash/files"),
    }
    .root_identity()?;
    let reference = TrashReference {
        destination: data.join("Trash/files").join(&key),
        parent_identity,
        info_checksum: Checksum {
            algorithm: "sha256".into(),
            value: format!("{:x}", Sha256::digest(text.as_bytes())),
        },
    };
    files_sync(&dest.parent)?;
    files_sync(&info)?;
    Ok(Some(TrashDestination {
        destination: dest,
        reference,
    }))
}

fn files_sync(directory: &File) -> Result<(), OperatorError> {
    directory.sync_all().map_err(error)
}
fn data_home() -> Result<PathBuf, OperatorError> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .ok_or_else(invalid)
}
impl Filesystem {
    /// A missing slot alone cannot distinguish a move or permission failure from recoverable removal.
    pub(in crate::orchestrator::operations) fn confirm_missing_incarnation(
        &self,
        expected: &LocalState,
        scope: &SyncRootId,
    ) -> Result<(), OperatorError> {
        use crate::detectors::observation::{
            remote::interfaces::CancellationToken, scanner::Scanner, ObservationCoverage,
        };
        self.validate_root()?;
        self.check_absent(expected.path.as_ref().ok_or_else(stale)?)?;
        let scanner = Scanner::open(self.absolute.clone(), scope.clone()).map_err(|_| stale())?;
        let scan = scanner
            .scan(&CancellationToken::new())
            .map_err(|_| stale())?;
        if scan.coverage != ObservationCoverage::Complete
            || scan
                .entries
                .values()
                .any(|e| e.identity == expected.identity)
        {
            return Err(stale());
        }
        self.verify_trashed_source(expected)
    }
    /// Confirm that the missing incarnation is recoverable in freedesktop trash, not moved out.
    pub(in crate::orchestrator::operations) fn verify_trashed_source(
        &self,
        expected: &LocalState,
    ) -> Result<(), OperatorError> {
        if !matches!(&expected.identity,Evidence::Known(id) if id.birth_ns.is_some()) {
            return Err(stale());
        }
        let data = data_home()?;
        let files_path = data.join("Trash/files");
        let trash = Filesystem::open(&files_path)?;
        let info = Filesystem::open(&data.join("Trash/info"))?;
        let original = self
            .absolute
            .join(expected.path.as_ref().ok_or_else(stale)?);
        let encoded: String = original
            .as_os_str()
            .as_bytes()
            .iter()
            .map(|b| {
                if b.is_ascii_alphanumeric() || b"/-_.~".contains(b) {
                    (*b as char).to_string()
                } else {
                    format!("%{b:02X}")
                }
            })
            .collect();
        let mut found = false;
        for (index, entry) in std::fs::read_dir(&files_path).map_err(error)?.enumerate() {
            if index >= 4096 {
                return Err(stale());
            }
            let entry = entry.map_err(error)?;
            let name = entry.file_name();
            let meta = std::fs::symlink_metadata(entry.path()).map_err(error)?;
            if !matches!(&expected.identity, Evidence::Known(id) if id.device_id==meta.dev() && id.inode==meta.ino() && id.birth_ns==crate::models::birth_ns(&meta))
            {
                continue;
            }
            if meta.is_file() && meta.nlink() != 1 {
                return Err(stale());
            }
            let observed = trash.observe(StdPath::new(&name))?;
            if observed.kind == Evidence::Known(FileKind::Directory) {
                trash.require_empty(StdPath::new(&name))?;
            }
            if !same_retained_content(&observed, expected) {
                return Err(stale());
            }
            let mut info_name = name;
            info_name.push(".trashinfo");
            let mut bytes = Vec::new();
            info.read(StdPath::new(&info_name))?
                .take(65537)
                .read_to_end(&mut bytes)
                .map_err(error)?;
            if bytes.len() > 65536
                || !std::str::from_utf8(&bytes)
                    .ok()
                    .is_some_and(|s| s.lines().any(|l| l == format!("Path={encoded}")))
            {
                return Err(stale());
            }
            found = true;
        }
        if !found {
            return Err(stale());
        }
        Ok(())
    }
    pub(in crate::orchestrator::operations) fn remove_to_trash(
        &self,
        request: &ExecuteOperationRequest,
        journal: Option<&dyn super::super::journal::ExecutionJournal>,
    ) -> Result<LocalState, OperatorError> {
        self.remove_with(request, journal, prepare)
    }
    #[cfg(test)]
    pub(in crate::orchestrator::operations) fn remove_to_test_trash(
        &self,
        request: &ExecuteOperationRequest,
        journal: &dyn super::super::journal::ExecutionJournal,
        data: &StdPath,
    ) -> Result<LocalState, OperatorError> {
        self.remove_with(request, Some(journal), |source, original| {
            prepare_to(source, original, data)
        })
    }
    fn remove_with<F>(
        &self,
        request: &ExecuteOperationRequest,
        journal: Option<&dyn super::super::journal::ExecutionJournal>,
        prepare: F,
    ) -> Result<LocalState, OperatorError>
    where
        F: FnOnce(&Entry, &StdPath) -> Result<TrashDestination, OperatorError>,
    {
        let path = request.local.path.as_ref().ok_or_else(stale)?;
        let source = self.entry(path)?;
        self.check(&request.local)?;
        if request.local.kind == Evidence::Known(FileKind::Regular)
            && self.read(path)?.metadata().map_err(error)?.nlink() != 1
        {
            return Err(stale());
        }
        let destination = prepare(&source, &self.absolute.join(path))?;
        let journal = journal.ok_or_else(stale)?;
        journal.checkpoint(crate::inventory::models::RecoveryCheckpoint::LocalTrash(
            destination.reference.clone(),
        ))?;
        self.check(&request.local)?;
        if request.local.kind == Evidence::Known(FileKind::Directory) {
            self.require_empty(path)?;
        }
        destination.commit(&source)?;
        let mut absent = request.local.clone();
        absent.state = FileState::Absent;
        if self.trashed(&journal.record())? != Some(absent.clone()) {
            return Err(stale().uncertain());
        }
        Ok(absent)
    }
    pub(in crate::orchestrator::operations) fn require_empty(
        &self,
        path: &StdPath,
    ) -> Result<(), OperatorError> {
        let entry = self.entry(path)?;
        let dir = directory(&entry.anchored())?;
        if std::fs::read_dir(format!("/proc/self/fd/{}", dir.as_raw_fd()))
            .map_err(error)?
            .next()
            .is_some()
        {
            return Err(stale());
        }
        Ok(())
    }
    /// Receipt, destination parent, retained inode/content and original absence must all agree.
    pub(in crate::orchestrator::operations) fn trashed(
        &self,
        record: &RecoveryRecord,
    ) -> Result<Option<LocalState>, OperatorError> {
        let reference = record.local_trash.as_ref().ok_or_else(stale)?;
        let parent = reference.destination.parent().ok_or_else(stale)?;
        let name = reference.destination.file_name().ok_or_else(stale)?;
        let trash = Filesystem::open(parent)?;
        if trash.root_identity()? != reference.parent_identity {
            return Err(stale());
        }
        let mut info_name = name.to_os_string();
        info_name.push(".trashinfo");
        let info = Filesystem::open(&parent.parent().ok_or_else(stale)?.join("info"))?;
        let mut bytes = Vec::new();
        info.read(StdPath::new(&info_name))?
            .take(65537)
            .read_to_end(&mut bytes)
            .map_err(error)?;
        if bytes.len() > 65536
            || reference.info_checksum
                != (Checksum {
                    algorithm: "sha256".into(),
                    value: format!("{:x}", Sha256::digest(bytes)),
                })
        {
            return Err(stale());
        }
        let original = record.local.path.as_ref().ok_or_else(stale)?;
        if trash.check_absent(StdPath::new(name)).is_ok() {
            self.check(&record.local)?;
            return Ok(None);
        }
        let observed = trash.observe(StdPath::new(name))?;
        if !same_retained_content(&observed, &record.local) {
            return Err(stale());
        }
        if observed.kind == Evidence::Known(FileKind::Directory) {
            trash.require_empty(StdPath::new(name))?;
        }
        self.check_absent(original)?;
        let mut absent = record.local.clone();
        absent.state = FileState::Absent;
        Ok(Some(absent))
    }
}
pub(super) fn same_retained_content(current: &LocalState, expected: &LocalState) -> bool {
    current.identity == expected.identity
        && current.kind == expected.kind
        && current.size == expected.size
        && current.checksum == expected.checksum
        && current.modified_at == expected.modified_at
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Receipt(std::sync::Mutex<RecoveryRecord>);
    impl super::super::super::journal::ExecutionJournal for Receipt {
        fn record(&self) -> RecoveryRecord {
            self.0.lock().unwrap().clone()
        }
        fn checkpoint(
            &self,
            c: crate::inventory::models::RecoveryCheckpoint,
        ) -> Result<(), OperatorError> {
            if let crate::inventory::models::RecoveryCheckpoint::LocalTrash(r) = c {
                self.0.lock().unwrap().local_trash = Some(r);
                Ok(())
            } else {
                Err(stale())
            }
        }
    }
    #[test]
    fn checkpointed_trash_survives_lost_receipt_and_rejects_destination_replacement() {
        use crate::orchestrator::operations::{
            journal::ExecutionJournal,
            recovery,
            tests::{request, setup, Sandbox},
        };
        let s = Sandbox::new();
        let executor = setup(&s);
        std::fs::write(executor.config.local_root.join("file"), b"retained content").unwrap();
        let r = request(&executor, OperationAction::RemoveLocal);
        let record = recovery::record(&executor.config, &r, &executor.filesystem, None).unwrap();
        let journal = Receipt(std::sync::Mutex::new(record));
        let absent = executor
            .filesystem
            .remove_with(&r, Some(&journal), |source, original| {
                prepare_to(source, original, &s.0)
            })
            .unwrap();
        let reopened = Filesystem::open(&executor.config.local_root).unwrap();
        assert_eq!(reopened.trashed(&journal.record()).unwrap(), Some(absent));
        let path = journal.record().local_trash.unwrap().destination;
        assert_eq!(std::fs::read(&path).unwrap(), b"retained content");
        std::fs::rename(s.0.join("Trash/files"), s.0.join("Trash/retained-files")).unwrap();
        std::fs::create_dir(s.0.join("Trash/files")).unwrap();
        assert!(reopened.trashed(&journal.record()).is_err());
        assert_eq!(
            std::fs::read(
                s.0.join("Trash/retained-files")
                    .join(path.file_name().unwrap())
            )
            .unwrap(),
            b"retained content"
        );
    }
    #[test]
    fn nonempty_folder_and_cross_device_trash_preserve_sources() {
        use crate::orchestrator::operations::{
            recovery,
            tests::{request, setup, Sandbox},
        };
        let s = Sandbox::new();
        let executor = setup(&s);
        std::fs::create_dir(executor.config.local_root.join("file")).unwrap();
        std::fs::write(
            executor.config.local_root.join("file/edited-child"),
            b"divergent",
        )
        .unwrap();
        let r = request(&executor, OperationAction::RemoveLocal);
        let journal = Receipt(std::sync::Mutex::new(
            recovery::record(&executor.config, &r, &executor.filesystem, None).unwrap(),
        ));
        assert!(executor
            .filesystem
            .remove_with(&r, Some(&journal), |source, original| prepare_to(
                source, original, &s.0
            ))
            .is_err());
        assert_eq!(
            std::fs::read(executor.config.local_root.join("file/edited-child")).unwrap(),
            b"divergent"
        );
        let other = PathBuf::from(format!(
            "/dev/shm/drivesync-stage10-cross-device-{}",
            std::process::id()
        ));
        if std::fs::metadata("/dev/shm").unwrap().dev() != std::fs::metadata(&s.0).unwrap().dev() {
            std::fs::create_dir(&other).unwrap();
            assert!(prepare_to(
                &executor.filesystem.entry(StdPath::new("file")).unwrap(),
                &executor.config.local_root.join("file"),
                &other
            )
            .is_err());
            assert!(executor
                .config
                .local_root
                .join("file/edited-child")
                .exists());
            std::fs::remove_dir_all(other).unwrap();
        }
    }
    #[test]
    fn trash_preserves_bytes_and_original_location() {
        let base = std::env::temp_dir().join(format!("drivesync-trash-{}", std::process::id()));
        std::fs::create_dir(&base).unwrap();
        let root = Filesystem::open(&base).unwrap();
        std::fs::write(base.join("original"), b"recoverable").unwrap();
        prepare_to(
            &root.entry(StdPath::new("original")).unwrap(),
            &base.join("original"),
            &base,
        )
        .unwrap()
        .commit(&root.entry(StdPath::new("original")).unwrap())
        .unwrap();
        assert!(!base.join("original").exists());
        let file = std::fs::read_dir(base.join("Trash/files"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(std::fs::read(file).unwrap(), b"recoverable");
        let info = std::fs::read_dir(base.join("Trash/info"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert!(std::fs::read_to_string(info)
            .unwrap()
            .contains(&format!("Path={}", base.join("original").display())));
        std::fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn unavailable_trash_never_removes_source() {
        let base = std::env::temp_dir().join(format!("drivesync-no-trash-{}", std::process::id()));
        std::fs::create_dir(&base).unwrap();
        let root = Filesystem::open(&base).unwrap();
        std::fs::write(base.join("original"), b"preserved").unwrap();
        assert!(prepare_to(
            &root.entry(StdPath::new("original")).unwrap(),
            &base.join("original"),
            &base.join("missing")
        )
        .is_err());
        assert_eq!(std::fs::read(base.join("original")).unwrap(), b"preserved");
        std::fs::remove_dir_all(base).unwrap();
    }
}
