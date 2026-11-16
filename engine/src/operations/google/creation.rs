//! Read-only creation prerequisites and recoverable directory replicas.
use super::*;
use std::os::unix::ffi::OsStrExt;
impl GoogleOperator {
    pub(super) fn probe(
        &self,
        r: VerifyCreationRequest,
    ) -> OperationFuture<'_, VerifyCreationResult> {
        let scope = self.scope.clone();
        let host = self.host.clone();
        let journal = self.journal.clone();
        let runtime = host.runtime();
        Box::pin(async move {
            let handle = runtime.clone();
            runtime
                .spawn_blocking(move || {
                    handle.block_on(async move {
                        fs::check(&r.options)?;
                        let workspace = fs::Workspace::open(&scope.local_root)?;
                        let access = host.access(scope.account_id.clone()).await?;
                        let intent = ExecutionIntent {
                            context: r.context,
                            scope: ExecutionScope::from(&scope),
                            command: r.command.clone(),
                        };
                        let worker = Worker {
                            scope,
                            workspace,
                            access,
                            journal,
                            checkpoint: ExecutionCheckpoint {
                                intent,
                                revision: 0,
                                state: ExecutionState::prepared(),
                            },
                            options: r.options,
                            transferred: 0,
                            publication_effect: None,
                        };
                        let source = worker
                            .checkpoint
                            .intent
                            .context
                            .source
                            .as_ref()
                            .map(|s| worker.inspect_source(s));
                        let source = if let Some(future) = source {
                            Some(future.await?)
                        } else {
                            None
                        };
                        match &r.command {
                            OperationCommand::Upload {
                                destination, name, ..
                            }
                            | OperationCommand::CreateDirectory {
                                parent: destination,
                                name,
                            } => worker.free_destination(destination, name).await?,
                            OperationCommand::Download {
                                destination, name, ..
                            } => worker.free_destination(destination, name).await?,
                            // Internal source-only inspection has no destination effect.
                            OperationCommand::CreateFile { content: None, .. } => {}
                            _ => return Err(worker.error(OperatorError::UnsupportedOperation)),
                        }
                        Ok(VerifyCreationResult {
                            command: r.command,
                            source,
                        })
                    })
                })
                .await
                .map_err(|_| fs::fail(OperatorError::Internal))?
        })
    }
}
impl Worker {
    async fn free_destination(&self, parent: &DirectoryRef, n: &ItemName) -> OperatorResult<()> {
        self.check()?;
        match parent {
            DirectoryRef::Remote {
                provider_id,
                reference,
            } => {
                if provider_id.0 != "google" || reference.account_id != self.scope.account_id {
                    return Err(self.error(OperatorError::WrongScope));
                }
                let name = remote_name(n)?;
                let folder = self.within_root(&reference.file_id.0, true).await?;
                if folder
                    .capabilities
                    .as_ref()
                    .and_then(|c| c.can_add_children)
                    != Some(true)
                {
                    return Err(self.error(OperatorError::PermissionDenied));
                }
                self.no_homonym(&reference.file_id.0, &name).await
            }
            _ => {
                let parent = self.workspace.directory(parent)?;
                fs::Workspace::free(&parent, &fs::local_name(n)?)
            }
        }
    }
    pub(super) async fn inspect_source(
        &self,
        source: &SourceEvidence,
    ) -> OperatorResult<VerifiedSource> {
        self.check()?;
        match source {
            SourceEvidence::Local { metadata, sha256 } => {
                let mut file = if metadata.kind == MetadataValue::Known(LocalEntryKind::Directory) {
                    self.workspace.directory(&DirectoryRef::Local {
                        relative_path: metadata.relative_path.clone(),
                        local_id: metadata.local_id.clone(),
                    })?
                } else {
                    self.workspace.source(metadata)?
                };
                fs::matches_observation(&file, metadata)?;
                let hash = if metadata.kind == MetadataValue::Known(LocalEntryKind::Regular) {
                    Some(fs::hash(&mut file, &self.options)?)
                } else {
                    None
                };
                if sha256
                    .as_ref()
                    .is_some_and(|expected| hash.as_ref() != Some(expected))
                {
                    return Err(self.error(OperatorError::IntegrityMismatch));
                }
                fs::matches_observation(&file, metadata)?;
                Ok(VerifiedSource {
                    metadata: FileData::Local(fs::observe(&file, metadata.relative_path.clone())?),
                    sha256: hash,
                })
            }
            SourceEvidence::Remote { metadata, revision } => {
                if metadata.provider.0 != "google"
                    || metadata.reference.account_id != self.scope.account_id
                {
                    return Err(self.error(OperatorError::WrongScope));
                }
                let directory = metadata.kind == MetadataValue::Known(RemoteEntryKind::Directory);
                let actual = self
                    .within_root(&metadata.reference.file_id.0, directory)
                    .await?;
                if !directory {
                    self.check_remote_source(&actual, metadata)?;
                } else if actual
                    .version
                    .as_ref()
                    .map(|v| MetadataValue::Known(v.clone()))
                    != Some(metadata.version.clone())
                    || directory
                        && (actual
                            .name
                            .as_ref()
                            .map(|n| MetadataValue::Known(n.clone()))
                            != Some(metadata.name.clone())
                            || actual.parents.as_ref().map(|p| {
                                MetadataValue::Known(
                                    p.iter()
                                        .map(|id| RemoteFileId(id.0.clone()))
                                        .collect::<Vec<_>>(),
                                )
                            }) != Some(metadata.parent_ids.clone()))
                {
                    return Err(self.error(OperatorError::PreconditionFailed));
                }
                if revision
                    .as_ref()
                    .is_some_and(|r| actual.head_revision_id.as_ref().map(|id| &id.0) != Some(r))
                {
                    return Err(self.error(OperatorError::PreconditionFailed));
                }
                Ok(VerifiedSource {
                    metadata: FileData::Remote(self.remote_metadata(&actual, directory)?),
                    sha256: None,
                })
            }
        }
    }
    pub(super) fn remote_metadata(
        &self,
        actual: &g::GoogleFile,
        directory: bool,
    ) -> OperatorResult<RemoteFile> {
        let value = |v: Option<String>| {
            v.map(MetadataValue::Known)
                .unwrap_or(MetadataValue::NotCollected)
        };
        let remote = RemoteFile {
            reference: RemoteFileRef {
                account_id: self.scope.account_id.clone(),
                file_id: RemoteFileId(
                    actual
                        .id
                        .as_ref()
                        .ok_or_else(|| self.error(OperatorError::IndeterminateEffect))?
                        .0
                        .clone(),
                ),
            },
            provider: ProviderId("google".into()),
            name: value(actual.name.clone()),
            kind: MetadataValue::Known(if directory {
                RemoteEntryKind::Directory
            } else {
                RemoteEntryKind::File
            }),
            mime_type: value(actual.mime_type.as_ref().map(|m| m.0.clone())),
            parent_ids: actual
                .parents
                .as_ref()
                .map(|p| {
                    MetadataValue::Known(p.iter().map(|id| RemoteFileId(id.0.clone())).collect())
                })
                .unwrap_or(MetadataValue::NotCollected),
            trashed: actual
                .trashed
                .map(MetadataValue::Known)
                .unwrap_or(MetadataValue::NotCollected),
            version: value(actual.version.clone()),
            created_time: value(actual.created_time.clone()),
            modified_time: value(actual.modified_time.clone()),
            size: actual
                .size
                .as_ref()
                .and_then(|s| s.parse().ok())
                .map(MetadataValue::Known)
                .unwrap_or(MetadataValue::NotCollected),
            md5_checksum: value(actual.md5_checksum.clone()),
            sha256_checksum: value(actual.sha256_checksum.clone()),
            provider_data: MetadataValue::NotCollected,
        };
        remote
            .validate()
            .map_err(|_| self.error(OperatorError::IndeterminateEffect))?;
        Ok(remote)
    }
    pub(super) async fn create_directory(&mut self) -> OperatorResult<ItemResult> {
        let OperationCommand::CreateDirectory { parent, name } =
            self.checkpoint.intent.command.clone()
        else {
            return Err(self.error(OperatorError::InvalidRequest));
        };
        let id = self.checkpoint.intent.context.id.clone();
        if let DirectoryRef::Remote {
            provider_id,
            reference,
        } = &parent
        {
            let name = remote_name(&name)?;
            if provider_id.0 != "google" || reference.account_id != self.scope.account_id {
                return Err(self.error(OperatorError::WrongScope));
            }
            let parent_file = self.within_root(&reference.file_id.0, true).await?;
            if parent_file
                .capabilities
                .as_ref()
                .and_then(|c| c.can_add_children)
                != Some(true)
            {
                return Err(self.error(OperatorError::PermissionDenied));
            }
            if self.checkpoint.state.reserved_remote.is_none() {
                self.free_destination(&parent, &ItemName::Remote(name.clone()))
                    .await?;
                let session = self.access.session().await?;
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
                    .filter(|v| v.len() == 1)
                    .ok_or_else(|| self.error(OperatorError::IndeterminateEffect))?;
                let mut next = self.checkpoint.state.clone();
                next.reserved_remote = Some(ResourceRef::Remote {
                    provider_id: provider_id.clone(),
                    reference: RemoteFileRef {
                        account_id: self.scope.account_id.clone(),
                        file_id: RemoteFileId(ids[0].0.clone()),
                    },
                });
                self.save(next)?;
            }
            let remote_id = self.reserved_id()?;
            let actual = match self.get(&remote_id).await {
                Ok(file) if self.checkpoint.state.phase != CheckpointPhase::Prepared => file,
                Ok(_) => return Err(self.error(OperatorError::DestinationOccupied)),
                Err(e)
                    if e.kind == OperatorError::NotFoundOrInaccessible
                        && self.checkpoint.state.phase != CheckpointPhase::Completed =>
                {
                    self.free_destination(&parent, &ItemName::Remote(name.clone()))
                        .await?;
                    if let Some(source) = &self.checkpoint.intent.context.source {
                        self.inspect_source(source).await?;
                    }
                    let mut next = self.checkpoint.state.clone();
                    next.phase = CheckpointPhase::Executing;
                    next.effect = ExecutionEffect::Unknown;
                    self.save(next)?;
                    let session = self.access.session().await?;
                    self.access
                        .client
                        .files_create(
                            &session,
                            g::GoogleFileWrite {
                                id: Some(g::FileId(remote_id.clone())),
                                name: g::GooglePatch::Set(name.clone()),
                                parents: g::GooglePatch::Set(vec![g::FileId(
                                    reference.file_id.0.clone(),
                                )]),
                                mime_type: g::GooglePatch::Set(g::MimeType(FOLDER.into())),
                                ..Default::default()
                            },
                            g::FileCreateOptions {
                                supports_all_drives: Some(true),
                                fields: Some(g::FieldMask(FIELDS.into())),
                                ..Default::default()
                            },
                        )
                        .await
                        .map_err(|e| self.provider_error(e))?;
                    self.get(&remote_id).await?
                }
                Err(e) => return Err(e),
            };
            if actual.id.as_ref().map(|id| &id.0) != Some(&remote_id)
                || actual.name.as_deref() != Some(name.as_str())
                || actual
                    .parents
                    .as_ref()
                    .is_none_or(|p| p.len() != 1 || p[0].0 != reference.file_id.0)
                || actual.trashed != Some(false)
                || actual.mime_type.as_ref().map(|m| m.0.as_str()) != Some(FOLDER)
            {
                return Err(self.error(OperatorError::PreconditionFailed));
            }
            let result = ItemResult {
                resource: self.checkpoint.state.reserved_remote.clone().unwrap(),
                metadata: Some(FileData::Remote(self.remote_metadata(&actual, true)?)),
                verification: VerificationStatus::Verified,
            };
            return self.finish_item(result);
        }
        let n = fs::local_name(&name)?;
        let directory = self.workspace.directory(&parent)?;
        let mut path = match &parent {
            DirectoryRef::LocalRoot => vec![],
            DirectoryRef::Local { relative_path, .. } => {
                let mut p = relative_path.as_bytes().to_vec();
                p.push(b'/');
                p
            }
            _ => return Err(self.error(OperatorError::InvalidRequest)),
        };
        path.extend(n.as_bytes());
        let path =
            LocalRelativePath::new(path).map_err(|_| self.error(OperatorError::InvalidRequest))?;
        let target = DirectoryRef::Local {
            relative_path: path.clone(),
            local_id: None,
        };
        if self.checkpoint.state.local_scope.is_some() {
            if let Ok(file) = self.workspace.directory(&target) {
                let scope = self.checkpoint.state.local_scope.as_ref().unwrap();
                if self.checkpoint.state.phase == CheckpointPhase::Prepared
                    || fs::identity(&file)? != scope.staging_directory
                    || fs::identity(&directory)? != scope.parent
                    || fs::identity(&self.workspace.root)? != scope.root
                {
                    return Err(self.error(OperatorError::DestinationOccupied));
                }
                let observed = fs::observe(&file, path.clone())?;
                return self.finish_item(ItemResult {
                    resource: ResourceRef::Local {
                        relative_path: path,
                        local_id: observed.local_id.clone(),
                    },
                    metadata: Some(FileData::Local(observed)),
                    verification: VerificationStatus::Verified,
                });
            }
            if self.checkpoint.state.phase == CheckpointPhase::Completed {
                return Err(self.error(OperatorError::PreconditionFailed));
            }
        }
        fs::Workspace::free(&directory, &n)?;
        if let Some(source) = &self.checkpoint.intent.context.source {
            self.inspect_source(source).await?;
        }
        let staged = self.workspace.directory_stage(
            &id,
            self.checkpoint.state.phase == CheckpointPhase::Prepared,
        )?;
        let mut scope = self.workspace.scope(&directory)?;
        scope.staging_directory = fs::identity(&staged)?;
        if self
            .checkpoint
            .state
            .local_scope
            .as_ref()
            .is_some_and(|s| *s != scope)
        {
            return Err(self.error(OperatorError::WrongScope));
        }
        if self.checkpoint.state.local_scope.is_none() {
            let mut next = self.checkpoint.state.clone();
            next.local_scope = Some(scope);
            self.save(next)?;
        }
        if self.checkpoint.state.phase == CheckpointPhase::Prepared {
            let mut next = self.checkpoint.state.clone();
            next.phase = CheckpointPhase::Executing;
            next.effect = ExecutionEffect::Unknown;
            self.save(next)?;
        }
        if self.checkpoint.state.phase != CheckpointPhase::Verifying {
            let mut next = self.checkpoint.state.clone();
            next.phase = CheckpointPhase::Verifying;
            next.effect = ExecutionEffect::Unknown;
            self.save(next)?;
        }
        self.check()?;
        if fs::identity(&self.workspace.directory(&parent)?)? != fs::identity(&directory)? {
            return Err(self.error(OperatorError::WrongScope));
        }
        self.workspace.publish_directory(&id, &directory, &n)?;
        self.publication_effect = Some(ExecutionEffect::Applied);
        let file = self.workspace.directory(&target)?;
        let observed = fs::observe(&file, path.clone())?;
        self.finish_item(ItemResult {
            resource: ResourceRef::Local {
                relative_path: path,
                local_id: observed.local_id.clone(),
            },
            metadata: Some(FileData::Local(observed)),
            verification: VerificationStatus::Verified,
        })
    }
    pub(super) fn finish_item(&mut self, result: ItemResult) -> OperatorResult<ItemResult> {
        if self.checkpoint.state.phase == CheckpointPhase::Completed {
            return Ok(result);
        }
        self.publication_effect = Some(ExecutionEffect::Applied);
        if self.checkpoint.state.phase == CheckpointPhase::Prepared {
            let mut next = self.checkpoint.state.clone();
            next.phase = CheckpointPhase::Executing;
            next.effect = ExecutionEffect::Unknown;
            self.save(next)?;
        }
        if self.checkpoint.state.phase != CheckpointPhase::Verifying {
            let mut next = self.checkpoint.state.clone();
            next.phase = CheckpointPhase::Verifying;
            next.effect = ExecutionEffect::Applied;
            self.save(next)?;
        }
        let mut next = self.checkpoint.state.clone();
        next.phase = CheckpointPhase::Completed;
        next.effect = ExecutionEffect::Applied;
        next.result = Some(ExecutionResult::Item(result.clone()));
        self.save(next)?;
        Ok(result)
    }
}
fn remote_name(name: &ItemName) -> OperatorResult<String> {
    match name {
        ItemName::Remote(n)
            if !n.is_empty()
                && n.len() <= 255
                && !n.contains(['\0', '/'])
                && !matches!(n.as_str(), "." | ".." | ".drivesync") =>
        {
            Ok(n.clone())
        }
        _ => Err(fs::fail(OperatorError::InvalidRequest)),
    }
}
