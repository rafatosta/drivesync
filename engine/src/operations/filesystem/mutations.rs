//! Descriptor-relative namespace changes and a private recoverable per-root trash.
//! No unlink, recursive removal or overwrite fallback is permitted.
use super::*;
#[derive(serde::Serialize, serde::Deserialize)]
pub(in crate::operations) struct TrashManifest {
    pub original: LocalFile,
    pub parent: PhysicalIdentity,
}
impl Workspace {
    pub fn resource(
        &self,
        path: &LocalRelativePath,
        expected: Option<&LocalFileId>,
        trash: bool,
    ) -> OperatorResult<File> {
        let (parent, n) = self.resource_parent(path, trash)?;
        let file = open_at(&parent, &n, libc::O_RDONLY)?;
        let m = file.metadata().map_err(io)?;
        if !m.is_dir() {
            regular(&file)?;
        }
        let actual = observe(&file, path.clone())?;
        if expected.is_some_and(|id| actual.local_id.as_ref() != Some(id)) {
            return Err(fail(OperatorError::PreconditionFailed));
        }
        Ok(file)
    }
    pub fn resource_parent(
        &self,
        path: &LocalRelativePath,
        trash: bool,
    ) -> OperatorResult<(File, OsString)> {
        if trash && path.as_bytes().starts_with(b".drivesync/trash/") {
            let parts: Vec<_> = path.as_bytes().split(|b| *b == b'/').collect();
            if parts.len() != 4 || parts[3] != b"item" {
                return Err(fail(OperatorError::WrongScope));
            }
            let id = ExecutionId::new(
                String::from_utf8(parts[2].to_vec())
                    .map_err(|_| fail(OperatorError::WrongScope))?,
            )
            .map_err(|_| fail(OperatorError::WrongScope))?;
            Ok((self.trash_slot(&id, false)?, OsString::from("item")))
        } else {
            self.parent(path)
        }
    }
    pub fn trash_slot(&self, id: &ExecutionId, create: bool) -> OperatorResult<File> {
        let trash = if create {
            private_directory(&self.private, OsStr::new("trash"))?
        } else {
            directory_at(&self.private, OsStr::new("trash"))?
        };
        if create {
            private_directory(&trash, OsStr::new(id.as_str()))
        } else {
            directory_at(&trash, OsStr::new(id.as_str()))
        }
    }
    pub fn trash_manifest(&self, path: &LocalRelativePath) -> OperatorResult<TrashManifest> {
        let (slot, _) = self.resource_parent(path, true)?;
        let file = open_at(&slot, OsStr::new("manifest.json"), libc::O_RDONLY)?;
        let size = regular(&file)?.len();
        if size > 65536 {
            return Err(fail(OperatorError::WrongScope));
        }
        let mut bytes = Vec::new();
        file.take(65537).read_to_end(&mut bytes).map_err(io)?;
        let manifest: TrashManifest =
            serde_json::from_slice(&bytes).map_err(|_| fail(OperatorError::PreconditionFailed))?;
        components(&manifest.original.relative_path)?;
        manifest
            .original
            .validate()
            .map_err(|_| fail(OperatorError::PreconditionFailed))?;
        Ok(manifest)
    }
    pub fn write_trash_manifest(
        &self,
        id: &ExecutionId,
        original: &LocalFile,
        parent: &File,
    ) -> OperatorResult<()> {
        let slot = self.trash_slot(id, true)?;
        let manifest = TrashManifest {
            original: original.clone(),
            parent: identity(parent)?,
        };
        let bytes =
            serde_json::to_vec(&manifest).map_err(|_| fail(OperatorError::InvalidRequest))?;
        let mut file = match open_at(
            &slot,
            OsStr::new("manifest.json"),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
        ) {
            Ok(file) => file,
            Err(e) if e.kind == OperatorError::DestinationOccupied => {
                let path = LocalRelativePath::new(
                    format!(".drivesync/trash/{}/item", id.as_str()).into_bytes(),
                )
                .map_err(|_| fail(OperatorError::InvalidRequest))?;
                let old = self.trash_manifest(&path)?;
                if old.original != *original || old.parent != manifest.parent {
                    return Err(fail(OperatorError::PreconditionFailed));
                }
                return Ok(());
            }
            Err(e) => return Err(e),
        };
        file.write_all(&bytes).map_err(io)?;
        file.sync_all().map_err(io)?;
        slot.sync_all().map_err(io)
    }
    /// Atomic move without replacement. Caller verifies the moved inode and the
    /// current parent paths afterwards; uncertain effects retain both resources.
    pub fn relocate(&self, from: &File, old: &OsStr, to: &File, new: &OsStr) -> OperatorResult<()> {
        self.validate()?;
        let old = name(old)?;
        let new = name(new)?;
        // SAFETY: owned directory descriptors and validated single components.
        let result = unsafe {
            libc::renameat2(
                from.as_raw_fd(),
                old.as_ptr(),
                to.as_raw_fd(),
                new.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if result < 0 {
            return Err(io(std::io::Error::last_os_error()));
        }
        let applied = |e| {
            let mut e = io(e);
            e.effect = ExecutionEffect::Applied;
            e
        };
        from.sync_all().map_err(applied)?;
        to.sync_all().map_err(applied)?;
        Ok(())
    }
}
impl Workspace {
    /// Atomically exchange new content and the previous target. The displaced
    /// bytes remain in private staging even if verification or persistence fails.
    pub fn exchange_stage(&self, id: &ExecutionId, parent: &File, n: &OsStr) -> OperatorResult<()> {
        self.validate()?;
        let staged = name(&Self::stage_name(id))?;
        let n = name(n)?;
        // SAFETY: pinned owned descriptors and single components; no unlink.
        let result = unsafe {
            libc::renameat2(
                self.stages.as_raw_fd(),
                staged.as_ptr(),
                parent.as_raw_fd(),
                n.as_ptr(),
                libc::RENAME_EXCHANGE,
            )
        };
        if result < 0 {
            return Err(io(std::io::Error::last_os_error()));
        }
        let applied = |error| {
            let mut e = io(error);
            e.effect = ExecutionEffect::Applied;
            e
        };
        parent.sync_all().map_err(applied)?;
        self.stages.sync_all().map_err(applied)
    }
}
