//! Preserve divergent byte streams before applying an explicit conflict choice.
use super::*;
use std::io::{Seek, SeekFrom};
impl GoogleOperator {
    pub(super) async fn preserve_conflict(
        &self,
        r: &ExecuteOperationRequest,
        fs: &Filesystem,
        session: &GoogleSession,
    ) -> Result<(), OperatorError> {
        let Some(id) = &r.operation.conflict_id else {
            return Ok(());
        };
        if fs.conflict_archived(id)? {
            return Ok(());
        }
        if !crate::conflicts::resolvable(&r.local, &r.remote) {
            return Err(filesystem::stale());
        }
        let remote_id = r
            .operation
            .remote_id
            .as_ref()
            .ok_or_else(filesystem::stale)?;
        let before = self.snapshot(session, remote_id).await?;
        if !crate::conflicts::remote_matches(&normalize(&before.file), &r.remote) {
            return Err(filesystem::stale());
        }
        fs.check(&r.local)?;
        if r.local.kind == Evidence::Known(FileKind::Directory) {
            fs.archive_directory(id, &r.execution_id, &r.local, &r.remote)?;
            let after = self.snapshot(session, remote_id).await?;
            if after.file.version != before.file.version {
                return Err(filesystem::stale());
            }
            fs.check(&r.local)?;
            return Ok(());
        }
        let (
            Evidence::Known(local_hash),
            Evidence::Known(remote_hash),
            Evidence::Known(local_size),
            Evidence::Known(remote_size),
        ) = (
            &r.local.checksum,
            &r.remote.checksum,
            &r.local.size,
            &r.remote.size,
        )
        else {
            return Err(filesystem::stale());
        };
        let mut source = fs.read(r.local.path.as_deref().ok_or_else(filesystem::stale)?)?;
        fs.archive_bytes(
            id,
            &r.execution_id,
            "local.bin",
            &mut source,
            *local_size,
            &local_hash.value,
        )?;
        let mut download = self
            .client
            .get_file_bytes(session, &FileId(remote_id.0.clone()), None)
            .await
            .map_err(read_error)?;
        let mut temp = fs.temporary()?;
        receive_verified(
            &mut download.body,
            &mut temp,
            *remote_size,
            &remote_hash.value,
        )
        .await?;
        temp.seek(SeekFrom::Start(0)).map_err(filesystem::error)?;
        fs.archive_bytes(
            id,
            &r.execution_id,
            "remote.bin",
            &mut temp,
            *remote_size,
            &remote_hash.value,
        )?;
        let after = self.snapshot(session, remote_id).await?;
        if after.file.version != before.file.version {
            return Err(filesystem::stale());
        }
        fs.check(&r.local)?;
        fs.seal_conflict(
            id,
            &r.execution_id,
            filesystem::conflicts::ArchiveManifest {
                id: id.0.clone(),
                local: r.local.clone(),
                remote: r.remote.clone(),
                local_hash: local_hash.value.clone(),
                local_size: *local_size,
                remote_hash: remote_hash.value.clone(),
                remote_size: *remote_size,
            },
        )
    }
}
