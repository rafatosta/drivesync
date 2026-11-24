//! Linux descriptor-anchored operations. No symlink traversal or destination replacement.
pub(super) mod conflicts;
mod staging;
mod trash;
use super::models::*;
use crate::{inventory::models::LocalState, models::*};
use sha2::{Digest, Sha256};
use std::{
    ffi::CString,
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        },
    },
    path::{Component, Path as StdPath, PathBuf},
};

pub(super) struct Filesystem {
    root: File,
    absolute: PathBuf,
}
pub(super) struct Entry {
    pub parent: File,
    pub name: CString,
}
impl Entry {
    pub fn anchored(&self) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.parent.as_raw_fd()))
            .join(std::ffi::OsStr::from_bytes(self.name.as_bytes()))
    }
}
pub(super) fn error(_: std::io::Error) -> OperatorError {
    OperatorError::new(OperatorErrorKind::Unavailable)
}
pub(super) fn invalid() -> OperatorError {
    OperatorError::new(OperatorErrorKind::InvalidRequest)
}
pub(super) fn stale() -> OperatorError {
    OperatorError::new(OperatorErrorKind::PreconditionFailed)
}
fn directory(path: &StdPath) -> Result<File, OperatorError> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(error)
}
impl Filesystem {
    pub fn open(root: &StdPath) -> Result<Self, OperatorError> {
        if !root.is_absolute() {
            return Err(invalid());
        }
        let mut dir = directory(StdPath::new("/"))?;
        for part in root.components().skip(1) {
            let Component::Normal(name) = part else {
                return Err(invalid());
            };
            dir =
                directory(&PathBuf::from(format!("/proc/self/fd/{}", dir.as_raw_fd())).join(name))?;
        }
        Ok(Self {
            root: dir,
            absolute: root.to_owned(),
        })
    }
    pub fn validate_root(&self) -> Result<(), OperatorError> {
        let current = Self::open(&self.absolute)?;
        let original = self.root.metadata().map_err(error)?;
        let current = current.root.metadata().map_err(error)?;
        if original.dev() != current.dev() || original.ino() != current.ino() {
            return Err(stale());
        }
        Ok(())
    }
    pub fn entry(&self, path: &StdPath) -> Result<Entry, OperatorError> {
        let parts: Vec<_> = path.components().collect();
        if parts.is_empty()
            || parts.iter().any(|c| !matches!(c, Component::Normal(_)))
            || parts.first().is_some_and(|c| c.as_os_str() == ".drivesync")
        {
            return Err(invalid());
        }
        let mut parent = self.root.try_clone().map_err(error)?;
        for part in &parts[..parts.len() - 1] {
            parent = directory(
                &PathBuf::from(format!("/proc/self/fd/{}", parent.as_raw_fd()))
                    .join(part.as_os_str()),
            )?;
        }
        Ok(Entry {
            parent,
            name: CString::new(parts.last().unwrap().as_os_str().as_bytes())
                .map_err(|_| invalid())?,
        })
    }
    pub fn temporary(&self) -> Result<File, OperatorError> {
        let fd = unsafe {
            libc::openat(
                self.root.as_raw_fd(),
                c".".as_ptr(),
                libc::O_TMPFILE | libc::O_RDWR | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            return Err(error(std::io::Error::last_os_error()));
        }
        Ok(unsafe { File::from_raw_fd(fd) })
    }
    pub fn read(&self, path: &StdPath) -> Result<File, OperatorError> {
        let entry = self.entry(path)?;
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(entry.anchored())
            .map_err(error)?;
        if !file.metadata().map_err(error)?.is_file() {
            return Err(invalid());
        }
        Ok(file)
    }
    pub fn observe(&self, path: &StdPath) -> Result<LocalState, OperatorError> {
        let entry = self.entry(path)?;
        let meta = std::fs::symlink_metadata(entry.anchored()).map_err(error)?;
        if !meta.is_file() && !meta.is_dir() {
            return Err(invalid());
        }
        let checksum = if meta.is_file() {
            let mut file = self.read(path)?;
            let before = file.metadata().map_err(error)?;
            let mut digest = Sha256::new();
            let mut buf = [0; 65536];
            loop {
                let n = file.read(&mut buf).map_err(error)?;
                if n == 0 {
                    break;
                }
                digest.update(&buf[..n]);
            }
            let after = file.metadata().map_err(error)?;
            if stamp(&before) != stamp(&after) || stamp(&meta) != stamp(&after) {
                return Err(stale());
            }
            Evidence::Known(Checksum {
                algorithm: "sha256".into(),
                value: format!("{:x}", digest.finalize()),
            })
        } else {
            Evidence::NotApplicable
        };
        Ok(LocalState {
            state: FileState::Present,
            path: Some(path.to_owned()),
            name: path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| Evidence::Known(n.to_owned()))
                .unwrap_or(Evidence::Unsupported),
            parent: Evidence::Known(path.parent().unwrap_or(StdPath::new("")).to_owned()),
            kind: Evidence::Known(if meta.is_file() {
                FileKind::Regular
            } else {
                FileKind::Directory
            }),
            identity: Evidence::Known(LocalIdentity {
                device_id: meta.dev(),
                inode: meta.ino(),
                mount_id: None,
                birth_ns: crate::models::birth_ns(&meta),
            }),
            size: if meta.is_file() {
                Evidence::Known(meta.len())
            } else {
                Evidence::NotApplicable
            },
            modified_at: Evidence::Known(DateTime {
                unix_millis: meta.mtime() * 1000 + meta.mtime_nsec() / 1_000_000,
            }),
            changed_at: Evidence::Known(DateTime {
                unix_millis: meta.ctime() * 1000 + meta.ctime_nsec() / 1_000_000,
            }),
            checksum,
            capabilities: Evidence::NotCollected,
        })
    }
    /// Read-only recovery may accept incidental timestamp changes when the pinned source bytes match.
    /// This never authorizes continuing a transfer or mutating from stale preconditions.
    pub(in crate::orchestrator::operations) fn reconcile_source(
        &self,
        expected: &LocalState,
    ) -> Result<LocalState, OperatorError> {
        let current = self.observe(expected.path.as_deref().ok_or_else(invalid)?)?;
        if current.identity != expected.identity
            || current.kind != Evidence::Known(FileKind::Regular)
            || current.size != expected.size
            || !matches!((&current.checksum,&expected.checksum),(Evidence::Known(a),Evidence::Known(b)) if a==b && a.algorithm=="sha256" && !a.value.is_empty())
        {
            return Err(stale());
        }
        Ok(current)
    }
    pub fn check_absent(&self, path: &StdPath) -> Result<(), OperatorError> {
        let entry = self.entry(path)?;
        match std::fs::symlink_metadata(entry.anchored()) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            _ => Err(stale()),
        }
    }
    pub fn check(&self, expected: &LocalState) -> Result<LocalState, OperatorError> {
        let path = expected.path.as_deref().ok_or_else(invalid)?;
        let current = self.observe(path)?;
        let identity_matches = match (&expected.identity, &current.identity) {
            (Evidence::Known(a), Evidence::Known(b)) => a == b,
            _ => false,
        };
        if expected.state != FileState::Present
            || !identity_matches
            || expected.kind != current.kind
            || expected.modified_at != current.modified_at
            || expected.changed_at != current.changed_at
            || expected.size != current.size
        {
            return Err(stale());
        }
        if let Evidence::Known(hash) = &expected.checksum {
            if hash.algorithm != "sha256" || expected.checksum != current.checksum {
                return Err(stale());
            }
        }
        Ok(current)
    }
    pub(in crate::orchestrator::operations) fn movement_target(
        op: &Operation,
    ) -> Result<PathBuf, OperatorError> {
        let path = op.local_path.as_ref().ok_or_else(invalid)?;
        if op.action == OperationAction::RenameLocal {
            let name = op.name.as_ref().ok_or_else(invalid)?;
            let mut parts = StdPath::new(name).components();
            if !matches!(parts.next(), Some(Component::Normal(_))) || parts.next().is_some() {
                return Err(invalid());
            }
            Ok(path.with_file_name(name))
        } else if op.action == OperationAction::MoveLocal {
            let parent = op.parent.as_ref().ok_or_else(invalid)?;
            if parent.starts_with(path) {
                return Err(stale());
            }
            Ok(parent.join(path.file_name().ok_or_else(invalid)?))
        } else {
            Err(invalid())
        }
    }
    /// Recovery identifies the same incarnation and tagged effect; names or bytes alone are insufficient.
    pub(in crate::orchestrator::operations) fn moved(
        &self,
        record: &crate::inventory::models::RecoveryRecord,
    ) -> Result<Option<LocalState>, OperatorError> {
        let target = Self::movement_target(&record.command)?;
        if let Ok(current) = self.observe(record.local.path.as_deref().ok_or_else(invalid)?) {
            if current.identity == record.local.identity
                && current.kind == record.local.kind
                && current.checksum == record.local.checksum
            {
                return Ok(None);
            }
            return Err(stale());
        }
        self.check_absent(record.local.path.as_deref().ok_or_else(invalid)?)?;
        let current = self.observe(&target)?;
        if current.identity != record.local.identity
            || current.kind != record.local.kind
            || current.checksum != record.local.checksum
            || !matches!(&current.identity, Evidence::Known(i) if i.birth_ns.is_some())
        {
            return Err(stale());
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(self.entry(&target)?.anchored())
            .map_err(error)?;
        // A read-only shortcut cannot be tagged again. Its recorded birth identity,
        // digest and destination prove that the original inode reached the target.
        if crate::projections::workspace(&record.remote) {
            let meta = file.metadata().map_err(error)?;
            if record.parent_identity.as_ref()
                == Some(&self.parent_identity(record.local.path.as_deref().ok_or_else(invalid)?)?)
                && record.destination_parent_identity.as_ref()
                    == Some(&self.parent_identity(&target)?)
                && meta.mode() & 0o777 == 0o444
                && meta.nlink() == 1
                && current.identity
                    == Evidence::Known(LocalIdentity {
                        device_id: meta.dev(),
                        inode: meta.ino(),
                        mount_id: None,
                        birth_ns: birth_ns(&meta),
                    })
            {
                return Ok(Some(current));
            }
            return Err(stale());
        }
        let mut marker = vec![0u8; record.execution_id.0.len()];
        let n = unsafe {
            libc::fgetxattr(
                file.as_raw_fd(),
                c"user.drivesync.execution".as_ptr(),
                marker.as_mut_ptr().cast(),
                marker.len(),
            )
        };
        if n != marker.len() as isize || marker != record.execution_id.0.as_bytes() {
            return Err(stale());
        }
        Ok(Some(current))
    }
    pub fn execute(
        &self,
        request: &ExecuteOperationRequest,
        journal: Option<&dyn super::journal::ExecutionJournal>,
    ) -> Result<AppliedEffect, OperatorError> {
        let op = &request.operation;
        let path = op.local_path.as_deref().ok_or_else(invalid)?;
        let local = match op.action {
            OperationAction::CreateLocalDirectory => {
                self.create_staged_directory(request, journal.ok_or_else(stale)?)?
            }
            OperationAction::RenameLocal | OperationAction::MoveLocal => {
                self.check(&request.local)?;
                let target = Self::movement_target(&request.operation)?;
                let source_parent = self.parent_identity(path)?;
                let destination_parent = self.parent_identity(&target)?;
                if let Some(journal) = journal {
                    let record = journal.record();
                    if record.parent_identity.as_ref() != Some(&source_parent)
                        || record.destination_parent_identity.as_ref() != Some(&destination_parent)
                    {
                        return Err(stale());
                    }
                }
                let file = OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                    .open(self.entry(path)?.anchored())
                    .map_err(error)?;
                let meta = file.metadata().map_err(error)?;
                if meta.is_file() && meta.nlink() != 1 {
                    return Err(stale());
                }
                if request.local.identity
                    != Evidence::Known(LocalIdentity {
                        device_id: meta.dev(),
                        inode: meta.ino(),
                        mount_id: None,
                        birth_ns: birth_ns(&meta),
                    })
                {
                    return Err(stale());
                }
                if birth_ns(&meta).is_none() {
                    return Err(stale());
                }
                let marker = request.execution_id.0.as_bytes();
                if crate::projections::workspace(&request.remote) {
                    if meta.mode() & 0o777 != 0o444 {
                        return Err(stale());
                    }
                } else if unsafe {
                    libc::fsetxattr(
                        file.as_raw_fd(),
                        c"user.drivesync.execution".as_ptr(),
                        marker.as_ptr().cast(),
                        marker.len(),
                        0,
                    )
                } != 0
                {
                    return Err(error(std::io::Error::last_os_error()));
                }
                file.sync_all().map_err(error)?;
                let source = self.entry(path)?;
                let dest = self.entry(&target)?;
                if source_parent != self.parent_identity(path)?
                    || destination_parent != self.parent_identity(&target)?
                    || stamp(&std::fs::symlink_metadata(source.anchored()).map_err(error)?)
                        != stamp(&file.metadata().map_err(error)?)
                {
                    return Err(stale());
                }
                rename(&source, &dest, libc::RENAME_NOREPLACE)?;
                source.parent.sync_all().map_err(|e| error(e).uncertain())?;
                dest.parent.sync_all().map_err(|e| error(e).uncertain())?;
                if destination_parent != self.parent_identity(&target)? {
                    return Err(stale().uncertain());
                }
                let observed = self.observe(&target).map_err(OperatorError::uncertain)?;
                if observed.identity != request.local.identity
                    || observed.kind != request.local.kind
                    || observed.checksum != request.local.checksum
                    || observed.size != request.local.size
                {
                    return Err(stale().uncertain());
                }
                observed
            }
            OperationAction::RemoveLocal => self.remove_to_trash(request, journal)?,
            _ => return Err(OperatorError::new(OperatorErrorKind::UnsupportedOperation)),
        };
        Ok(AppliedEffect {
            local: Some(local),
            remote: None,
            remote_id: op.remote_id.clone(),
            verified: true,
        })
    }
    /// Publishes a verified replacement atomically and retains the displaced version.
    pub fn install(
        &self,
        request: &ExecuteOperationRequest,
        bytes: &mut File,
        journal: Option<&dyn super::journal::ExecutionJournal>,
    ) -> Result<LocalState, OperatorError> {
        self.install_with_trash(request, bytes, trash::prepare_replacement, journal)
    }
    #[cfg(test)]
    pub(in crate::orchestrator::operations) fn install_with_test_trash(
        &self,
        request: &ExecuteOperationRequest,
        bytes: &mut File,
        data: &StdPath,
        journal: &dyn super::journal::ExecutionJournal,
    ) -> Result<LocalState, OperatorError> {
        self.install_with_trash(
            request,
            bytes,
            |source, original| trash::prepare_replacement_to(source, original, data),
            Some(journal),
        )
    }
    fn install_with_trash(
        &self,
        request: &ExecuteOperationRequest,
        bytes: &mut File,
        prepare: impl FnOnce(&Entry, &StdPath) -> Result<Option<trash::TrashDestination>, OperatorError>,
        journal: Option<&dyn super::journal::ExecutionJournal>,
    ) -> Result<LocalState, OperatorError> {
        let path = request
            .operation
            .local_path
            .as_deref()
            .ok_or_else(invalid)?;
        let dest = self.entry(path)?;
        let stage_path = path.with_file_name(format!(
            ".drivesync-transfer-{}",
            execution_key(&request.execution_id)
        ));
        let stage = self.entry(&stage_path)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(stage.anchored())
            .map_err(error)?;
        bytes.rewind().map_err(error)?;
        std::io::copy(bytes, &mut output).map_err(|e| error(e).uncertain())?;
        output
            .flush()
            .and_then(|_| output.sync_all())
            .map_err(|e| error(e).uncertain())?;
        if matches!(
            request.operation.action,
            OperationAction::Download
                | OperationAction::UpdateLocalContent
                | OperationAction::CreateLocalShortcut
        ) && journal.is_some()
        {
            let journal = journal.ok_or_else(stale)?;
            self.mark_stage(&output, request, &stage_path, journal)?;
        }
        if request.operation.action == OperationAction::CreateLocalShortcut {
            output
                .set_permissions(std::fs::Permissions::from_mode(0o444))
                .and_then(|_| output.sync_all())
                .map_err(|e| error(e).uncertain())?;
        }
        if request.operation.action == OperationAction::UpdateLocalContent {
            self.check(&request.local)?;
            let trash = prepare(&stage, &self.absolute.join(path))?;
            self.check(&request.local)?;
            rename(&stage, &dest, libc::RENAME_EXCHANGE)?;
            if let Some(trash) = trash {
                trash.commit(&stage).map_err(OperatorError::uncertain)?;
            } else {
                // The reserved staging path retains the previous inode when trash is on
                // another mount. Its location is recorded in the execution journal.
                let retained = self
                    .observe(&stage_path)
                    .map_err(OperatorError::uncertain)?;
                if !trash::same_retained_content(&retained, &request.local) {
                    return Err(stale().uncertain());
                }
            }
        } else {
            rename(&stage, &dest, libc::RENAME_NOREPLACE)?;
        }
        dest.parent
            .sync_all()
            .map_err(|error_| error(error_).uncertain())?;
        self.observe(path).map_err(OperatorError::uncertain)
    }
}
use std::io::Seek;
pub(super) fn stamp(meta: &std::fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        meta.dev(),
        meta.ino(),
        meta.len(),
        meta.mtime(),
        meta.mtime_nsec(),
        meta.ctime(),
        meta.ctime_nsec(),
    )
}
pub(super) fn execution_key(id: &Id) -> String {
    format!("{:x}", Sha256::digest(id.0.as_bytes()))
}
fn rename(source: &Entry, dest: &Entry, flags: u32) -> Result<(), OperatorError> {
    if unsafe {
        libc::renameat2(
            source.parent.as_raw_fd(),
            source.name.as_ptr(),
            dest.parent.as_raw_fd(),
            dest.name.as_ptr(),
            flags,
        )
    } != 0
    {
        return Err(error(std::io::Error::last_os_error()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::tests::{request, setup, Sandbox};
    use super::*;
    #[test]
    fn replacement_preserves_old_bytes_in_isolated_trash() {
        let sandbox = Sandbox::new();
        let executor = setup(&sandbox);
        std::fs::write(executor.config.local_root.join("file"), b"old version").unwrap();
        let request = request(&executor, OperationAction::UpdateLocalContent);
        let mut downloaded = executor.filesystem.temporary().unwrap();
        downloaded.write_all(b"new version").unwrap();
        executor
            .filesystem
            .install_with_trash(
                &request,
                &mut downloaded,
                |source, original| trash::prepare_replacement_to(source, original, &sandbox.0),
                None,
            )
            .unwrap();
        assert_eq!(
            std::fs::read(executor.config.local_root.join("file")).unwrap(),
            b"new version"
        );
        let old = std::fs::read_dir(sandbox.0.join("Trash/files"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(std::fs::read(old).unwrap(), b"old version");
    }
    #[test]
    #[ignore = "requires a separate writable trash mount, supplied by the Flatpak verification environment"]
    fn replacement_across_trash_mount_preserves_both_versions() {
        let sandbox = Sandbox::new();
        let executor = setup(&sandbox);
        let file = executor.config.local_root.join("file");
        std::fs::write(&file, b"old version").unwrap();
        let request = request(&executor, OperationAction::UpdateLocalContent);
        let mut downloaded = executor.filesystem.temporary().unwrap();
        downloaded.write_all(b"new version").unwrap();
        let data = PathBuf::from(std::env::var_os("DRIVESYNC_TEST_TRASH_DATA").unwrap());
        executor
            .filesystem
            .install_with_trash(
                &request,
                &mut downloaded,
                |source, original| trash::prepare_replacement_to(source, original, &data),
                None,
            )
            .unwrap();
        assert_eq!(std::fs::read(file).unwrap(), b"new version");
        let retained = executor
            .config
            .local_root
            .join(Filesystem::stage_path(&request).unwrap());
        assert_eq!(std::fs::read(retained).unwrap(), b"old version");
    }
    #[test]
    fn unavailable_trash_refuses_replacement_before_exchange() {
        let sandbox = Sandbox::new();
        let executor = setup(&sandbox);
        std::fs::write(executor.config.local_root.join("file"), b"old version").unwrap();
        let request = request(&executor, OperationAction::UpdateLocalContent);
        let mut downloaded = executor.filesystem.temporary().unwrap();
        downloaded.write_all(b"new version").unwrap();
        assert!(executor
            .filesystem
            .install_with_trash(
                &request,
                &mut downloaded,
                |source, original| {
                    trash::prepare_replacement_to(source, original, &sandbox.0.join("unavailable"))
                },
                None
            )
            .is_err());
        assert_eq!(
            std::fs::read(executor.config.local_root.join("file")).unwrap(),
            b"old version"
        );
    }
}
