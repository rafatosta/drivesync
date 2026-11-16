//! Descriptor-relative staging, exclusive publication and recoverable mutations.
//! Replacements preserve displaced bytes; no permanent deletion is implemented.
mod mutations;
use super::*;
use crate::{filesystem::metadata, models::*};
use sha2::{Digest, Sha256};
use std::{
    ffi::{CString, OsStr, OsString},
    fs::{File, Metadata},
    io::{Read, Seek, SeekFrom, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::{OsStrExt, OsStringExt},
            fs::MetadataExt,
        },
    },
    path::{Component, Path, PathBuf},
};

pub(super) fn fail(kind: OperatorError) -> OperatorFailure {
    OperatorFailure {
        kind,
        effect: ExecutionEffect::NotApplied,
        resource: None,
    }
}
pub(super) fn io(error: std::io::Error) -> OperatorFailure {
    fail(match error.raw_os_error() {
        Some(libc::ENOSPC | libc::EDQUOT) => OperatorError::InsufficientSpace,
        Some(libc::EACCES | libc::EPERM) => OperatorError::PermissionDenied,
        Some(libc::EEXIST) => OperatorError::DestinationOccupied,
        Some(libc::ENOENT) => OperatorError::NotFoundOrInaccessible,
        Some(libc::ELOOP | libc::ENOTDIR) => OperatorError::WrongScope,
        Some(libc::EWOULDBLOCK) => OperatorError::CheckpointConflict,
        Some(libc::EXDEV | libc::ENOSYS | libc::EOPNOTSUPP) => OperatorError::UnsupportedOperation,
        _ => OperatorError::Io,
    })
}
fn name(raw: &OsStr) -> OperatorResult<CString> {
    CString::new(raw.as_bytes()).map_err(|_| fail(OperatorError::InvalidRequest))
}
fn open_at(parent: &File, raw: &OsStr, flags: i32) -> OperatorResult<File> {
    let n = name(raw)?;
    // SAFETY: held directory descriptor and NUL-terminated component; ownership
    // of a successful openat result is transferred exactly once to File.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            n.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            0o600,
        )
    };
    if fd < 0 {
        return Err(io(std::io::Error::last_os_error()));
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn directory_at(parent: &File, raw: &OsStr) -> OperatorResult<File> {
    open_at(parent, raw, libc::O_RDONLY | libc::O_DIRECTORY)
}
fn directory(path: &Path) -> OperatorResult<File> {
    if !path.is_absolute() {
        return Err(fail(OperatorError::InvalidConfiguration));
    }
    let mut current = File::open("/").map_err(io)?;
    for c in path.components() {
        match c {
            Component::RootDir => {}
            Component::Normal(n) => current = directory_at(&current, n)?,
            _ => return Err(fail(OperatorError::WrongScope)),
        }
    }
    Ok(current)
}
fn private_directory(parent: &File, raw: &OsStr) -> OperatorResult<File> {
    let n = name(raw)?;
    // SAFETY: valid directory descriptor and single component. Existing entries
    // are never changed; open below refuses symlinks and checks private ownership.
    let result = unsafe { libc::mkdirat(parent.as_raw_fd(), n.as_ptr(), 0o700) };
    if result < 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST) {
        return Err(io(std::io::Error::last_os_error()));
    }
    let file = directory_at(parent, raw)?;
    let m = file.metadata().map_err(io)?;
    if m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o077 != 0 {
        return Err(fail(OperatorError::PermissionDenied));
    }
    parent.sync_all().map_err(io)?;
    Ok(file)
}
fn regular(file: &File) -> OperatorResult<Metadata> {
    let m = file.metadata().map_err(io)?;
    if !m.is_file() || m.nlink() != 1 {
        return Err(fail(OperatorError::PreconditionFailed));
    }
    Ok(m)
}
pub(super) fn identity(file: &File) -> OperatorResult<PhysicalIdentity> {
    let m = file.metadata().map_err(io)?;
    let birth = metadata::statx(file).ok().and_then(|s| {
        (s.stx_mask & libc::STATX_BTIME != 0).then_some(LocalTimestamp {
            unix_seconds: s.stx_btime.tv_sec,
            nanoseconds: s.stx_btime.tv_nsec,
        })
    });
    Ok(PhysicalIdentity {
        device: m.dev(),
        inode: m.ino(),
        birth,
    })
}
pub(super) fn observe(file: &File, path: LocalRelativePath) -> OperatorResult<LocalFile> {
    let m = file.metadata().map_err(io)?;
    if !m.is_dir() {
        regular(file)?;
    }
    match metadata::statx(file) {
        Ok(s) => metadata::from_statx(&s, &m, path),
        Err(e) => metadata::from_fstat(&m, path, &e),
    }
    .map_err(|_| fail(OperatorError::Io))
}
fn same_stamp(a: &Metadata, b: &Metadata) -> bool {
    (
        a.dev(),
        a.ino(),
        a.len(),
        a.mtime(),
        a.mtime_nsec(),
        a.ctime(),
        a.ctime_nsec(),
        a.nlink(),
    ) == (
        b.dev(),
        b.ino(),
        b.len(),
        b.mtime(),
        b.mtime_nsec(),
        b.ctime(),
        b.ctime_nsec(),
        b.nlink(),
    )
}
pub(super) fn matches_observation(file: &File, expected: &LocalFile) -> OperatorResult<()> {
    let actual = observe(file, expected.relative_path.clone())?;
    let same = actual.storage == expected.storage
        && actual.times.modified_at == expected.times.modified_at
        && actual.times.changed_at == expected.times.changed_at
        && expected
            .local_id
            .as_ref()
            .is_none_or(|id| actual.local_id.as_ref() == Some(id));
    if !same {
        return Err(fail(OperatorError::PreconditionFailed));
    }
    Ok(())
}
pub(super) fn check(options: &ExecutionOptions) -> OperatorResult<()> {
    if options
        .cancellation
        .as_ref()
        .is_some_and(|c| c.is_cancelled())
    {
        return Err(fail(OperatorError::Cancelled));
    }
    Ok(())
}
pub(super) fn report(options: &ExecutionOptions, phase: ExecutionPhase, bytes: u64, total: u64) {
    if let Some(reporter) = &options.progress {
        let _ = reporter.try_report(ExecutionProgress {
            phase,
            bytes_transferred: Some(bytes),
            total_bytes: Some(total),
        });
    }
}

