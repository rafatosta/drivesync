//! Blob creation by verified private staging and exclusive atomic publication.
use super::*;
use md5::Md5;
use sha2::{Digest, Sha256};
use std::{
    io::{Seek, SeekFrom, Write},
    os::unix::ffi::OsStrExt,
};

impl Worker {
    pub(super) async fn download(&mut self) -> OperatorResult<TransferResult> {
        let OperationCommand::Download {
            source,
            destination,
            name,
        } = self.checkpoint.intent.command.clone()
        else {
            return Err(self.error(OperatorError::InvalidRequest));
        };
        let Some(SourceEvidence::Remote {
            metadata: expected,
            revision,
        }) = self.checkpoint.intent.context.source.clone()
        else {
            return Err(self.error(OperatorError::InvalidRequest));
        };
        let ResourceRef::Remote {
            provider_id,
            reference,
        } = source
        else {
            return Err(self.error(OperatorError::InvalidRequest));
        };
        if provider_id.0 != "google" || reference.account_id != self.scope.account_id {
            return Err(self.error(OperatorError::WrongScope));
        }
        let n = fs::local_name(&name)?;
        let parent = self.workspace.directory(&destination)?;
        let path = destination_path(&destination, &n)?;
        self.bind_scope(&parent)?;
        let id = self.checkpoint.intent.context.id.clone();
        if matches!(
            self.checkpoint.state.phase,
            CheckpointPhase::Prepared | CheckpointPhase::Executing
        ) {
            self.publication_effect = Some(ExecutionEffect::NotApplied);
        }
        if self.checkpoint.state.phase == CheckpointPhase::Completed {
            let result = self
                .completed_result()
                .ok_or_else(|| self.error(OperatorError::CheckpointUnavailable))?;
            self.verify_published(&parent, &n, &path)?;
            return Ok(result);
        }
        if self.checkpoint.state.staging.is_some() {
            match fs::Workspace::published(&parent, &n) {
                Ok(_) => {
                    if self.checkpoint.state.phase != CheckpointPhase::Verifying {
                        return Err(self.error(OperatorError::DestinationOccupied));
                    }
                    let result = self.verify_published(&parent, &n, &path)?;
                    return self.finish(result);
                }
                Err(e) if e.kind == OperatorError::NotFoundOrInaccessible => {}
                Err(e) => return Err(e),
            }
            if self.checkpoint.state.effect == ExecutionEffect::Applied {
                return Err(self.error(OperatorError::PreconditionFailed));
            }
        }
        fs::Workspace::free(&parent, &n)?;
        let before = self.within_root(&reference.file_id.0, false).await?;
        self.check_remote_source(&before, &expected)?;
        let size = match expected.size {
            MetadataValue::Known(n) => n,
            _ => return Err(self.error(OperatorError::InvalidRequest)),
        };
        let expected_sha = match &expected.sha256_checksum {
            MetadataValue::Known(s) => Some(
                Sha256Digest::new(s.clone())
                    .map_err(|_| self.error(OperatorError::InvalidRequest))?,
            ),
            _ => None,
        };
        let expected_md5 = match &expected.md5_checksum {
            MetadataValue::Known(s)
                if s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit()) =>
            {
                Some(s.to_ascii_lowercase())
            }
            MetadataValue::Known(_) => return Err(self.error(OperatorError::InvalidRequest)),
            _ => None,
        };
        if expected_sha.is_none() && expected_md5.is_none() {
            return Err(self.error(OperatorError::UnsupportedOperation));
        }
        if self.checkpoint.state.staging.is_none() {
            self.check()?;
            if self.checkpoint.state.phase == CheckpointPhase::Prepared {
                let mut next = self.checkpoint.state.clone();
                next.phase = CheckpointPhase::Executing;
                next.effect = ExecutionEffect::Unknown;
                self.save(next)?;
            }
            let mut stage = match self.workspace.stage(&id) {
                Ok(f) => f,
                Err(e) if e.kind == OperatorError::DestinationOccupied => {
                    self.workspace.existing_stage(&id)?
                }
                Err(e) => return Err(e),
            };
            // Interrupted, unverified bytes are never published or treated as a
            // resumable prefix. A retry safely downloads this blob from byte zero.
            stage.set_len(0).map_err(fs::io)?;
            stage.seek(SeekFrom::Start(0)).map_err(fs::io)?;
            let session = self
                .access
                .session()
                .await
                .map_err(|e| self.error(e.kind))?;
            let mut stream = if let Some(revision) = revision {
                if before
                    .head_revision_id
                    .as_ref()
                    .is_none_or(|r| r.0 != revision)
                {
                    return Err(self.error(OperatorError::PreconditionFailed));
                }
                self.access
                    .client
                    .get_revision_bytes(
                        &session,
                        &g::FileId(reference.file_id.0.clone()),
                        &g::RevisionId(revision),
                        None,
                    )
                    .await
            } else {
                self.access
                    .client
                    .get_file_bytes(&session, &g::FileId(reference.file_id.0.clone()), None)
                    .await
            }
            .map_err(|e| self.provider_error(e))?;
            let mut received = 0u64;
            let mut sha = Sha256::new();
            let mut md5 = Md5::new();
            fs::report(&self.options, ExecutionPhase::Transferring, 0, size);
            loop {
                self.check()?;
                let chunk = stream
                    .body
                    .next_chunk()
                    .await
                    .map_err(|e| self.provider_error(e))?;
                let Some(chunk) = chunk else {
                    break;
                };
                received = received
                    .checked_add(chunk.len() as u64)
                    .ok_or_else(|| self.error(OperatorError::IntegrityMismatch))?;
                if received > size {
                    return Err(self.error(OperatorError::IntegrityMismatch));
                }
                stage.write_all(&chunk).map_err(fs::io)?;
                sha.update(&chunk);
                md5.update(&chunk);
                self.transferred += chunk.len() as u64;
                fs::report(
                    &self.options,
                    ExecutionPhase::Transferring,
                    self.transferred,
                    size,
                );
            }
            let sha256 = Sha256Digest::new(format!("{:x}", sha.finalize()))
                .map_err(|_| self.error(OperatorError::Internal))?;
            let md5 = format!("{:x}", md5.finalize());
            if received != size
                || expected_sha.as_ref().is_some_and(|s| *s != sha256)
                || expected_md5.as_ref().is_some_and(|s| *s != md5)
            {
                return Err(self.error(OperatorError::IntegrityMismatch));
            }
            self.check()?;
            self.check_remote_source(
                &self.within_root(&reference.file_id.0, false).await?,
                &expected,
            )?;
            fs::report(
                &self.options,
                ExecutionPhase::Verifying,
                self.transferred,
                size,
            );
            stage.sync_all().map_err(fs::io)?;
            self.workspace.sync_stages()?;
            let evidence = StagingEvidence {
                metadata: Box::new(fs::observe(&stage, fs::Workspace::stage_path(&id))?),
                sha256,
            };
            self.workspace.verify_stage(&id, &evidence, &self.options)?;
            let mut next = self.checkpoint.state.clone();
            next.staging = Some(evidence);
            self.save(next)?;
        }
        self.check_remote_source(
            &self.within_root(&reference.file_id.0, false).await?,
            &expected,
        )?;
        self.check()?;
        if self
            .workspace
            .scope(&self.workspace.directory(&destination)?)?
            != self.checkpoint.state.local_scope.clone().unwrap()
        {
            return Err(self.error(OperatorError::WrongScope));
        }
        let staged = self
            .checkpoint
            .state
            .staging
            .clone()
            .ok_or_else(|| self.error(OperatorError::CheckpointUnavailable))?;
        self.workspace.verify_stage(&id, &staged, &self.options)?;
        fs::Workspace::free(&parent, &n)?;
        if self.checkpoint.state.phase != CheckpointPhase::Verifying {
            let mut next = self.checkpoint.state.clone();
            next.phase = CheckpointPhase::Verifying;
            next.effect = ExecutionEffect::Unknown;
            self.save(next)?;
        }
        fs::report(
            &self.options,
            ExecutionPhase::Applying,
            self.transferred,
            size,
        );
        // Check cancellation before entering the bounded atomic cutover. After
        // rename, complete verification despite a newly requested cancellation.
        self.check()?;
        let current_parent = self.workspace.directory(&destination)?;
        if fs::identity(&current_parent)? != fs::identity(&parent)? {
            return Err(self.error(OperatorError::WrongScope));
        }
        self.publication_effect = Some(ExecutionEffect::Unknown);
        if let Err(error) = self.workspace.publish(&id, &parent, &n) {
            self.publication_effect = Some(error.effect);
            return Err(error);
        }
        self.publication_effect = Some(ExecutionEffect::Applied);
        let result = self.verify_published(&parent, &n, &path)?;
        fs::report(
            &self.options,
            ExecutionPhase::Finalizing,
            self.transferred,
            size,
        );
        self.finish(result)
    }
    fn verify_published(
        &self,
        parent: &std::fs::File,
        n: &std::ffi::OsStr,
        path: &LocalRelativePath,
    ) -> OperatorResult<TransferResult> {
        let staged = self
            .checkpoint
            .state
            .staging
            .as_ref()
            .ok_or_else(|| self.error(OperatorError::CheckpointUnavailable))?;
        self.workspace.validate()?;
        let (current_parent, _) = self.workspace.parent(path)?;
        if Some(self.workspace.scope(&current_parent)?) != self.checkpoint.state.local_scope {
            return Err(self.error(OperatorError::WrongScope));
        }
        let file = fs::Workspace::published(parent, n)?;
        let metadata = fs::observe(&file, path.clone())?;
        if metadata.storage.inode != staged.metadata.storage.inode
            || metadata.storage.filesystem_device != staged.metadata.storage.filesystem_device
            || metadata.times.created_at != staged.metadata.times.created_at
            || metadata.storage.size_bytes != staged.metadata.storage.size_bytes
        {
            return Err(self.error(OperatorError::PreconditionFailed));
        }
        // Once publication may have happened, a cancellation request is not a
        // reason to leave its physical identity unverified or report NotApplied.
        let mut reading = file.try_clone().map_err(fs::io)?;
        if fs::hash(&mut reading, &ExecutionOptions::default())? != staged.sha256 {
            return Err(self.error(OperatorError::IntegrityMismatch));
        }
        let current = fs::Workspace::published(parent, n)?;
        if fs::identity(&current)? != fs::identity(&file)? {
            return Err(self.error(OperatorError::PreconditionFailed));
        }
        self.workspace.validate()?;
        let (current_parent, _) = self.workspace.parent(path)?;
        if Some(self.workspace.scope(&current_parent)?) != self.checkpoint.state.local_scope {
            return Err(self.error(OperatorError::WrongScope));
        }
        Ok(TransferResult {
            item: ItemResult {
                resource: ResourceRef::Local {
                    relative_path: path.clone(),
                    local_id: metadata.local_id.clone(),
                },
                metadata: Some(FileData::Local(metadata)),
                verification: VerificationStatus::Verified,
            },
            bytes_transferred: self.transferred,
            integrity: IntegrityStatus::Verified,
        })
    }
}
fn destination_path(
    parent: &DirectoryRef,
    name: &std::ffi::OsStr,
) -> OperatorResult<LocalRelativePath> {
    let mut bytes = match parent {
        DirectoryRef::LocalRoot => vec![],
        DirectoryRef::Local { relative_path, .. } => {
            let mut b = relative_path.as_bytes().to_vec();
            b.push(b'/');
            b
        }
        _ => return Err(fs::fail(OperatorError::InvalidRequest)),
    };
    bytes.extend_from_slice(name.as_bytes());
    LocalRelativePath::new(bytes).map_err(|_| fs::fail(OperatorError::InvalidRequest))
}
