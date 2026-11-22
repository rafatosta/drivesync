//! Tagged staging makes publication attributable across process loss without trusting a path/hash alone.
use super::*;
use crate::inventory::models::{RecoveryCheckpoint, RecoveryRecord};
use crate::orchestrator::operations::journal::ExecutionJournal;
const MARKER: &std::ffi::CStr = c"user.drivesync.execution";
fn identity(meta: &std::fs::Metadata) -> LocalIdentity {
    LocalIdentity {
        device_id: meta.dev(),
        inode: meta.ino(),
        mount_id: None,
    }
}
impl Filesystem {
    pub(in crate::orchestrator::operations) fn root_identity(
        &self,
    ) -> Result<LocalIdentity, OperatorError> {
        Ok(identity(&self.root.metadata().map_err(error)?))
    }
    pub(in crate::orchestrator::operations) fn parent_identity(
        &self,
        path: &StdPath,
    ) -> Result<LocalIdentity, OperatorError> {
        Ok(identity(
            &self.entry(path)?.parent.metadata().map_err(error)?,
        ))
    }
    pub(in crate::orchestrator::operations) fn stage_path(
        request: &ExecuteOperationRequest,
    ) -> Result<PathBuf, OperatorError> {
        Ok(request
            .operation
            .local_path
            .as_ref()
            .ok_or_else(invalid)?
            .with_file_name(format!(
                ".drivesync-transfer-{}",
                execution_key(&request.execution_id)
            )))
    }
    pub(super) fn mark_stage(
        &self,
        file: &File,
        request: &ExecuteOperationRequest,
        path: &StdPath,
        journal: &dyn ExecutionJournal,
    ) -> Result<(), OperatorError> {
        let record = journal.record();
        if record.stage_path.as_deref() != Some(path)
            || record.command != request.operation
            || record.execution_id != request.execution_id
        {
            return Err(stale());
        }
        let marker = request.execution_id.0.as_bytes();
        if unsafe {
            libc::fsetxattr(
                file.as_raw_fd(),
                MARKER.as_ptr(),
                marker.as_ptr().cast(),
                marker.len(),
                libc::XATTR_CREATE,
            )
        } != 0
        {
            return Err(error(std::io::Error::last_os_error()).uncertain());
        }
        file.sync_all().map_err(|e| error(e).uncertain())?;
        self.entry(path)?
            .parent
            .sync_all()
            .map_err(|e| error(e).uncertain())?;
        journal.checkpoint(RecoveryCheckpoint::LocalStage(identity(
            &file.metadata().map_err(error)?,
        )))
    }
    pub(super) fn create_staged_directory(
        &self,
        request: &ExecuteOperationRequest,
        journal: &dyn ExecutionJournal,
    ) -> Result<LocalState, OperatorError> {
        let path = request.operation.local_path.as_ref().ok_or_else(invalid)?;
        self.check_absent(path)?;
        let stage_path = Self::stage_path(request)?;
        let stage = self.entry(&stage_path)?;
        let destination = self.entry(path)?;
        if unsafe { libc::mkdirat(stage.parent.as_raw_fd(), stage.name.as_ptr(), 0o700) } != 0 {
            return Err(error(std::io::Error::last_os_error()));
        }
        let file = directory(&stage.anchored()).map_err(OperatorError::uncertain)?;
        self.mark_stage(&file, request, &stage_path, journal)?;
        rename(&stage, &destination, libc::RENAME_NOREPLACE).map_err(OperatorError::uncertain)?;
        destination
            .parent
            .sync_all()
            .map_err(|e| error(e).uncertain())?;
        self.observe(path).map_err(OperatorError::uncertain)
    }
    /// The marker and persisted staged inode are both necessary; a matching name/hash is insufficient.
    pub(in crate::orchestrator::operations) fn published(
        &self,
        record: &RecoveryRecord,
    ) -> Result<Option<LocalState>, OperatorError> {
        let path = record.command.local_path.as_ref().ok_or_else(invalid)?;
        let Some(expected) = &record.stage_identity else {
            self.check_absent(path)?;
            return Ok(None);
        };
        let local = self.observe(path)?;
        if local.identity != Evidence::Known(expected.clone()) {
            return Err(stale());
        }
        let entry = self.entry(path)?;
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(entry.anchored())
            .map_err(error)?;
        if identity(&file.metadata().map_err(error)?) != *expected {
            return Err(stale());
        }
        let mut marker = vec![0u8; 256];
        let count = unsafe {
            libc::fgetxattr(
                file.as_raw_fd(),
                MARKER.as_ptr(),
                marker.as_mut_ptr().cast(),
                marker.len(),
            )
        };
        if count < 0 || &marker[..count as usize] != record.execution_id.0.as_bytes() {
            return Err(stale());
        }
        entry.parent.sync_all().map_err(error)?;
        Ok(Some(local))
    }
}
