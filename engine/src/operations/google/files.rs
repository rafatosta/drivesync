//! File creation and local replacement by verified staging. Replaced bytes are
//! retained in private staging; namespace races never trigger destructive rollback.
use super::*;
use std::{
    io::{Seek, SeekFrom},
    os::unix::fs::MetadataExt,
};
impl Worker {
    pub(super) async fn create_file(&mut self) -> OperatorResult<ItemResult> {
        let OperationCommand::CreateFile {
            parent,
            name,
            content,
        } = self.checkpoint.intent.command.clone()
        else {
            return Err(self.error(OperatorError::InvalidRequest));
        };
        if matches!(parent, DirectoryRef::Remote { .. }) {
            return self.upload().await.map(|result| result.item);
        }
        self.local_file(
            parent,
            name,
            content.unwrap_or(ContentSource::Bytes { data: vec![] }),
            None,
        )
        .await
        .map(|result| result.item)
    }
    pub(super) async fn update_content(&mut self) -> OperatorResult<TransferResult> {
        let OperationCommand::UpdateContent {
            target,
            source,
            preconditions,
        } = self.checkpoint.intent.command.clone()
        else {
            return Err(self.error(OperatorError::InvalidRequest));
        };
        let ResourceRef::Local {
            relative_path,
            local_id: Some(local_id),
        } = target
        else {
            return self.upload().await;
        };
        let (parent, n) = self.workspace.parent(&relative_path)?;
        let _ = parent;
        let directory = super::mutations::parent_ref(&relative_path);
        self.local_file(
            directory,
            ItemName::Local(n),
            source,
            Some((relative_path, local_id, preconditions)),
        )
        .await
    }
    async fn local_file(
        &mut self,
        parent_ref: DirectoryRef,
        name: ItemName,
        content: ContentSource,
        replace: Option<(LocalRelativePath, LocalFileId, Preconditions)>,
    ) -> OperatorResult<TransferResult> {
        let parent = self.workspace.directory(&parent_ref)?;
        let n = fs::local_name(&name)?;
        let path = super::mutations::local_path(&parent_ref, &n)?;
        let id = self.checkpoint.intent.context.id.clone();
        self.bind_scope(&parent)?;
        if let Some(staging) = self.checkpoint.state.staging.clone() {
            if let Ok(file) = fs::Workspace::published(&parent, &n) {
                let observed = fs::observe(&file, path.clone())?;
                if observed.storage.inode == staging.metadata.storage.inode
                    && observed.storage.filesystem_device
                        == staging.metadata.storage.filesystem_device
                {
                    let result = self.verify_file_publication(&path, &staging)?;
                    if self.checkpoint.state.phase == CheckpointPhase::Prepared {
                        return Err(self.error(OperatorError::PreconditionFailed));
                    }
                    if self.checkpoint.state.phase == CheckpointPhase::Completed {
                        return self
                            .completed_result()
                            .ok_or_else(|| self.error(OperatorError::CheckpointUnavailable));
                    }
                    return self.finish(result);
                }
            }
        }
        if self.checkpoint.state.phase == CheckpointPhase::Completed {
            return Err(self.error(OperatorError::PreconditionFailed));
        }
        let old = if let Some((target, id, conditions)) = &replace {
            if conditions.expected_revision.is_none() {
                return Err(self.error(OperatorError::PreconditionFailed));
            }
            let file = self.workspace.resource(target, Some(id), false)?;
            if file.metadata().map_err(fs::io)?.uid() != unsafe { libc::geteuid() } {
                return Err(self.error(OperatorError::PermissionDenied));
            }
            let metadata = fs::observe(&file, target.clone())?;
            if metadata.kind != MetadataValue::Known(LocalEntryKind::Regular) {
                return Err(self.error(OperatorError::InvalidRequest));
            }
            super::mutations::check_local_conditions(&metadata, conditions)?;
            Some((file, metadata))
        } else {
            fs::Workspace::free(&parent, &n)?;
            None
        };
        let mut evidence = self.materialize(&content).await?;
        if self.checkpoint.state.staging.is_none() {
            if let Some((old, _)) = &old {
                let stage = self.workspace.existing_stage(&id)?;
                stage
                    .set_permissions(old.metadata().map_err(fs::io)?.permissions())
                    .map_err(fs::io)?;
                stage.sync_all().map_err(fs::io)?;
                *evidence.metadata = fs::observe(&stage, fs::Workspace::stage_path(&id))?;
            }
            let mut next = self.checkpoint.state.clone();
            next.staging = Some(evidence.clone());
            self.save(next)?;
        }
        self.validate_content_source().await?;
        self.workspace.verify_stage(&id, &evidence, &self.options)?;
        if self.checkpoint.state.phase == CheckpointPhase::Prepared {
            let mut next = self.checkpoint.state.clone();
            next.phase = CheckpointPhase::Executing;
            next.effect = ExecutionEffect::Unknown;
            self.save(next)?;
        }
        fs::report(&self.options, ExecutionPhase::Applying, 0, 0);
        self.check()?;
        if fs::identity(&self.workspace.directory(&parent_ref)?)? != fs::identity(&parent)? {
            return Err(self.error(OperatorError::WrongScope));
        }
        if let Some((old, metadata)) = &old {
            let current = self
                .workspace
                .resource(&path, metadata.local_id.as_ref(), false)?;
            fs::matches_observation(&current, metadata)?;
            if fs::identity(&current)? != fs::identity(old)? {
                return Err(self.error(OperatorError::PreconditionFailed));
            }
            self.workspace.exchange_stage(&id, &parent, &n)?;
            self.publication_effect = Some(ExecutionEffect::Applied);
            if fs::identity(&self.workspace.existing_stage(&id)?)? != fs::identity(old)? {
                return Err(self.error(OperatorError::IndeterminateEffect));
            }
        } else {
            if let Err(error) = self.workspace.publish(&id, &parent, &n) {
                self.publication_effect = Some(error.effect);
                return Err(error);
            }
            self.publication_effect = Some(ExecutionEffect::Applied);
        }
        let result = self.verify_file_publication(&path, &evidence)?;
        self.finish(result)
    }
    fn verify_file_publication(
        &self,
        path: &LocalRelativePath,
        expected: &StagingEvidence,
    ) -> OperatorResult<TransferResult> {
        let (parent, n) = self.workspace.parent(path)?;
        if self.checkpoint.state.local_scope.as_ref() != Some(&self.workspace.scope(&parent)?) {
            return Err(self.error(OperatorError::WrongScope));
        }
        let mut file = fs::Workspace::published(&parent, &n)?;
        let actual = fs::observe(&file, path.clone())?;
        if actual.storage.inode != expected.metadata.storage.inode
            || actual.storage.filesystem_device != expected.metadata.storage.filesystem_device
            || actual.times.created_at != expected.metadata.times.created_at
            || actual.storage.size_bytes != expected.metadata.storage.size_bytes
            || fs::hash(&mut file, &ExecutionOptions::default())? != expected.sha256
        {
            return Err(self.error(OperatorError::IntegrityMismatch));
        }
        file.seek(SeekFrom::Start(0)).map_err(fs::io)?;
        if fs::identity(&fs::Workspace::published(&parent, &n)?)? != fs::identity(&file)? {
            return Err(self.error(OperatorError::PreconditionFailed));
        }
        Ok(TransferResult {
            item: ItemResult {
                resource: ResourceRef::Local {
                    relative_path: path.clone(),
                    local_id: actual.local_id.clone(),
                },
                metadata: Some(FileData::Local(actual)),
                verification: VerificationStatus::Verified,
            },
            bytes_transferred: self.transferred,
            integrity: IntegrityStatus::Verified,
        })
    }
}
