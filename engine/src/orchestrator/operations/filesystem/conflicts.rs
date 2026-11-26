//! Private, no-replace archives keep both divergent byte streams recoverable before any replacement.
use super::*;
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
pub(in crate::orchestrator::operations) struct ArchiveManifest {
    pub id: String,
    pub local: crate::inventory::models::LocalState,
    pub remote: crate::inventory::models::RemoteState,
    pub local_hash: String,
    pub local_size: u64,
    pub remote_hash: String,
    pub remote_size: u64,
}
#[derive(Serialize, Deserialize)]
struct DirectoryArchive {
    id: String,
    local: crate::inventory::models::LocalState,
    remote: crate::inventory::models::RemoteState,
}
impl Filesystem {
    fn conflict_directory(&self, id: &Id) -> Result<File, OperatorError> {
        if id.0.is_empty()
            || id.0.len() > 128
            || !id.0.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(invalid());
        }
        let mut parent = self.root.try_clone().map_err(error)?;
        for name in [".drivesync", "conflicts", &id.0] {
            let c = CString::new(name).map_err(|_| invalid())?;
            if unsafe { libc::mkdirat(parent.as_raw_fd(), c.as_ptr(), 0o700) } != 0
                && std::io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST)
            {
                return Err(error(std::io::Error::last_os_error()));
            }
            parent.sync_all().map_err(error)?;
            parent = directory(
                &PathBuf::from(format!("/proc/self/fd/{}", parent.as_raw_fd())).join(name),
            )?;
            let meta = parent.metadata().map_err(error)?;
            if meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
                return Err(stale());
            }
        }
        Ok(parent)
    }
    fn archive_file(parent: &File, name: &str) -> Result<File, OperatorError> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(PathBuf::from(format!("/proc/self/fd/{}", parent.as_raw_fd())).join(name))
            .map_err(error)?;
        let m = file.metadata().map_err(error)?;
        if !m.is_file()
            || m.nlink() != 1
            || m.uid() != unsafe { libc::geteuid() }
            || m.mode() & 0o077 != 0
        {
            return Err(stale());
        }
        Ok(file)
    }
    fn verify_archive(
        parent: &File,
        name: &str,
        size: u64,
        hash: &str,
    ) -> Result<(), OperatorError> {
        let mut progress = HashProgress::new(Phase::VerifyArchive, Some(size), Algorithm::Sha256);
        let result = (|| {
            let mut file = Self::archive_file(parent, name)?;
            if file.metadata().map_err(error)?.len() != size {
                return Err(stale());
            }
            let mut digest = Sha256::new();
            let mut buffer = [0; 65536];
            loop {
                let n = file.read(&mut buffer).map_err(error)?;
                if n == 0 {
                    break;
                }
                digest.update(&buffer[..n]);
                progress.processed(n);
            }
            if format!("{:x}", digest.finalize()) != hash {
                return Err(stale());
            }
            Ok(())
        })();
        progress.result(&result);
        result
    }
    pub(in crate::orchestrator::operations) fn conflict_archived(
        &self,
        id: &Id,
    ) -> Result<bool, OperatorError> {
        let parent = self.conflict_directory(id)?;
        let path =
            PathBuf::from(format!("/proc/self/fd/{}", parent.as_raw_fd())).join("manifest.json");
        match std::fs::symlink_metadata(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return self.directory_archived(&parent, id)
            }
            Err(e) => return Err(error(e)),
            _ => {}
        }
        let file = Self::archive_file(&parent, "manifest.json")?;
        let manifest: ArchiveManifest =
            serde_json::from_reader(file.take(65536)).map_err(|_| stale())?;
        if manifest.id != id.0
            || !crate::conflicts::resolvable(&manifest.local, &manifest.remote)
            || manifest.local.size != Evidence::Known(manifest.local_size)
            || manifest.remote.size != Evidence::Known(manifest.remote_size)
            || !matches!(&manifest.local.checksum,Evidence::Known(h) if h.algorithm=="sha256" && h.value==manifest.local_hash)
            || !matches!(&manifest.remote.checksum,Evidence::Known(h) if h.algorithm=="sha256" && h.value==manifest.remote_hash)
        {
            return Err(stale());
        }
        Self::verify_archive(
            &parent,
            "local.bin",
            manifest.local_size,
            &manifest.local_hash,
        )?;
        Self::verify_archive(
            &parent,
            "remote.bin",
            manifest.remote_size,
            &manifest.remote_hash,
        )?;
        Ok(true)
    }
    fn directory_archived(&self, parent: &File, id: &Id) -> Result<bool, OperatorError> {
        let path =
            PathBuf::from(format!("/proc/self/fd/{}", parent.as_raw_fd())).join("directory.json");
        match std::fs::symlink_metadata(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(error(e)),
            _ => {}
        }
        let file = Self::archive_file(parent, "directory.json")?;
        let record: DirectoryArchive =
            serde_json::from_reader(file.take(65536)).map_err(|_| stale())?;
        if record.id != id.0
            || record.local.kind != Evidence::Known(FileKind::Directory)
            || !crate::conflicts::resolvable(&record.local, &record.remote)
        {
            return Err(stale());
        }
        Ok(true)
    }
    pub(in crate::orchestrator::operations) fn archive_directory(
        &self,
        id: &Id,
        execution: &Id,
        local: &crate::inventory::models::LocalState,
        remote: &crate::inventory::models::RemoteState,
    ) -> Result<(), OperatorError> {
        let bytes = serde_json::to_vec(&DirectoryArchive {
            id: id.0.clone(),
            local: local.clone(),
            remote: remote.clone(),
        })
        .map_err(|_| invalid())?;
        self.archive_bytes(
            id,
            execution,
            "directory.json",
            &mut bytes.as_slice(),
            bytes.len() as u64,
            &format!("{:x}", Sha256::digest(&bytes)),
        )?;
        if !self.conflict_archived(id)? {
            return Err(stale());
        }
        Ok(())
    }
    pub(in crate::orchestrator::operations) fn archive_bytes(
        &self,
        id: &Id,
        execution: &Id,
        name: &str,
        source: &mut impl Read,
        size: u64,
        hash: &str,
    ) -> Result<(), OperatorError> {
        let parent = self.conflict_directory(id)?;
        if Self::verify_archive(&parent, name, size, hash).is_ok() {
            return Ok(());
        }
        let stage_name = format!(".stage-{}-{name}", execution_key(execution));
        let entry = Entry {
            parent: parent.try_clone().map_err(error)?,
            name: CString::new(stage_name).map_err(|_| invalid())?,
        };
        let mut stage = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(entry.anchored())
            .map_err(error)?;
        let mut copy = Span::start(Phase::ArchiveConflict, State::Running);
        let mut hash_progress = HashProgress::new(Phase::Checksum, Some(size), Algorithm::Sha256);
        let mut bytes = Bytes {
            total: Some(size),
            read: Some(0),
            written: Some(0),
            ..Bytes::default()
        };
        let result = (|| {
            let mut count = 0u64;
            let mut digest = Sha256::new();
            let mut buffer = [0; 65536];
            loop {
                let n = source.read(&mut buffer).map_err(error)?;
                if n == 0 {
                    break;
                }
                count += n as u64;
                bytes.read = Some(count);
                if count > size {
                    return Err(stale());
                }
                stage.write_all(&buffer[..n]).map_err(error)?;
                bytes.written = Some(count);
                copy.progress(Progress::Bytes(bytes), false);
                digest.update(&buffer[..n]);
                hash_progress.processed(n);
            }
            if count != size || format!("{:x}", digest.finalize()) != hash {
                return Err(stale());
            }
            Ok(())
        })();
        hash_progress.result(&result);
        copy.progress(Progress::Bytes(bytes), true);
        copy.result(&result);
        result?;
        stage.sync_all().map_err(error)?;
        let dest = Entry {
            parent: parent.try_clone().map_err(error)?,
            name: CString::new(name).map_err(|_| invalid())?,
        };
        rename(&entry, &dest, libc::RENAME_NOREPLACE)?;
        parent.sync_all().map_err(error)?;
        Self::verify_archive(&parent, name, size, hash)
    }
    pub(in crate::orchestrator::operations) fn seal_conflict(
        &self,
        id: &Id,
        execution: &Id,
        manifest: ArchiveManifest,
    ) -> Result<(), OperatorError> {
        let bytes = serde_json::to_vec(&manifest).map_err(|_| invalid())?;
        let hash = format!("{:x}", Sha256::digest(&bytes));
        self.archive_bytes(
            id,
            execution,
            "manifest.json",
            &mut bytes.as_slice(),
            bytes.len() as u64,
            &hash,
        )?;
        if !self.conflict_archived(id)? {
            return Err(stale());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::operations::tests::{setup, Sandbox};
    #[test]
    fn archives_are_verified_no_replace_and_reject_symlinked_private_directories() {
        let s = Sandbox::new();
        let executor = setup(&s);
        let id = Id("conflict-test".into());
        let execution = Id("execution".into());
        let local = b"local";
        let remote = b"remote";
        let lh = format!("{:x}", Sha256::digest(local));
        let rh = format!("{:x}", Sha256::digest(remote));
        std::fs::write(executor.config.local_root.join("file"), local).unwrap();
        let mut states = crate::orchestrator::operations::tests::request(
            &executor,
            OperationAction::UpdateRemoteContent,
        );
        states.remote.mime_type = Evidence::Known("application/octet-stream".into());
        states.remote.size = Evidence::Known(6);
        states.remote.checksum = Evidence::Known(Checksum {
            algorithm: "sha256".into(),
            value: rh.clone(),
        });
        assert!(!executor.filesystem.conflict_archived(&id).unwrap());
        executor
            .filesystem
            .archive_bytes(&id, &execution, "local.bin", &mut local.as_slice(), 5, &lh)
            .unwrap();
        executor
            .filesystem
            .archive_bytes(
                &id,
                &execution,
                "remote.bin",
                &mut remote.as_slice(),
                6,
                &rh,
            )
            .unwrap();
        executor
            .filesystem
            .seal_conflict(
                &id,
                &execution,
                ArchiveManifest {
                    id: id.0.clone(),
                    local: states.local.clone(),
                    remote: states.remote.clone(),
                    local_hash: lh.clone(),
                    local_size: 5,
                    remote_hash: rh.clone(),
                    remote_size: 6,
                },
            )
            .unwrap();
        assert!(executor.filesystem.conflict_archived(&id).unwrap());
        assert!(executor
            .filesystem
            .archive_bytes(
                &id,
                &Id("other".into()),
                "local.bin",
                &mut b"other".as_slice(),
                5,
                &format!("{:x}", Sha256::digest(b"other"))
            )
            .is_err());
        assert_eq!(
            std::fs::read(
                executor
                    .config
                    .local_root
                    .join(".drivesync/conflicts/conflict-test/local.bin")
            )
            .unwrap(),
            local
        );
        std::fs::write(
            executor
                .config
                .local_root
                .join(".drivesync/conflicts/conflict-test/remote.bin"),
            b"tamper",
        )
        .unwrap();
        assert!(executor.filesystem.conflict_archived(&id).is_err());
        let second = Sandbox::new();
        let second = setup(&second);
        std::os::unix::fs::symlink(&s.0, second.config.local_root.join(".drivesync")).unwrap();
        assert!(second.filesystem.conflict_archived(&id).is_err());
    }
}

#[cfg(test)]
mod directory_tests {
    use super::*;
    use crate::orchestrator::operations::{
        models::OperationAction,
        tests::{request, setup, Sandbox},
    };
    #[test]
    fn folder_location_archive_keeps_original_locations_and_leaves_descendants_untouched() {
        let s = Sandbox::new();
        let executor = setup(&s);
        std::fs::create_dir(executor.config.local_root.join("file")).unwrap();
        std::fs::write(
            executor.config.local_root.join("file/child"),
            b"child bytes",
        )
        .unwrap();
        let r = request(&executor, OperationAction::MoveLocal);
        let id = Id("directory-conflict".into());
        executor
            .filesystem
            .archive_directory(&id, &Id("execution".into()), &r.local, &r.remote)
            .unwrap();
        assert!(executor.filesystem.conflict_archived(&id).unwrap());
        let data = std::fs::read(
            executor
                .config
                .local_root
                .join(".drivesync/conflicts/directory-conflict/directory.json"),
        )
        .unwrap();
        let stored: DirectoryArchive = serde_json::from_slice(&data).unwrap();
        assert_eq!(stored.local, r.local);
        assert_eq!(stored.remote, r.remote);
        assert_eq!(
            std::fs::read(executor.config.local_root.join("file/child")).unwrap(),
            b"child bytes"
        );
    }
}