/// Held across the entire worker, including HTTP awaits driven by its runtime.
/// flock is process-wide exclusion across separate handles/instances, not a lease
/// inferred from CAS. A cancelled caller cannot release a still-running worker.
pub(super) struct Workspace {
    pub root: File,
    private: File,
    stages: File,
    _lock: File,
    path: PathBuf,
    private_identity: PhysicalIdentity,
    stages_identity: PhysicalIdentity,
}
impl Workspace {
    pub fn open(path: &Path) -> OperatorResult<Self> {
        let root = directory(path)?;
        let private = private_directory(&root, OsStr::new(".drivesync"))?;
        let lock = open_at(
            &private,
            OsStr::new("operator.lock"),
            libc::O_RDWR | libc::O_CREAT,
        )?;
        let m = regular(&lock)?;
        if m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o077 != 0 {
            return Err(fail(OperatorError::PermissionDenied));
        }
        // SAFETY: valid owned descriptor; never blocks another execution/GTK.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } < 0 {
            return Err(io(std::io::Error::last_os_error()));
        }
        let stages = private_directory(&private, OsStr::new("transfers"))?;
        let value = Self {
            private_identity: identity(&private)?,
            stages_identity: identity(&stages)?,
            root,
            private,
            stages,
            _lock: lock,
            path: path.into(),
        };
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> OperatorResult<()> {
        let root = directory(&self.path)?;
        if identity(&root)? != identity(&self.root)?
            || identity(&directory_at(&root, OsStr::new(".drivesync"))?)? != self.private_identity
            || identity(&directory_at(&self.private, OsStr::new("transfers"))?)?
                != self.stages_identity
            || identity(&open_at(
                &self.private,
                OsStr::new("operator.lock"),
                libc::O_RDONLY,
            )?)? != identity(&self._lock)?
        {
            return Err(fail(OperatorError::WrongScope));
        }
        Ok(())
    }
    pub fn directory(&self, target: &DirectoryRef) -> OperatorResult<File> {
        match target {
            DirectoryRef::LocalRoot => self.root.try_clone().map_err(io),
            DirectoryRef::Local {
                relative_path,
                local_id,
            } => {
                let mut current = self.root.try_clone().map_err(io)?;
                for n in components(relative_path)? {
                    current = directory_at(&current, &n)?;
                }
                if let Some(id) = local_id {
                    let m = current.metadata().map_err(io)?;
                    let s = metadata::statx(&current).map_err(io)?;
                    let observed = metadata::from_statx(&s, &m, relative_path.clone())
                        .map_err(|_| fail(OperatorError::Io))?;
                    if observed.local_id.as_ref() != Some(id) {
                        return Err(fail(OperatorError::PreconditionFailed));
                    }
                }
                Ok(current)
            }
            _ => Err(fail(OperatorError::InvalidRequest)),
        }
    }
    pub fn parent(&self, path: &LocalRelativePath) -> OperatorResult<(File, OsString)> {
        let mut parts = components(path)?;
        let last = parts
            .pop()
            .ok_or_else(|| fail(OperatorError::InvalidRequest))?;
        let mut parent = self.root.try_clone().map_err(io)?;
        for n in parts {
            parent = directory_at(&parent, &n)?;
        }
        Ok((parent, last))
    }
    pub fn source(&self, expected: &LocalFile) -> OperatorResult<File> {
        let (parent, n) = self.parent(&expected.relative_path)?;
        let file = open_at(&parent, &n, libc::O_RDONLY)?;
        matches_observation(&file, expected)?;
        Ok(file)
    }
    pub fn scope(&self, parent: &File) -> OperatorResult<LocalExecutionScope> {
        Ok(LocalExecutionScope {
            root: identity(&self.root)?,
            parent: identity(parent)?,
            staging_directory: self.stages_identity.clone(),
        })
    }
    pub fn stage_path(id: &ExecutionId) -> LocalRelativePath {
        LocalRelativePath::new(format!(".drivesync/transfers/{}.part", id.as_str()).into_bytes())
            .expect("validated identity yields a relative staging path")
    }
    fn stage_name(id: &ExecutionId) -> OsString {
        format!("{}.part", id.as_str()).into()
    }
    pub fn stage(&self, id: &ExecutionId) -> OperatorResult<File> {
        self.validate()?;
        open_at(
            &self.stages,
            &Self::stage_name(id),
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL,
        )
    }
    pub fn existing_stage(&self, id: &ExecutionId) -> OperatorResult<File> {
        self.validate()?;
        let file = open_at(&self.stages, &Self::stage_name(id), libc::O_RDWR)?;
        regular(&file)?;
        Ok(file)
    }
    pub fn snapshot(
        &self,
        id: &ExecutionId,
        source: &LocalFile,
        expected_hash: Option<&Sha256Digest>,
        options: &ExecutionOptions,
    ) -> OperatorResult<StagingEvidence> {
        let mut input = self.source(source)?;
        let before = regular(&input)?;
        // A partial private stage is rewritten only before a checkpoint has
        // committed its identity/hash. No user entry is ever truncated.
        let mut output = match self.stage(id) {
            Ok(f) => f,
            Err(e) if e.kind == OperatorError::DestinationOccupied => self.existing_stage(id)?,
            Err(e) => return Err(e),
        };
        output.set_len(0).map_err(io)?;
        output.seek(SeekFrom::Start(0)).map_err(io)?;
        let mut hash = Sha256::new();
        let mut copied = 0;
        let mut buffer = [0; 65536];
        loop {
            check(options)?;
            self.validate()?;
            let limit = before
                .len()
                .saturating_sub(copied)
                .saturating_add(1)
                .min(buffer.len() as u64) as usize;
            let n = input.read(&mut buffer[..limit]).map_err(io)?;
            if n == 0 {
                break;
            }
            copied += n as u64;
            if copied > before.len() {
                return Err(fail(OperatorError::PreconditionFailed));
            }
            output.write_all(&buffer[..n]).map_err(io)?;
            hash.update(&buffer[..n]);
        }
        if copied != before.len() || !same_stamp(&before, &regular(&input)?) {
            return Err(fail(OperatorError::PreconditionFailed));
        }
        self.source(source)?;
        output.sync_all().map_err(io)?;
        self.stages.sync_all().map_err(io)?;
        let sha256 = Sha256Digest::new(format!("{:x}", hash.finalize()))
            .map_err(|_| fail(OperatorError::Internal))?;
        if expected_hash.is_some_and(|h| h != &sha256) {
            return Err(fail(OperatorError::IntegrityMismatch));
        }
        let evidence = StagingEvidence {
            metadata: Box::new(observe(&output, Self::stage_path(id))?),
            sha256,
        };
        self.verify_stage(id, &evidence, options)?;
        Ok(evidence)
    }
    pub fn verify_stage(
        &self,
        id: &ExecutionId,
        evidence: &StagingEvidence,
        options: &ExecutionOptions,
    ) -> OperatorResult<File> {
        if evidence.metadata.relative_path != Self::stage_path(id) {
            return Err(fail(OperatorError::WrongScope));
        }
        let mut file = self.existing_stage(id)?;
        matches_observation(&file, &evidence.metadata)?;
        if hash(&mut file, options)? != evidence.sha256 {
            return Err(fail(OperatorError::IntegrityMismatch));
        }
        self.validate()?;
        matches_observation(&self.existing_stage(id)?, &evidence.metadata)?;
        file.seek(SeekFrom::Start(0)).map_err(io)?;
        Ok(file)
    }
    pub fn free(parent: &File, n: &OsStr) -> OperatorResult<()> {
        let name = name(n)?;
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: valid directory, name and output; no link traversal.
        if unsafe {
            libc::fstatat(
                parent.as_raw_fd(),
                name.as_ptr(),
                stat.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } == 0
        {
            return Err(fail(OperatorError::DestinationOccupied));
        }
        let e = std::io::Error::last_os_error();
        if e.raw_os_error() != Some(libc::ENOENT) {
            return Err(io(e));
        }
        Ok(())
    }
    pub fn sync_stages(&self) -> OperatorResult<()> {
        self.stages.sync_all().map_err(io)
    }
    pub fn publish(&self, id: &ExecutionId, parent: &File, n: &OsStr) -> OperatorResult<()> {
        self.publish_named(&Self::stage_name(id), parent, n)
    }
    pub fn directory_stage(&self, id: &ExecutionId, create: bool) -> OperatorResult<File> {
        let n = OsString::from(format!("{}.directory", id.as_str()));
        if create {
            private_directory(&self.stages, &n)
        } else {
            directory_at(&self.stages, &n)
        }
    }
    pub fn publish_directory(
        &self,
        id: &ExecutionId,
        parent: &File,
        n: &OsStr,
    ) -> OperatorResult<()> {
        self.publish_named(
            &OsString::from(format!("{}.directory", id.as_str())),
            parent,
            n,
        )
    }
    fn publish_named(&self, old: &OsStr, parent: &File, n: &OsStr) -> OperatorResult<()> {
        self.validate()?;
        let old = name(old)?;
        let new = name(n)?;
        // SAFETY: held directory descriptors and names; NOREPLACE never removes
        // a destination inserted after the earlier collision check.
        let result = unsafe {
            libc::renameat2(
                self.stages.as_raw_fd(),
                old.as_ptr(),
                parent.as_raw_fd(),
                new.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if result < 0 {
            return Err(io(std::io::Error::last_os_error()));
        }
        let applied = |error| {
            let mut failure = io(error);
            failure.effect = ExecutionEffect::Applied;
            failure
        };
        parent.sync_all().map_err(applied)?;
        self.stages.sync_all().map_err(applied)?;
        Ok(())
    }
    pub fn published(parent: &File, n: &OsStr) -> OperatorResult<File> {
        let f = open_at(parent, n, libc::O_RDONLY)?;
        regular(&f)?;
        Ok(f)
    }
}
fn components(path: &LocalRelativePath) -> OperatorResult<Vec<OsString>> {
    let parts: Vec<_> = path
        .as_bytes()
        .split(|b| *b == b'/')
        .map(|n| OsString::from_vec(n.to_vec()))
        .collect();
    if parts.iter().any(|n| n == ".drivesync") {
        return Err(fail(OperatorError::WrongScope));
    }
    Ok(parts)
}
pub(super) fn local_name(n: &ItemName) -> OperatorResult<OsString> {
    let ItemName::Local(n) = n else {
        return Err(fail(OperatorError::InvalidRequest));
    };
    let b = n.as_bytes();
    if b.is_empty()
        || b.len() > 255
        || b.contains(&0)
        || b.contains(&b'/')
        || matches!(b, b"." | b".." | b".drivesync")
    {
        return Err(fail(OperatorError::InvalidRequest));
    }
    Ok(n.clone())
}
pub(super) fn hash(file: &mut File, options: &ExecutionOptions) -> OperatorResult<Sha256Digest> {
    let before = regular(file)?;
    file.seek(SeekFrom::Start(0)).map_err(io)?;
    let mut hash = Sha256::new();
    let mut copied = 0;
    let mut bytes = [0; 65536];
    loop {
        check(options)?;
        let n = file.read(&mut bytes).map_err(io)?;
        if n == 0 {
            break;
        }
        copied += n as u64;
        if copied > before.len() {
            return Err(fail(OperatorError::IntegrityMismatch));
        }
        hash.update(&bytes[..n]);
    }
    if copied != before.len() || !same_stamp(&before, &regular(file)?) {
        return Err(fail(OperatorError::IntegrityMismatch));
    }
    Sha256Digest::new(format!("{:x}", hash.finalize())).map_err(|_| fail(OperatorError::Internal))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tree(label: &str) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "operator-fs-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        path
    }
    #[test]
    fn descriptor_staging_rejects_symlinks_and_publishes_without_clobbering() {
        let path = tree("publish");
        std::fs::write(path.join("source"), b"abc").unwrap();
        let root = Workspace::open(&path).unwrap();
        let relative = LocalRelativePath::new(b"source".to_vec()).unwrap();
        let source = observe(&File::open(path.join("source")).unwrap(), relative).unwrap();
        let id = ExecutionId::new("execution".into()).unwrap();
        let staged = root
            .snapshot(&id, &source, None, &ExecutionOptions::default())
            .unwrap();
        std::fs::write(path.join("target"), b"keep").unwrap();
        assert_eq!(
            root.publish(&id, &root.root, OsStr::new("target"))
                .unwrap_err()
                .kind,
            OperatorError::DestinationOccupied
        );
        assert_eq!(std::fs::read(path.join("target")).unwrap(), b"keep");
        root.verify_stage(&id, &staged, &ExecutionOptions::default())
            .unwrap();
        root.publish(&id, &root.root, OsStr::new("new")).unwrap();
        assert_eq!(std::fs::read(path.join("new")).unwrap(), b"abc");
        std::os::unix::fs::symlink("/etc", path.join("escape")).unwrap();
        assert!(root
            .directory(&DirectoryRef::Local {
                relative_path: LocalRelativePath::new(b"escape".to_vec()).unwrap(),
                local_id: None
            })
            .is_err());
        drop(root);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn lock_survives_separate_handles_and_root_replacement_is_detected() {
        let path = tree("lock");
        let root = Workspace::open(&path).unwrap();
        assert!(
            matches!(Workspace::open(&path), Err(e) if e.kind == OperatorError::CheckpointConflict)
        );
        let moved = path.with_extension("moved");
        std::fs::rename(&path, &moved).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert_eq!(root.validate().unwrap_err().kind, OperatorError::WrongScope);
        drop(root);
        std::fs::remove_dir_all(path).unwrap();
        std::fs::remove_dir_all(moved).unwrap();
    }
    #[test]
    fn modified_snapshot_and_replaced_sources_are_not_adopted() {
        let path = tree("snapshot");
        std::fs::write(path.join("source"), b"abc").unwrap();
        let root = Workspace::open(&path).unwrap();
        let source = observe(
            &File::open(path.join("source")).unwrap(),
            LocalRelativePath::new(b"source".to_vec()).unwrap(),
        )
        .unwrap();
        let id = ExecutionId::new("stable".into()).unwrap();
        let stage = root
            .snapshot(&id, &source, None, &ExecutionOptions::default())
            .unwrap();
        std::fs::write(path.join(".drivesync/transfers/stable.part"), b"bad").unwrap();
        assert!(root
            .verify_stage(&id, &stage, &ExecutionOptions::default())
            .is_err());
        std::fs::rename(path.join("source"), path.join("old")).unwrap();
        std::fs::write(path.join("source"), b"abc").unwrap();
        assert!(root.source(&source).is_err());
        drop(root);
        std::fs::remove_dir_all(path).unwrap();
    }
}
