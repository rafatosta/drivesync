//! Identity-bound metadata mutations, with postcondition recovery and no deletion.
use super::*;
use std::{
    ffi::OsString,
    os::unix::ffi::{OsStrExt, OsStringExt},
};
pub(super) fn local_path(
    parent: &DirectoryRef,
    n: &std::ffi::OsStr,
) -> OperatorResult<LocalRelativePath> {
    let mut bytes = match parent {
        DirectoryRef::LocalRoot => vec![],
        DirectoryRef::Local { relative_path, .. } => {
            let mut b = relative_path.as_bytes().to_vec();
            b.push(b'/');
            b
        }
        _ => return Err(fs::fail(OperatorError::WrongScope)),
    };
    bytes.extend(n.as_bytes());
    LocalRelativePath::new(bytes).map_err(|_| fs::fail(OperatorError::InvalidRequest))
}
pub(super) fn parent_ref(path: &LocalRelativePath) -> DirectoryRef {
    match path.as_bytes().iter().rposition(|b| *b == b'/') {
        Some(slash) => DirectoryRef::Local {
            relative_path: LocalRelativePath::new(path.as_bytes()[..slash].to_vec())
                .expect("validated parent"),
            local_id: None,
        },
        None => DirectoryRef::LocalRoot,
    }
}
impl Worker {
    pub(super) async fn mutate(&mut self) -> OperatorResult<ItemResult> {
        let target = match &self.checkpoint.intent.command {
            OperationCommand::Rename { target, .. }
            | OperationCommand::Move { target, .. }
            | OperationCommand::Trash { target, .. }
            | OperationCommand::Restore { target, .. } => target,
            _ => return Err(self.error(OperatorError::InvalidRequest)),
        };
        if matches!(target, ResourceRef::Local { .. }) {
            self.local_mutation()
        } else {
            self.remote_mutation().await
        }
    }
    fn local_mutation(&mut self) -> OperatorResult<ItemResult> {
        let Some(SourceEvidence::Local {
            metadata: expected, ..
        }) = self.checkpoint.intent.context.source.clone()
        else {
            return Err(self.error(OperatorError::InvalidRequest));
        };
        let command = self.checkpoint.intent.command.clone();
        let id = self.checkpoint.intent.context.id.clone();
        let original = expected.relative_path.clone();
        let restore = matches!(command, OperationCommand::Restore { .. });
        let (source_parent, old) = self.workspace.resource_parent(&original, restore)?;
        let (destination_parent, n, path, preconditions) = match command {
            OperationCommand::Rename {
                new_name,
                preconditions,
                ..
            } => {
                let n = fs::local_name(&new_name)?;
                let destination = parent_ref(&original);
                (
                    self.workspace.directory(&destination)?,
                    n.clone(),
                    local_path(&destination, &n)?,
                    preconditions,
                )
            }
            OperationCommand::Move {
                destination,
                preconditions,
                ..
            } => {
                let path = local_path(&destination, &old)?;
                if expected.kind == MetadataValue::Known(LocalEntryKind::Directory)
                    && path
                        .as_bytes()
                        .starts_with(&[original.as_bytes(), b"/"].concat())
                {
                    return Err(self.error(OperatorError::InvalidRequest));
                }
                (
                    self.workspace.directory(&destination)?,
                    old.clone(),
                    path,
                    preconditions,
                )
            }
            OperationCommand::Trash { preconditions, .. } => {
                self.workspace
                    .write_trash_manifest(&id, &expected, &source_parent)?;
                let path = LocalRelativePath::new(
                    format!(".drivesync/trash/{}/item", id.as_str()).into_bytes(),
                )
                .map_err(|_| self.error(OperatorError::InvalidRequest))?;
                (
                    self.workspace.trash_slot(&id, false)?,
                    OsString::from("item"),
                    path,
                    preconditions,
                )
            }
            OperationCommand::Restore {
                destination,
                preconditions,
                ..
            } => {
                let manifest = self.workspace.trash_manifest(&original)?;
                if manifest.original.local_id != expected.local_id {
                    return Err(self.error(OperatorError::PreconditionFailed));
                }
                let default = destination.is_none();
                let destination =
                    destination.unwrap_or_else(|| parent_ref(&manifest.original.relative_path));
                let parent = self.workspace.directory(&destination)?;
                if default && fs::identity(&parent)? != manifest.parent {
                    return Err(self.error(OperatorError::PreconditionFailed));
                }
                let name = OsString::from_vec(
                    manifest
                        .original
                        .relative_path
                        .as_bytes()
                        .rsplit(|b| *b == b'/')
                        .next()
                        .unwrap()
                        .to_vec(),
                );
                let path = local_path(&destination, &name)?;
                (parent, name, path, preconditions)
            }
            _ => return Err(self.error(OperatorError::InvalidRequest)),
        };
        self.bind_scope(&destination_parent)?;
        let destination_is_trash = path.as_bytes().starts_with(b".drivesync/trash/");
        if path == original && self.checkpoint.state.phase == CheckpointPhase::Prepared {
            let file = self
                .workspace
                .resource(&original, expected.local_id.as_ref(), restore)?;
            fs::matches_observation(&file, &expected)?;
            check_local_conditions(&expected, &preconditions)?;
        }
        if self.checkpoint.state.phase != CheckpointPhase::Prepared || path == original {
            match self
                .workspace
                .resource(&path, expected.local_id.as_ref(), destination_is_trash)
            {
                Ok(file) => {
                    self.check_local_digest(&file)?;
                    let current = fs::observe(&file, path.clone())?;
                    if expected.kind != current.kind
                        || (expected.kind == MetadataValue::Known(LocalEntryKind::Regular)
                            && (expected.storage.size_bytes != current.storage.size_bytes
                                || expected.times.modified_at != current.times.modified_at))
                    {
                        return Err(self.error(OperatorError::PreconditionFailed));
                    }
                    if path != original
                        && self
                            .workspace
                            .resource(&original, expected.local_id.as_ref(), restore)
                            .is_ok()
                    {
                        return Err(self.error(OperatorError::IndeterminateEffect));
                    }
                    return self.finish_local_item(current);
                }
                Err(e) if e.kind == OperatorError::NotFoundOrInaccessible => {}
                Err(e) => return Err(e),
            }
            if self.checkpoint.state.phase == CheckpointPhase::Completed {
                return Err(self.error(OperatorError::PreconditionFailed));
            }
        }
        let file = self
            .workspace
            .resource(&original, expected.local_id.as_ref(), restore)?;
        fs::matches_observation(&file, &expected)?;
        self.check_local_digest(&file)?;
        fs::matches_observation(&file, &expected)?;
        check_local_conditions(&expected, &preconditions)?;
        fs::Workspace::free(&destination_parent, &n)?;
        if self.checkpoint.state.phase == CheckpointPhase::Prepared {
            let mut next = self.checkpoint.state.clone();
            next.phase = CheckpointPhase::Executing;
            next.effect = ExecutionEffect::Unknown;
            self.save(next)?;
        }
        fs::report(&self.options, ExecutionPhase::Applying, 0, 0);
        self.check()?;
        fs::matches_observation(
            &self
                .workspace
                .resource(&original, expected.local_id.as_ref(), restore)?,
            &expected,
        )?;
        self.workspace.validate()?;
        if fs::identity(&self.workspace.resource_parent(&original, restore)?.0)?
            != fs::identity(&source_parent)?
            || fs::identity(
                &self
                    .workspace
                    .resource_parent(&path, destination_is_trash)?
                    .0,
            )? != fs::identity(&destination_parent)?
        {
            return Err(self.error(OperatorError::WrongScope));
        }
        if let Err(e) = self
            .workspace
            .relocate(&source_parent, &old, &destination_parent, &n)
        {
            self.publication_effect = Some(e.effect);
            return Err(e);
        }
        self.publication_effect = Some(ExecutionEffect::Applied);
        let current =
            self.workspace
                .resource(&path, expected.local_id.as_ref(), destination_is_trash)?;
        if fs::identity(&current)? != fs::identity(&file)? {
            return Err(self.error(OperatorError::IndeterminateEffect));
        }
        self.check_local_digest(&current)?;
        let current = fs::observe(&current, path)?;
        if expected.kind == MetadataValue::Known(LocalEntryKind::Regular)
            && (expected.storage.size_bytes != current.storage.size_bytes
                || expected.times.modified_at != current.times.modified_at)
        {
            return Err(self.error(OperatorError::PreconditionFailed));
        }
        self.finish_local_item(current)
    }
    fn check_local_digest(&self, file: &std::fs::File) -> OperatorResult<()> {
        if let Some(SourceEvidence::Local {
            sha256: Some(expected),
            ..
        }) = &self.checkpoint.intent.context.source
        {
            let mut file = file.try_clone().map_err(fs::io)?;
            let options = if self.publication_effect == Some(ExecutionEffect::Applied) {
                ExecutionOptions::default()
            } else {
                self.options.clone()
            };
            if fs::hash(&mut file, &options)? != *expected {
                return Err(self.error(OperatorError::IntegrityMismatch));
            }
        }
        Ok(())
    }
    fn finish_local_item(&mut self, metadata: LocalFile) -> OperatorResult<ItemResult> {
        self.finish_item(ItemResult {
            resource: ResourceRef::Local {
                relative_path: metadata.relative_path.clone(),
                local_id: metadata.local_id.clone(),
            },
            metadata: Some(FileData::Local(metadata)),
            verification: VerificationStatus::Verified,
        })
    }
    async fn remote_mutation(&mut self) -> OperatorResult<ItemResult> {
        let Some(SourceEvidence::Remote {
            metadata: expected, ..
        }) = self.checkpoint.intent.context.source.clone()
        else {
            return Err(self.error(OperatorError::InvalidRequest));
        };
        if expected.provider.0 != "google"
            || expected.reference.account_id != self.scope.account_id
            || expected.reference.file_id == self.scope.remote_root_id
        {
            return Err(self.error(OperatorError::WrongScope));
        }
        let command = self.checkpoint.intent.command.clone();
        let (conditions, destination, name, trashed) = match &command {
            OperationCommand::Rename {
                new_name,
                preconditions,
                ..
            } => {
                let ItemName::Remote(name) = new_name else {
                    return Err(self.error(OperatorError::WrongScope));
                };
                (preconditions, None, Some(name.clone()), None)
            }
            OperationCommand::Move {
                destination,
                preconditions,
                ..
            } => (preconditions, Some(destination), None, None),
            OperationCommand::Trash { preconditions, .. } => {
                (preconditions, None, None, Some(true))
            }
            OperationCommand::Restore {
                destination,
                preconditions,
                ..
            } => (preconditions, destination.as_ref(), None, Some(false)),
            _ => return Err(self.error(OperatorError::InvalidRequest)),
        };
        let session = self.access.session().await?;
        let snapshot = self
            .access
            .client
            .files_get_snapshot(
                &session,
                &g::FileId(expected.reference.file_id.0.clone()),
                g::FileGetOptions {
                    fields: Some(g::FieldMask(FIELDS.into())),
                    supports_all_drives: Some(true),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| self.provider_error(e))?;
        let actual = snapshot.file;
        let actual_kind = if actual.mime_type.as_ref().is_some_and(|m| m.0 == FOLDER) {
            RemoteEntryKind::Directory
        } else {
            RemoteEntryKind::File
        };
        if expected.kind != MetadataValue::Known(actual_kind) {
            return Err(self.error(OperatorError::PreconditionFailed));
        }
        let old_parents = match &expected.parent_ids {
            MetadataValue::Known(p) if p.len() == 1 => p,
            _ => return Err(self.error(OperatorError::PreconditionFailed)),
        };
        let new_parent = if let Some(parent) = destination {
            let DirectoryRef::Remote {
                provider_id,
                reference,
            } = parent
            else {
                return Err(self.error(OperatorError::WrongScope));
            };
            if provider_id.0 != "google" || reference.account_id != self.scope.account_id {
                return Err(self.error(OperatorError::WrongScope));
            }
            let parent = self.within_root(&reference.file_id.0, true).await?;
            if parent
                .capabilities
                .as_ref()
                .and_then(|c| c.can_add_children)
                != Some(true)
            {
                return Err(self.error(OperatorError::PermissionDenied));
            }
            reference.file_id.0.clone()
        } else {
            old_parents[0].0.clone()
        };
        self.within_root(&old_parents[0].0, true).await?;
        let desired_name = name
            .as_deref()
            .or(match &expected.name {
                MetadataValue::Known(n) => Some(n.as_str()),
                _ => None,
            })
            .ok_or_else(|| self.error(OperatorError::InvalidRequest))?;
        if desired_name.is_empty()
            || desired_name.contains(['\0', '/'])
            || matches!(desired_name, "." | ".." | ".drivesync")
            || desired_name.len() > 255
        {
            return Err(self.error(OperatorError::InvalidRequest));
        }
        if expected.kind == MetadataValue::Known(RemoteEntryKind::Directory)
            && destination.is_some()
        {
            self.no_remote_cycle(&expected.reference.file_id.0, &new_parent)
                .await?;
        }
        let desired_trashed = trashed.unwrap_or(false);
        let postcondition = |file: &g::GoogleFile| {
            file.id
                .as_ref()
                .is_some_and(|id| id.0 == expected.reference.file_id.0)
                && file.name.as_deref() == Some(desired_name)
                && file.trashed == Some(desired_trashed)
                && file
                    .parents
                    .as_ref()
                    .is_some_and(|p| p.len() == 1 && p[0].0 == new_parent)
        };
        if self.checkpoint.state.phase != CheckpointPhase::Prepared {
            if !postcondition(&actual) {
                return Err(self.error(OperatorError::IndeterminateEffect));
            }
            return self.finish_remote_item(&actual);
        }
        if actual.parents.as_ref().map(|p| {
            p.iter()
                .map(|id| RemoteFileId(id.0.clone()))
                .collect::<Vec<_>>()
        }) != Some(old_parents.clone())
            || actual.trashed
                != match expected.trashed {
                    MetadataValue::Known(value) => Some(value),
                    _ => None,
                }
        {
            return Err(self.error(OperatorError::WrongScope));
        }
        check_remote_conditions(&actual, &expected, conditions)?;
        if actual.trashed != Some(matches!(command, OperationCommand::Restore { .. })) {
            return Err(self.error(OperatorError::PreconditionFailed));
        }
        let capability = actual.capabilities.as_ref().and_then(|c| match command {
            OperationCommand::Rename { .. } => c.can_rename,
            OperationCommand::Move { .. } => c.can_move_item_within_drive,
            OperationCommand::Trash { .. } => c.can_trash,
            OperationCommand::Restore { .. } => c.can_untrash,
            _ => None,
        });
        if capability != Some(true) {
            return Err(self.error(OperatorError::PermissionDenied));
        }

        let etag = snapshot
            .etag
            .ok_or_else(|| self.error(OperatorError::UnsupportedOperation))?;
        if trashed != Some(true) {
            self.no_homonym_for(
                &new_parent,
                desired_name,
                Some(&expected.reference.file_id.0),
            )
            .await?;
        }
        let mut patch = g::GoogleFileWrite::default();
        if let Some(name) = name.as_ref() {
            patch.name = g::GooglePatch::Set(name.clone());
        }
        if let Some(value) = trashed {
            patch.trashed = g::GooglePatch::Set(value);
        }
        let mut options = g::FileUpdateOptions {
            supports_all_drives: Some(true),
            fields: Some(g::FieldMask(FIELDS.into())),
            ..Default::default()
        };
        if destination.is_some() && new_parent != old_parents[0].0 {
            options.add_parents = Some(vec![g::FileId(new_parent.clone())]);
            options.remove_parents = Some(vec![g::FileId(old_parents[0].0.clone())]);
        }
        self.check()?;
        let mut next = self.checkpoint.state.clone();
        next.phase = CheckpointPhase::Executing;
        next.effect = ExecutionEffect::Unknown;
        self.save(next)?;
        let session = self.access.session().await?;
        self.access
            .client
            .files_update_conditional(
                &session,
                &g::FileId(expected.reference.file_id.0.clone()),
                patch,
                options,
                &etag,
            )
            .await
            .map_err(|e| self.provider_error(e))?;
        self.publication_effect = Some(ExecutionEffect::Applied);
        let actual = self.get(&expected.reference.file_id.0).await?;
        if !postcondition(&actual) {
            return Err(self.error(OperatorError::PreconditionFailed));
        }
        self.finish_remote_item(&actual)
    }
    fn finish_remote_item(&mut self, file: &g::GoogleFile) -> OperatorResult<ItemResult> {
        let Some(SourceEvidence::Remote {
            metadata: expected, ..
        }) = &self.checkpoint.intent.context.source
        else {
            return Err(self.error(OperatorError::InvalidRequest));
        };
        if let MetadataValue::Known(mime) = &expected.mime_type {
            if file.mime_type.as_ref().map(|m| &m.0) != Some(mime) {
                return Err(self.error(OperatorError::PreconditionFailed));
            }
        }
        if let MetadataValue::Known(size) = expected.size {
            if file.size.as_ref().and_then(|s| s.parse::<u64>().ok()) != Some(size) {
                return Err(self.error(OperatorError::PreconditionFailed));
            }
        }
        for (expected, actual) in [
            (&expected.md5_checksum, &file.md5_checksum),
            (&expected.sha256_checksum, &file.sha256_checksum),
        ] {
            if let MetadataValue::Known(hash) = expected {
                if actual
                    .as_ref()
                    .is_none_or(|value| !value.eq_ignore_ascii_case(hash))
                {
                    return Err(self.error(OperatorError::PreconditionFailed));
                }
            }
        }
        let metadata =
            self.remote_metadata(file, file.mime_type.as_ref().is_some_and(|m| m.0 == FOLDER))?;
        self.finish_item(ItemResult {
            resource: ResourceRef::Remote {
                provider_id: ProviderId("google".into()),
                reference: metadata.reference.clone(),
            },
            metadata: Some(FileData::Remote(metadata)),
            verification: VerificationStatus::Verified,
        })
    }
    async fn no_remote_cycle(&self, target: &str, parent: &str) -> OperatorResult<()> {
        let mut current = parent.to_owned();
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..256 {
            if current == target || !seen.insert(current.clone()) {
                return Err(self.error(OperatorError::InvalidRequest));
            }
            if current == self.scope.remote_root_id.0 {
                return Ok(());
            }
            let file = self.within_root(&current, true).await?;
            let parents = file
                .parents
                .filter(|p| p.len() == 1)
                .ok_or_else(|| self.error(OperatorError::WrongScope))?;
            current = parents[0].0.clone();
        }
        Err(self.error(OperatorError::WrongScope))
    }
}
pub(super) fn check_local_conditions(
    expected: &LocalFile,
    p: &Preconditions,
) -> OperatorResult<()> {
    if p.expected_version.is_some()
        || p.expected_revision
            .as_ref()
            .is_some_and(|v| local_revision(expected).as_ref() != Ok(v))
    {
        return Err(fs::fail(OperatorError::PreconditionFailed));
    }
    Ok(())
}
pub(super) fn check_remote_conditions(
    actual: &g::GoogleFile,
    expected: &RemoteFile,
    p: &Preconditions,
) -> OperatorResult<()> {
    let valid = actual
        .id
        .as_ref()
        .is_some_and(|id| id.0 == expected.reference.file_id.0)
        && actual
            .version
            .as_ref()
            .is_some_and(|v| expected.version == MetadataValue::Known(v.clone()))
        && p.expected_version
            .as_ref()
            .is_none_or(|v| actual.version.as_ref() == Some(v))
        && p.expected_revision.as_ref().is_none_or(|r| {
            actual
                .head_revision_id
                .as_ref()
                .is_some_and(|id| &id.0 == r)
        });
    if !valid {
        return Err(fs::fail(OperatorError::PreconditionFailed));
    }
    Ok(())
}
