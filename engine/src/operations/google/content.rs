//! Private bounded content materialization shared by file creation and replacement.
use super::*;
use md5::{Digest, Md5};
use sha2::Sha256;
use std::io::{Seek, SeekFrom, Write};
impl Worker {
    pub(super) async fn validate_content_source(&self) -> OperatorResult<()> {
        if let Some(source) = &self.checkpoint.intent.context.source {
            if matches!(
                self.checkpoint.intent.command,
                OperationCommand::Upload { .. }
                    | OperationCommand::CreateFile {
                        parent: DirectoryRef::Remote { .. },
                        ..
                    }
                    | OperationCommand::UpdateContent {
                        target: ResourceRef::Remote { .. },
                        ..
                    }
            ) && self.checkpoint.state.local_scope.is_some()
            {
                if let SourceEvidence::Local { metadata, .. } = source {
                    self.local_source(metadata)?;
                }
            }
            self.inspect_source(source).await?;
        }
        Ok(())
    }
    pub(super) async fn materialize(
        &self,
        content: &ContentSource,
    ) -> OperatorResult<StagingEvidence> {
        let id = &self.checkpoint.intent.context.id;
        if let Some(staging) = &self.checkpoint.state.staging {
            self.workspace.verify_stage(id, staging, &self.options)?;
            return Ok(staging.clone());
        }
        self.validate_content_source().await?;
        if let ContentSource::LocalFile { .. } = content {
            let Some(SourceEvidence::Local { metadata, sha256 }) =
                &self.checkpoint.intent.context.source
            else {
                return Err(self.error(OperatorError::InvalidRequest));
            };
            return self
                .workspace
                .snapshot(id, metadata, sha256.as_ref(), &self.options);
        }
        let mut output = match self.workspace.stage(id) {
            Ok(file) => file,
            Err(e) if e.kind == OperatorError::DestinationOccupied => {
                self.workspace.existing_stage(id)?
            }
            Err(e) => return Err(e),
        };
        output.set_len(0).map_err(fs::io)?;
        output.seek(SeekFrom::Start(0)).map_err(fs::io)?;
        let mut sha = Sha256::new();
        let mut md5 = Md5::new();
        let mut count = 0u64;
        match content {
            ContentSource::Bytes { data } => {
                for chunk in data.chunks(65536) {
                    self.check()?;
                    output.write_all(chunk).map_err(fs::io)?;
                    sha.update(chunk);
                    md5.update(chunk);
                    count += chunk.len() as u64;
                    fs::report(
                        &self.options,
                        ExecutionPhase::Preparing,
                        count,
                        data.len() as u64,
                    );
                }
            }
            ContentSource::RemoteFile {
                resource:
                    ResourceRef::Remote {
                        provider_id,
                        reference,
                    },
            } => {
                let Some(SourceEvidence::Remote { metadata, revision }) =
                    &self.checkpoint.intent.context.source
                else {
                    return Err(self.error(OperatorError::InvalidRequest));
                };
                if metadata.provider != *provider_id || metadata.reference != *reference {
                    return Err(self.error(OperatorError::WrongScope));
                }
                let size = match metadata.size {
                    MetadataValue::Known(size) => size,
                    _ => return Err(self.error(OperatorError::InvalidRequest)),
                };
                let session = self.access.session().await?;
                let mut stream = if let Some(revision) = revision {
                    self.access
                        .client
                        .get_revision_bytes(
                            &session,
                            &g::FileId(reference.file_id.0.clone()),
                            &g::RevisionId(revision.clone()),
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
                while let Some(chunk) = stream
                    .body
                    .next_chunk()
                    .await
                    .map_err(|e| self.provider_error(e))?
                {
                    self.check()?;
                    count = count
                        .checked_add(chunk.len() as u64)
                        .ok_or_else(|| self.error(OperatorError::IntegrityMismatch))?;
                    if count > size {
                        return Err(self.error(OperatorError::IntegrityMismatch));
                    }
                    output.write_all(&chunk).map_err(fs::io)?;
                    sha.update(&chunk);
                    md5.update(&chunk);
                    fs::report(&self.options, ExecutionPhase::Preparing, count, size);
                }
                let sha_hex = format!("{:x}", sha.clone().finalize());
                let md5_hex = format!("{:x}", md5.clone().finalize());
                let mut verified = false;
                for (expected, actual) in [
                    (&metadata.sha256_checksum, &sha_hex),
                    (&metadata.md5_checksum, &md5_hex),
                ] {
                    if let MetadataValue::Known(expected) = expected {
                        verified = true;
                        if !expected.eq_ignore_ascii_case(actual) {
                            return Err(self.error(OperatorError::IntegrityMismatch));
                        }
                    }
                }
                if count != size || !verified {
                    return Err(self.error(OperatorError::IntegrityMismatch));
                }
            }
            _ => return Err(self.error(OperatorError::InvalidRequest)),
        }
        self.validate_content_source().await?;
        output.sync_all().map_err(fs::io)?;
        self.workspace.sync_stages()?;
        let evidence = StagingEvidence {
            metadata: Box::new(fs::observe(&output, fs::Workspace::stage_path(id))?),
            sha256: Sha256Digest::new(format!("{:x}", sha.finalize()))
                .map_err(|_| self.error(OperatorError::Internal))?,
        };
        self.workspace.verify_stage(id, &evidence, &self.options)?;
        Ok(evidence)
    }
}
impl Worker {
    pub(super) fn content_size(&self, content: &ContentSource) -> OperatorResult<u64> {
        match content {
            ContentSource::Bytes { data } => Ok(data.len() as u64),
            _ => match &self.checkpoint.intent.context.source {
                Some(SourceEvidence::Local { metadata, .. }) => match metadata.storage.size_bytes {
                    MetadataValue::Known(size) => Ok(size),
                    _ => Err(self.error(OperatorError::InvalidRequest)),
                },
                Some(SourceEvidence::Remote { metadata, .. }) => match metadata.size {
                    MetadataValue::Known(size) => Ok(size),
                    _ => Err(self.error(OperatorError::InvalidRequest)),
                },
                _ => Err(self.error(OperatorError::InvalidRequest)),
            },
        }
    }
    pub(super) async fn replacement_snapshot(
        &self,
        reference: &RemoteFileRef,
        p: &Preconditions,
    ) -> OperatorResult<g::GoogleFileSnapshot> {
        self.within_root(&reference.file_id.0, false).await?;
        let session = self.access.session().await?;
        let snapshot = self
            .access
            .client
            .files_get_snapshot(
                &session,
                &g::FileId(reference.file_id.0.clone()),
                g::FileGetOptions {
                    supports_all_drives: Some(true),
                    fields: Some(g::FieldMask(FIELDS.into())),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| self.provider_error(e))?;
        let file = &snapshot.file;
        if file
            .id
            .as_ref()
            .is_none_or(|id| id.0 != reference.file_id.0)
            || file
                .mime_type
                .as_ref()
                .is_none_or(|m| m.0.starts_with("application/vnd.google-apps."))
            || file.trashed != Some(false)
        {
            return Err(self.error(OperatorError::PreconditionFailed));
        }
        let parents = file
            .parents
            .as_ref()
            .filter(|p| p.len() == 1)
            .ok_or_else(|| self.error(OperatorError::WrongScope))?;
        self.within_root(&parents[0].0, true).await?;
        if self.checkpoint.state.phase == CheckpointPhase::Completed {
            return Ok(snapshot);
        }
        if self.checkpoint.state.phase != CheckpointPhase::Prepared {
            if let Some(staged) = &self.checkpoint.state.staging {
                let size = match staged.metadata.storage.size_bytes {
                    MetadataValue::Known(size) => size,
                    _ => return Err(self.error(OperatorError::InvalidRequest)),
                };
                if let Some(name) = &file.name {
                    if self
                        .verified_upload(file, name, &parents[0].0, size)
                        .is_ok()
                    {
                        return Ok(snapshot);
                    }
                }
            }
        }
        if p.expected_version
            .as_ref()
            .is_none_or(|v| file.version.as_ref() != Some(v))
            || p.expected_revision
                .as_ref()
                .is_some_and(|r| file.head_revision_id.as_ref().is_none_or(|id| &id.0 != r))
        {
            return Err(self.error(OperatorError::PreconditionFailed));
        }
        if file.capabilities.as_ref().and_then(|c| c.can_edit) != Some(true)
            || snapshot.etag.is_none()
        {
            return Err(self.error(OperatorError::UnsupportedOperation));
        }
        Ok(snapshot)
    }
}
