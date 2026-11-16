//! Resumable create-only upload. Every retry reconciles the reserved identity
//! and asks the server for its offset before sending any further snapshot bytes.
use super::*;
use md5::{Digest, Md5};
use std::io::{Read, Seek, SeekFrom};

impl Worker {
    pub(super) async fn upload(&mut self) -> OperatorResult<TransferResult> {
        let command = self.checkpoint.intent.command.clone();
        let mut replacement = None;
        let (content, destination, name) = match command {
            OperationCommand::Upload {
                source:
                    ResourceRef::Local {
                        relative_path,
                        local_id,
                    },
                destination,
                name,
            } => (
                ContentSource::LocalFile {
                    relative_path,
                    local_id,
                },
                destination,
                name,
            ),
            OperationCommand::CreateFile {
                parent,
                name,
                content,
            } => (
                content.unwrap_or(ContentSource::Bytes { data: vec![] }),
                parent,
                name,
            ),
            OperationCommand::UpdateContent {
                target:
                    ResourceRef::Remote {
                        provider_id,
                        reference,
                    },
                source,
                preconditions,
            } => {
                if provider_id.0 != "google"
                    || reference.account_id != self.scope.account_id
                    || reference.file_id == self.scope.remote_root_id
                {
                    return Err(self.error(OperatorError::WrongScope));
                }
                let snapshot = self
                    .replacement_snapshot(&reference, &preconditions)
                    .await?;
                let file = &snapshot.file;
                let parent = file
                    .parents
                    .as_ref()
                    .filter(|p| p.len() == 1)
                    .ok_or_else(|| self.error(OperatorError::WrongScope))?;
                let name = file
                    .name
                    .clone()
                    .ok_or_else(|| self.error(OperatorError::PreconditionFailed))?;
                let destination = DirectoryRef::Remote {
                    provider_id: provider_id.clone(),
                    reference: RemoteFileRef {
                        account_id: reference.account_id.clone(),
                        file_id: RemoteFileId(parent[0].0.clone()),
                    },
                };
                replacement = Some((reference, preconditions, snapshot));
                (source, destination, ItemName::Remote(name))
            }
            _ => return Err(self.error(OperatorError::InvalidRequest)),
        };
        let DirectoryRef::Remote {
            provider_id,
            reference: parent_ref,
        } = destination
        else {
            return Err(self.error(OperatorError::WrongScope));
        };
        if provider_id.0 != "google" || parent_ref.account_id != self.scope.account_id {
            return Err(self.error(OperatorError::WrongScope));
        }
        let ItemName::Remote(name) = name else {
            return Err(self.error(OperatorError::InvalidRequest));
        };
        if name.is_empty()
            || name.len() > 255
            || name.contains(['\0', '/'])
            || matches!(name.as_str(), "." | ".." | ".drivesync")
        {
            return Err(self.error(OperatorError::InvalidRequest));
        }
        let source_parent = match &self.checkpoint.intent.context.source {
            Some(SourceEvidence::Local { metadata, .. }) => {
                self.workspace.parent(&metadata.relative_path)?.0
            }
            _ => self.workspace.root.try_clone().map_err(fs::io)?,
        };
        self.bind_scope(&source_parent)?;
        let id = self.checkpoint.intent.context.id.clone();
        let size = self.content_size(&content)?;
        let parent = self.within_root(&parent_ref.file_id.0, true).await?;
        if replacement.is_none()
            && self.checkpoint.state.phase != CheckpointPhase::Completed
            && parent
                .capabilities
                .as_ref()
                .and_then(|c| c.can_add_children)
                != Some(true)
        {
            return Err(self.error(OperatorError::PermissionDenied));
        }
        if self.checkpoint.state.phase == CheckpointPhase::Completed {
            let remote = self
                .remote_creation()
                .await?
                .ok_or_else(|| self.error(OperatorError::PreconditionFailed))?;
            self.verified_upload(&remote, &name, &parent_ref.file_id.0, size)?;
            return self
                .completed_result()
                .ok_or_else(|| self.error(OperatorError::CheckpointUnavailable));
        }
        if self.checkpoint.state.staging.is_none() {
            fs::report(&self.options, ExecutionPhase::Preparing, 0, size);
            let evidence = self.materialize(&content).await?;
            let mut next = self.checkpoint.state.clone();
            next.staging = Some(evidence);
            self.save(next)?;
        }
        let staged = self
            .checkpoint
            .state
            .staging
            .clone()
            .ok_or_else(|| self.error(OperatorError::CheckpointUnavailable))?;
        let mut snapshot = self.workspace.verify_stage(&id, &staged, &self.options)?;
        if let Some((reference, _, _)) = &replacement {
            if self.checkpoint.state.reserved_remote.is_none() {
                let mut next = self.checkpoint.state.clone();
                next.reserved_remote = Some(ResourceRef::Remote {
                    provider_id: ProviderId("google".into()),
                    reference: reference.clone(),
                });
                self.save(next)?;
            }
        }
        if self.checkpoint.state.reserved_remote.is_none() {
            self.validate_content_source().await?;
            if replacement.is_none() {
                self.no_homonym(&parent_ref.file_id.0, &name).await?;
            }
            let session = self
                .access
                .session()
                .await
                .map_err(|e| self.error(e.kind))?;
            let generated = self
                .access
                .client
                .files_generate_ids(
                    &session,
                    g::GenerateIdsOptions {
                        count: Some(1),
                        space: Some("drive".into()),
                        type_: Some("files".into()),
                    },
                )
                .await
                .map_err(|e| self.provider_error(e))?;
            let ids = generated
                .ids
                .filter(|ids| ids.len() == 1)
                .ok_or_else(|| self.error(OperatorError::IndeterminateEffect))?;
            let remote_id = ids[0].0.clone();
            if remote_id.is_empty()
                || remote_id.len() > 190
                || !remote_id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
            {
                return Err(self.error(OperatorError::IndeterminateEffect));
            }
            let mut next = self.checkpoint.state.clone();
            next.reserved_remote = Some(ResourceRef::Remote {
                provider_id,
                reference: RemoteFileRef {
                    account_id: self.scope.account_id.clone(),
                    file_id: RemoteFileId(remote_id),
                },
            });
            self.save(next)?;
        }
        if let Some((_, _, snapshot)) = &replacement {
            if self.checkpoint.state.phase != CheckpointPhase::Prepared
                && self
                    .verified_upload(&snapshot.file, &name, &parent_ref.file_id.0, size)
                    .is_ok()
            {
                let result =
                    self.verified_upload(&snapshot.file, &name, &parent_ref.file_id.0, size)?;
                return self.finish(result);
            }
        } else {
            if let Some(remote) = self.remote_creation().await? {
                if self.checkpoint.state.phase == CheckpointPhase::Prepared {
                    return Err(self.error(OperatorError::PreconditionFailed));
                }
                let result = self.verified_upload(&remote, &name, &parent_ref.file_id.0, size)?;
                return self.finish(result);
            }
        }
        if self.checkpoint.state.phase == CheckpointPhase::Verifying {
            return Err(self.error(OperatorError::IndeterminateEffect));
        }
        self.validate_content_source().await?;
        if self.checkpoint.state.phase == CheckpointPhase::Prepared {
            let mut next = self.checkpoint.state.clone();
            next.phase = CheckpointPhase::Executing;
            next.effect = ExecutionEffect::Unknown;
            self.save(next)?;
        }
        let mut restarts = 0;
        loop {
            self.check()?;
            if self.checkpoint.state.resume_token.is_none() {
                self.workspace.verify_stage(&id, &staged, &self.options)?;
                self.validate_content_source().await?;
                if replacement.is_none() {
                    self.no_homonym(&parent_ref.file_id.0, &name).await?;
                }
                let parent = self.within_root(&parent_ref.file_id.0, true).await?;
                if replacement.is_none()
                    && parent
                        .capabilities
                        .as_ref()
                        .and_then(|c| c.can_add_children)
                        != Some(true)
                {
                    return Err(self.error(OperatorError::PermissionDenied));
                }
                let session = self
                    .access
                    .session()
                    .await
                    .map_err(|e| self.error(e.kind))?;
                let remote_id = self.reserved_id()?;
                let upload = if let Some((reference, conditions, _)) = &replacement {
                    let current = self.replacement_snapshot(reference, conditions).await?;
                    let etag = current
                        .etag
                        .ok_or_else(|| self.error(OperatorError::UnsupportedOperation))?;
                    self.access
                        .client
                        .start_resumable_update_conditional(
                            &session,
                            &g::FileId(reference.file_id.0.clone()),
                            g::GoogleFileWrite::default(),
                            g::FileUpdateOptions {
                                supports_all_drives: Some(true),
                                fields: Some(g::FieldMask(FIELDS.into())),
                                ..Default::default()
                            },
                            Some(size),
                            &etag,
                        )
                        .await
                        .map_err(|e| self.provider_error(e))?
                } else {
                    self.access
                        .client
                        .start_resumable_create(
                            &session,
                            g::GoogleFileWrite {
                                id: Some(g::FileId(remote_id)),
                                name: g::GooglePatch::Set(name.clone()),
                                parents: g::GooglePatch::Set(vec![g::FileId(
                                    parent_ref.file_id.0.clone(),
                                )]),
                                mime_type: g::GooglePatch::Set(g::MimeType(
                                    "application/octet-stream".into(),
                                )),
                                ..Default::default()
                            },
                            g::FileCreateOptions {
                                supports_all_drives: Some(true),
                                fields: Some(g::FieldMask(FIELDS.into())),
                                ..Default::default()
                            },
                            Some(size),
                        )
                        .await
                        .map_err(|e| self.provider_error(e))?
                };
                let mut next = self.checkpoint.state.clone();
                next.resume_token = Some(
                    ResumeToken::new(upload.session_uri.into_bytes())
                        .map_err(|_| self.error(OperatorError::IndeterminateEffect))?,
                );
                self.save(next)?;
            }
            let upload = self.upload_session(size)?;
            let session = self
                .access
                .session()
                .await
                .map_err(|e| self.error(e.kind))?;
            let progress = if let Some((reference, conditions, _)) = &replacement {
                let current = self.replacement_snapshot(reference, conditions).await?;
                let etag = current
                    .etag
                    .ok_or_else(|| self.error(OperatorError::UnsupportedOperation))?;
                self.access
                    .client
                    .query_upload_progress_conditional(&session, &upload, &etag)
                    .await
            } else {
                self.access
                    .client
                    .query_upload_progress(&session, &upload)
                    .await
            };
            let mut progress = match progress {
                Ok(progress) => progress,
                Err(e) if e.kind == g::GoogleApiError::ExpiredUploadSession => {
                    if let Some(remote) = self.remote_creation().await? {
                        if replacement.is_none()
                            || self
                                .verified_upload(&remote, &name, &parent_ref.file_id.0, size)
                                .is_ok()
                        {
                            let result =
                                self.verified_upload(&remote, &name, &parent_ref.file_id.0, size)?;
                            return self.finish(result);
                        }
                    }
                    // The same reserved ID is retained even if a 404 concealed a
                    // lost completion. Google rejects recreation of a used ID.
                    self.checkpoint =
                        self.journal
                            .reset_expired_upload(ResetExpiredUploadRequest {
                                id: id.clone(),
                                expected_revision: self.checkpoint.revision,
                                expired_session: self
                                    .checkpoint
                                    .state
                                    .resume_token
                                    .clone()
                                    .unwrap(),
                                reserved_remote: self
                                    .checkpoint
                                    .state
                                    .reserved_remote
                                    .clone()
                                    .unwrap(),
                            })?;
                    restarts += 1;
                    if restarts > 1 {
                        return Err(self.error(OperatorError::Network));
                    }
                    continue;
                }
                Err(e) => return Err(self.provider_error(e)),
            };
            loop {
                match progress {
                    g::GoogleUploadProgress::Complete { file } => {
                        self.verified_upload(&file, &name, &parent_ref.file_id.0, size)?;
                        self.publication_effect = Some(ExecutionEffect::Applied);
                        let mut next = self.checkpoint.state.clone();
                        next.phase = CheckpointPhase::Verifying;
                        next.effect = ExecutionEffect::Applied;
                        self.save(next)?;
                        let remote = self.within_root(&self.reserved_id()?, false).await?;
                        let result =
                            self.verified_upload(&remote, &name, &parent_ref.file_id.0, size)?;
                        fs::report(
                            &self.options,
                            ExecutionPhase::Finalizing,
                            self.transferred,
                            size,
                        );
                        return self.finish(result);
                    }
                    g::GoogleUploadProgress::Incomplete { received_range } => {
                        let offset = match received_range {
                            None => 0,
                            Some(range) if range.start == 0 => range
                                .end_inclusive
                                .and_then(|n| n.checked_add(1))
                                .ok_or_else(|| self.error(OperatorError::IndeterminateEffect))?,
                            _ => return Err(self.error(OperatorError::IndeterminateEffect)),
                        };
                        if offset < self.checkpoint.state.confirmed_offset
                            || offset > size
                            || offset == size && size != 0
                        {
                            return Err(self.error(OperatorError::IndeterminateEffect));
                        }
                        let mut next = self.checkpoint.state.clone();
                        next.confirmed_offset = offset;
                        self.save(next)?;
                        self.check()?;
                        self.validate_content_source().await?;
                        fs::matches_observation(&snapshot, &staged.metadata)?;
                        let count = (size - offset).min(1024 * 1024) as usize;
                        snapshot.seek(SeekFrom::Start(offset)).map_err(fs::io)?;
                        let mut bytes = vec![0; count];
                        snapshot.read_exact(&mut bytes).map_err(fs::io)?;
                        fs::matches_observation(&snapshot, &staged.metadata)?;
                        if offset + count as u64 == size {
                            self.workspace.verify_stage(&id, &staged, &self.options)?;
                            self.validate_content_source().await?;
                            let parent = self.within_root(&parent_ref.file_id.0, true).await?;
                            if replacement.is_none()
                                && parent
                                    .capabilities
                                    .as_ref()
                                    .and_then(|c| c.can_add_children)
                                    != Some(true)
                            {
                                return Err(self.error(OperatorError::PermissionDenied));
                            }
                            if replacement.is_none() {
                                self.no_homonym(&parent_ref.file_id.0, &name).await?;
                            }
                        }
                        self.check()?;
                        let session = self
                            .access
                            .session()
                            .await
                            .map_err(|e| self.error(e.kind))?;
                        // Count attempted payload bytes separately from the
                        // cumulative server offset. Lost replies remain Unknown.
                        self.transferred = self
                            .transferred
                            .checked_add(count as u64)
                            .ok_or_else(|| self.error(OperatorError::Internal))?;
                        let chunk = g::GoogleUploadChunk {
                            start: offset,
                            bytes,
                            total_bytes: Some(size),
                        };
                        let next = if let Some((reference, conditions, _)) = &replacement {
                            let current = self.replacement_snapshot(reference, conditions).await?;
                            let etag = current
                                .etag
                                .ok_or_else(|| self.error(OperatorError::UnsupportedOperation))?;
                            self.access
                                .client
                                .upload_chunk_conditional(&session, &upload, chunk, &etag)
                                .await
                        } else {
                            self.access
                                .client
                                .upload_chunk(&session, &upload, chunk)
                                .await
                        }
                        .map_err(|e| self.provider_error(e))?;
                        if let g::GoogleUploadProgress::Incomplete {
                            received_range: range,
                        } = &next
                        {
                            let advanced = range
                                .as_ref()
                                .and_then(|r| r.end_inclusive)
                                .and_then(|n| n.checked_add(1))
                                .unwrap_or(0);
                            if advanced <= offset || advanced > offset + count as u64 {
                                return Err(self.error(OperatorError::IndeterminateEffect));
                            }
                        }
                        fs::report(
                            &self.options,
                            ExecutionPhase::Transferring,
                            self.transferred,
                            size,
                        );
                        progress = next;
                    }
                }
            }
        }
    }
    pub(super) fn reserved_id(&self) -> OperatorResult<String> {
        match &self.checkpoint.state.reserved_remote {
            Some(ResourceRef::Remote { reference, .. }) => Ok(reference.file_id.0.clone()),
            _ => Err(self.error(OperatorError::CheckpointUnavailable)),
        }
    }
    fn upload_session(&self, size: u64) -> OperatorResult<g::GoogleUploadSession> {
        let token = self
            .checkpoint
            .state
            .resume_token
            .as_ref()
            .ok_or_else(|| self.error(OperatorError::CheckpointUnavailable))?;
        let uri = std::str::from_utf8(token.as_bytes())
            .map_err(|_| self.error(OperatorError::CheckpointUnavailable))?;
        Ok(g::GoogleUploadSession {
            session_uri: uri.into(),
            total_bytes: Some(size),
        })
    }
    async fn remote_creation(&self) -> OperatorResult<Option<g::GoogleFile>> {
        if self.checkpoint.state.reserved_remote.is_none() {
            return Ok(None);
        }
        match self.get(&self.reserved_id()?).await {
            Ok(file) => Ok(Some(file)),
            Err(e) if e.kind == OperatorError::NotFoundOrInaccessible => Ok(None),
            Err(e) => Err(e),
        }
    }
    pub(super) async fn no_homonym(&self, parent: &str, name: &str) -> OperatorResult<()> {
        let reserved = self
            .checkpoint
            .state
            .reserved_remote
            .as_ref()
            .and_then(|r| match r {
                ResourceRef::Remote { reference, .. } => Some(reference.file_id.0.as_str()),
                _ => None,
            });
        self.no_homonym_for(parent, name, reserved).await
    }
    pub(super) async fn no_homonym_for(
        &self,
        parent: &str,
        name: &str,
        reserved: Option<&str>,
    ) -> OperatorResult<()> {
        let quote = |s: &str| s.replace('\\', "\\\\").replace('\'', "\\'");
        let mut cursor = None;
        let mut seen = std::collections::HashSet::new();
        for _ in 0..256 {
            self.check()?;
            let session = self
                .access
                .session()
                .await
                .map_err(|e| self.error(e.kind))?;
            let page = self
                .access
                .client
                .files_list(
                    &session,
                    g::FileListOptions {
                        q: Some(format!(
                            "'{}' in parents and trashed=false and name='{}'",
                            quote(parent),
                            quote(name)
                        )),
                        page_size: Some(100),
                        page_token: cursor,
                        supports_all_drives: Some(true),
                        include_items_from_all_drives: Some(true),
                        fields: Some(g::FieldMask(
                            "nextPageToken,incompleteSearch,files(id)".into(),
                        )),
                        ..Default::default()
                    },
                )
                .await
                .map_err(|e| self.provider_error(e))?;
            if page.incomplete_search == Some(true) {
                return Err(self.error(OperatorError::PreconditionFailed));
            }
            for file in page.files {
                if file.id.as_ref().map(|id| id.0.as_str()) != reserved || file.id.is_none() {
                    return Err(self.error(OperatorError::DestinationOccupied));
                }
            }
            match page.next_page_token {
                None => return Ok(()),
                Some(token) if !token.is_empty() && seen.insert(token.clone()) => {
                    cursor = Some(token)
                }
                _ => return Err(self.error(OperatorError::IndeterminateEffect)),
            }
        }
        Err(self.error(OperatorError::PreconditionFailed))
    }
    pub(super) fn verified_upload(
        &self,
        remote: &g::GoogleFile,
        name: &str,
        parent: &str,
        size: u64,
    ) -> OperatorResult<TransferResult> {
        let staged = self
            .checkpoint
            .state
            .staging
            .as_ref()
            .ok_or_else(|| self.error(OperatorError::CheckpointUnavailable))?;
        if remote.id.as_ref().is_none_or(|id| {
            Some(id.0.as_str())
                != self
                    .checkpoint
                    .state
                    .reserved_remote
                    .as_ref()
                    .and_then(|r| match r {
                        ResourceRef::Remote { reference, .. } => Some(reference.file_id.0.as_str()),
                        _ => None,
                    })
        }) || remote.name.as_deref() != Some(name)
            || remote
                .parents
                .as_ref()
                .is_none_or(|p| p.len() != 1 || p[0].0 != parent)
            || remote.trashed != Some(false)
            || remote.version.as_ref().is_none_or(|v| v.is_empty())
            || remote
                .mime_type
                .as_ref()
                .is_none_or(|m| m.0.starts_with("application/vnd.google-apps."))
            || remote.size.as_deref().and_then(|n| n.parse::<u64>().ok()) != Some(size)
        {
            return Err(self.error(OperatorError::PreconditionFailed));
        }
        let mut available = false;
        if let Some(sha) = &remote.sha256_checksum {
            available = true;
            if !sha.eq_ignore_ascii_case(staged.sha256.as_str()) {
                return Err(self.error(OperatorError::IntegrityMismatch));
            }
        }
        if let Some(expected) = &remote.md5_checksum {
            available = true;
            let mut snapshot = self.workspace.verify_stage(
                &self.checkpoint.intent.context.id,
                staged,
                &ExecutionOptions::default(),
            )?;
            let mut hash = Md5::new();
            let mut buffer = [0; 65536];
            let mut received = 0u64;
            loop {
                let limit = size
                    .saturating_sub(received)
                    .saturating_add(1)
                    .min(buffer.len() as u64) as usize;
                let n = snapshot.read(&mut buffer[..limit]).map_err(fs::io)?;
                if n == 0 {
                    break;
                }
                received += n as u64;
                if received > size {
                    return Err(self.error(OperatorError::IntegrityMismatch));
                }
                hash.update(&buffer[..n]);
            }
            fs::matches_observation(&snapshot, &staged.metadata)?;
            if received != size || !format!("{:x}", hash.finalize()).eq_ignore_ascii_case(expected)
            {
                return Err(self.error(OperatorError::IntegrityMismatch));
            }
        }
        if !available {
            return Err(self.error(OperatorError::IntegrityMismatch));
        }
        let value = |v: Option<String>| {
            v.map(MetadataValue::Known)
                .unwrap_or(MetadataValue::NotCollected)
        };
        let metadata = RemoteFile {
            reference: RemoteFileRef {
                account_id: self.scope.account_id.clone(),
                file_id: RemoteFileId(remote.id.as_ref().unwrap().0.clone()),
            },
            provider: ProviderId("google".into()),
            name: value(remote.name.clone()),
            kind: MetadataValue::Known(RemoteEntryKind::File),
            mime_type: value(remote.mime_type.as_ref().map(|m| m.0.clone())),
            parent_ids: MetadataValue::Known(vec![RemoteFileId(parent.into())]),
            trashed: MetadataValue::Known(false),
            version: value(remote.version.clone()),
            created_time: value(remote.created_time.clone()),
            modified_time: value(remote.modified_time.clone()),
            size: MetadataValue::Known(size),
            md5_checksum: value(remote.md5_checksum.clone()),
            sha256_checksum: value(remote.sha256_checksum.clone()),
            provider_data: MetadataValue::NotCollected,
        };
        Ok(TransferResult {
            item: ItemResult {
                resource: self.checkpoint.state.reserved_remote.clone().unwrap(),
                metadata: Some(FileData::Remote(metadata)),
                verification: VerificationStatus::Verified,
            },
            bytes_transferred: self.transferred,
            integrity: IntegrityStatus::Verified,
        })
    }
}
