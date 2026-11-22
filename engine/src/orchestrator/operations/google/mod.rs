//! Google specialization using the preserved provider API and private Accounts sessions.
use super::{
    filesystem::{self, Filesystem},
    interfaces::{EffectFuture, RemoteExecutor},
    models::*,
};
use crate::{
    accounts::{Accounts, AcquireSessionRequest},
    inventory::models::RemoteState,
    models::*,
    providers::google::*,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    io::{Read, Write},
};
const FIELDS: &str =
    "id,name,mimeType,parents,size,sha256Checksum,version,trashed,capabilities,webViewLink";
const FOLDER: &str = "application/vnd.google-apps.folder";
pub(super) struct GoogleOperator {
    accounts: Accounts,
    client: GoogleDriveClient,
}
impl GoogleOperator {
    pub fn new(accounts: Accounts, client: GoogleDriveClient) -> Self {
        Self { accounts, client }
    }
    async fn snapshot(
        &self,
        session: &GoogleSession,
        id: &RemoteId,
    ) -> Result<GoogleFileSnapshot, OperatorError> {
        let snapshot = self
            .client
            .files_get_snapshot(
                session,
                &FileId(id.0.clone()),
                FileGetOptions {
                    fields: Some(FieldMask(FIELDS.into())),
                    supports_all_drives: Some(true),
                    ..Default::default()
                },
            )
            .await
            .map_err(read_error)?;
        if snapshot.file.id.as_ref().map(|actual| actual.0.as_str()) != Some(id.0.as_str()) {
            return Err(filesystem::stale());
        }
        Ok(snapshot)
    }
    /// Follow actual parent identities, never names, with bounded cycle rejection.
    async fn within(
        &self,
        session: &GoogleSession,
        id: &RemoteId,
        root: &RemoteId,
    ) -> Result<(), OperatorError> {
        let mut next = id.clone();
        let mut visited = BTreeSet::new();
        for _ in 0..128 {
            if &next == root {
                return Ok(());
            }
            if !visited.insert(next.clone()) {
                return Err(filesystem::stale());
            }
            let file = self.snapshot(session, &next).await?.file;
            if &next != id && file.trashed != Some(false) {
                return Err(filesystem::stale());
            }
            let parents = file.parents.ok_or_else(filesystem::stale)?;
            if parents.len() != 1 {
                return Err(filesystem::stale());
            }
            next = RemoteId(parents[0].0.clone());
        }
        Err(filesystem::stale())
    }
    async fn destination(
        &self,
        session: &GoogleSession,
        config: &OperatorConfig,
        parent: &RemoteId,
        name: &str,
        except: Option<&RemoteId>,
    ) -> Result<(), OperatorError> {
        self.check_destination(session, config, parent, name, except, true)
            .await
    }
    async fn check_destination(
        &self,
        session: &GoogleSession,
        config: &OperatorConfig,
        parent: &RemoteId,
        name: &str,
        except: Option<&RemoteId>,
        requires_write: bool,
    ) -> Result<(), OperatorError> {
        self.within(session, parent, &config.remote_root_id).await?;
        let parent_file = self.snapshot(session, parent).await?.file;
        if parent_file.mime_type.as_ref().map(|m| m.0.as_str()) != Some(FOLDER)
            || parent_file.trashed != Some(false)
            || (requires_write
                && !parent_file
                    .capabilities
                    .as_ref()
                    .is_some_and(|c| c.can_add_children == Some(true)))
            || (!requires_write
                && !parent_file
                    .capabilities
                    .as_ref()
                    .is_some_and(|c| c.can_list_children == Some(true)))
        {
            return Err(filesystem::stale());
        }
        let quote = |s: &str| s.replace('\\', "\\\\").replace('\'', "\\'");
        let matches = self
            .client
            .files_list(
                session,
                FileListOptions {
                    q: Some(format!(
                        "'{}' in parents and name = '{}' and trashed = false",
                        quote(&parent.0),
                        quote(name)
                    )),
                    fields: Some(FieldMask("files(id),nextPageToken,incompleteSearch".into())),
                    page_size: Some(100),
                    supports_all_drives: Some(true),
                    include_items_from_all_drives: Some(true),
                    ..Default::default()
                },
            )
            .await
            .map_err(read_error)?;
        if matches.incomplete_search != Some(false)
            || (!requires_write && except.is_some() && matches.files.len() != 1)
            || matches.next_page_token.is_some()
            || matches
                .files
                .iter()
                .any(|f| f.id.as_ref().map(|id| RemoteId(id.0.clone())).as_ref() != except)
        {
            return Err(filesystem::stale());
        }
        Ok(())
    }
    async fn run(
        &self,
        config: &OperatorConfig,
        r: &ExecuteOperationRequest,
        fs: &Filesystem,
        session: &GoogleSession,
    ) -> Result<AppliedEffect, OperatorError> {
        let op = &r.operation;
        let pre = op.preconditions.as_ref().ok_or_else(filesystem::stale)?;
        let snapshot = if let Some(id) = &op.remote_id {
            self.within(session, id, &config.remote_root_id).await?;
            let snapshot = self.snapshot(session, id).await?;
            if snapshot.file.id.as_ref().map(|v| &v.0) != Some(&id.0)
                || snapshot
                    .file
                    .version
                    .as_ref()
                    .and_then(|v| v.parse::<u64>().ok())
                    != pre.observed_remote_version
                || pre.observed_remote_version.is_none()
            {
                return Err(filesystem::stale());
            }
            Some(snapshot)
        } else {
            None
        };
        let mut local = if r.local.state == FileState::Present {
            Some(fs.check(&r.local)?)
        } else {
            None
        };
        if r.local.state == FileState::Absent {
            let path = op
                .local_path
                .as_deref()
                .or(r.local.path.as_deref())
                .ok_or_else(filesystem::stale)?;
            fs.check_absent(path)?;
            local = Some(r.local.clone());
        }
        if let Some(snapshot) = &snapshot {
            if op.action != OperationAction::RemoveLocal && snapshot.file.trashed != Some(false) {
                return Err(filesystem::stale());
            }
        }
        let remote = match op.action {
            OperationAction::RegisterIdentity
            | OperationAction::Adopt
            | OperationAction::ConfirmBaseline => {
                if op.action == OperationAction::RegisterIdentity
                    && r.local.state == FileState::Present
                    && r.remote.state == FileState::Absent
                {
                    let parent = op.parent_id.as_ref().ok_or_else(filesystem::stale)?;
                    let name = match &r.local.name {
                        Evidence::Known(name) => name,
                        _ => return Err(filesystem::stale()),
                    };
                    self.check_destination(session, config, parent, name, None, false)
                        .await?;
                    return Ok(AppliedEffect {
                        local: Some(fs.check(&r.local)?),
                        remote: Some(r.remote.clone()),
                        remote_id: None,
                        verified: true,
                    });
                }
                let before = snapshot.as_ref().ok_or_else(filesystem::stale)?;
                if matches!(
                    op.action,
                    OperationAction::Adopt | OperationAction::ConfirmBaseline
                ) {
                    let parents = before
                        .file
                        .parents
                        .as_ref()
                        .filter(|parents| parents.len() == 1)
                        .ok_or_else(filesystem::stale)?;
                    let name = before.file.name.as_deref().ok_or_else(filesystem::stale)?;
                    self.check_destination(
                        session,
                        config,
                        &RemoteId(parents[0].0.clone()),
                        name,
                        op.remote_id.as_ref(),
                        false,
                    )
                    .await?;
                }
                let after = self
                    .snapshot(
                        session,
                        op.remote_id.as_ref().ok_or_else(filesystem::stale)?,
                    )
                    .await?;
                if before.file.version != after.file.version {
                    return Err(filesystem::stale());
                }
                return Ok(AppliedEffect {
                    local,
                    remote: Some(normalize(&after.file)),
                    remote_id: op.remote_id.clone(),
                    verified: true,
                });
            }
            OperationAction::RenameLocal
            | OperationAction::MoveLocal
            | OperationAction::RemoveLocal
            | OperationAction::CreateLocalDirectory => {
                let source = snapshot.as_ref().ok_or_else(filesystem::stale)?;
                if matches!(op.action, OperationAction::RemoveLocal) {
                    if source.file.trashed != Some(true) {
                        return Err(filesystem::stale());
                    }
                } else if source.file.trashed != Some(false) {
                    return Err(filesystem::stale());
                }
                if op.action == OperationAction::CreateLocalDirectory
                    && source.file.mime_type.as_ref().map(|m| m.0.as_str()) != Some(FOLDER)
                {
                    return Err(filesystem::stale());
                }
                let mut effect = fs.execute(r)?;
                effect.remote = Some(normalize(&source.file));
                return Ok(effect);
            }
            OperationAction::Upload | OperationAction::CreateRemoteDirectory => {
                if local.is_none() || op.remote_id.is_some() || r.remote.state != FileState::Absent
                {
                    return Err(filesystem::stale());
                }
                let parent = op.parent_id.as_ref().ok_or_else(filesystem::invalid)?;
                let name = op.name.as_deref().ok_or_else(filesystem::invalid)?;
                self.destination(session, config, parent, name, None)
                    .await?;
                let metadata = GoogleFileWrite {
                    name: GooglePatch::Set(name.into()),
                    parents: GooglePatch::Set(vec![FileId(parent.0.clone())]),
                    mime_type: if op.action == OperationAction::CreateRemoteDirectory {
                        GooglePatch::Set(MimeType(FOLDER.into()))
                    } else {
                        GooglePatch::Unchanged
                    },
                    ..Default::default()
                };
                let file = if op.action == OperationAction::CreateRemoteDirectory {
                    if r.local.kind != Evidence::Known(FileKind::Directory) {
                        return Err(filesystem::stale());
                    }
                    self.client
                        .files_create(
                            session,
                            metadata,
                            FileCreateOptions {
                                fields: Some(FieldMask(FIELDS.into())),
                                supports_all_drives: Some(true),
                                ..Default::default()
                            },
                        )
                        .await
                        .map_err(mutation_error)?
                } else {
                    self.upload(session, r, fs, metadata, None).await?
                };
                let id = file
                    .id
                    .as_ref()
                    .map(|id| RemoteId(id.0.clone()))
                    .ok_or_else(|| filesystem::stale().uncertain())?;
                let observed = self
                    .snapshot(session, &id)
                    .await
                    .map_err(OperatorError::uncertain)?
                    .file;
                if observed.name.as_deref() != Some(name)
                    || observed.parents.as_ref() != Some(&vec![FileId(parent.0.clone())])
                    || observed.trashed != Some(false)
                    || (op.action == OperationAction::CreateRemoteDirectory
                        && observed.mime_type.as_ref().map(|m| m.0.as_str()) != Some(FOLDER))
                {
                    return Err(filesystem::stale().uncertain());
                }
                observed
            }
            OperationAction::RenameRemote
            | OperationAction::MoveRemote
            | OperationAction::TrashRemote => {
                let snapshot = snapshot.as_ref().ok_or_else(filesystem::stale)?;
                let id = op.remote_id.as_ref().ok_or_else(filesystem::invalid)?;
                let etag = snapshot.etag.as_deref().ok_or_else(filesystem::stale)?;
                let caps = snapshot
                    .file
                    .capabilities
                    .as_ref()
                    .ok_or_else(filesystem::stale)?;
                let mut metadata = GoogleFileWrite::default();
                let mut options = FileUpdateOptions {
                    supports_all_drives: Some(true),
                    fields: Some(FieldMask(FIELDS.into())),
                    ..Default::default()
                };
                match op.action {
                    OperationAction::RenameRemote => {
                        if caps.can_rename != Some(true) {
                            return Err(filesystem::stale());
                        }
                        let name = op.name.as_deref().ok_or_else(filesystem::invalid)?;
                        let parents = snapshot
                            .file
                            .parents
                            .as_ref()
                            .filter(|p| p.len() == 1)
                            .ok_or_else(filesystem::stale)?;
                        self.destination(
                            session,
                            config,
                            &RemoteId(parents[0].0.clone()),
                            name,
                            Some(id),
                        )
                        .await?;
                        metadata.name = GooglePatch::Set(name.into());
                    }
                    OperationAction::MoveRemote => {
                        if caps.can_move_item_within_drive != Some(true) {
                            return Err(filesystem::stale());
                        }
                        let parent = op.parent_id.as_ref().ok_or_else(filesystem::invalid)?;
                        let name = snapshot
                            .file
                            .name
                            .as_deref()
                            .ok_or_else(filesystem::stale)?;
                        self.destination(session, config, parent, name, Some(id))
                            .await?;
                        let parents = snapshot
                            .file
                            .parents
                            .as_ref()
                            .filter(|p| p.len() == 1)
                            .ok_or_else(filesystem::stale)?;
                        options.add_parents = Some(vec![FileId(parent.0.clone())]);
                        options.remove_parents = Some(vec![parents[0].clone()]);
                    }
                    _ => {
                        if caps.can_trash != Some(true) {
                            return Err(filesystem::stale());
                        }
                        metadata.trashed = GooglePatch::Set(true);
                    }
                }
                self.client
                    .files_update_conditional(
                        session,
                        &FileId(id.0.clone()),
                        metadata,
                        options,
                        etag,
                    )
                    .await
                    .map_err(mutation_error)?;
                let after = self
                    .snapshot(session, id)
                    .await
                    .map_err(OperatorError::uncertain)?
                    .file;
                let verified = match op.action {
                    OperationAction::RenameRemote => after.name == op.name,
                    OperationAction::MoveRemote => {
                        after.parents.as_ref()
                            == op
                                .parent_id
                                .as_ref()
                                .map(|id| vec![FileId(id.0.clone())])
                                .as_ref()
                    }
                    _ => after.trashed == Some(true),
                };
                if !verified {
                    return Err(filesystem::stale().uncertain());
                }
                after
            }
            OperationAction::UpdateRemoteContent => {
                let snapshot = snapshot.as_ref().ok_or_else(filesystem::stale)?;
                if !snapshot
                    .file
                    .capabilities
                    .as_ref()
                    .is_some_and(|c| c.can_edit == Some(true))
                {
                    return Err(filesystem::stale());
                }
                self.upload(
                    session,
                    r,
                    fs,
                    GoogleFileWrite::default(),
                    snapshot.etag.as_deref(),
                )
                .await?;
                self.snapshot(session, op.remote_id.as_ref().unwrap())
                    .await
                    .map_err(OperatorError::uncertain)?
                    .file
            }
            OperationAction::Download | OperationAction::UpdateLocalContent => {
                let snapshot = snapshot.as_ref().ok_or_else(filesystem::stale)?;
                if !snapshot
                    .file
                    .capabilities
                    .as_ref()
                    .is_some_and(|c| c.can_download == Some(true))
                    || snapshot.file.trashed != Some(false)
                {
                    return Err(filesystem::stale());
                }
                let expected = snapshot
                    .file
                    .sha256_checksum
                    .as_deref()
                    .ok_or_else(filesystem::stale)?;
                let size = snapshot
                    .file
                    .size
                    .as_ref()
                    .and_then(|s| s.parse::<u64>().ok())
                    .ok_or_else(filesystem::stale)?;
                let mut stream = self
                    .client
                    .get_file_bytes(
                        session,
                        &FileId(op.remote_id.as_ref().unwrap().0.clone()),
                        None,
                    )
                    .await
                    .map_err(read_error)?;
                let mut temp = fs.temporary()?;
                let mut digest = Sha256::new();
                let mut count = 0u64;
                while let Some(bytes) = stream.body.next_chunk().await.map_err(read_error)? {
                    count = count
                        .checked_add(bytes.len() as u64)
                        .ok_or_else(filesystem::stale)?;
                    if count > size {
                        return Err(filesystem::stale());
                    }
                    digest.update(&bytes);
                    temp.write_all(&bytes).map_err(filesystem::error)?;
                }
                if count != size || format!("{:x}", digest.finalize()) != expected {
                    return Err(filesystem::stale());
                }
                let after = self
                    .snapshot(session, op.remote_id.as_ref().unwrap())
                    .await?;
                if after.file.version != snapshot.file.version {
                    return Err(filesystem::stale());
                }
                local = Some(fs.install(r, &mut temp)?);
                after.file
            }
            OperationAction::CreateLocalShortcut => {
                let snapshot = snapshot.as_ref().ok_or_else(filesystem::stale)?;
                if !snapshot.file.mime_type.as_ref().is_some_and(|m| {
                    m.0.starts_with("application/vnd.google-apps.") && m.0 != FOLDER
                }) || snapshot.file.trashed != Some(false)
                {
                    return Err(filesystem::stale());
                }
                let id = op.remote_id.as_ref().unwrap();
                // An inert desktop link, with no executable command and no provider-controlled URL.
                let encoded: String =
                    id.0.bytes()
                        .map(|b| {
                            if b.is_ascii_alphanumeric() || b"-_".contains(&b) {
                                (b as char).to_string()
                            } else {
                                format!("%{b:02X}")
                            }
                        })
                        .collect();
                let mut temp = fs.temporary()?;
                write!(temp,"[Desktop Entry]\nType=Link\nName=Google Drive item\nURL=https://drive.google.com/open?id={encoded}\n").map_err(filesystem::error)?;
                local = Some(fs.install(r, &mut temp)?);
                snapshot.file.clone()
            }
        };
        if r.local.state == FileState::Present
            && !matches!(
                op.action,
                OperationAction::Download
                    | OperationAction::UpdateLocalContent
                    | OperationAction::CreateLocalShortcut
            )
        {
            local = Some(fs.check(&r.local).map_err(OperatorError::uncertain)?);
        }
        let remote = normalize(&remote);
        let mut verified = true;
        if matches!(
            op.action,
            OperationAction::Upload | OperationAction::UpdateRemoteContent
        ) {
            verified=local.as_ref().is_some_and(|l| matches!((&l.checksum,&remote.checksum),(Evidence::Known(a),Evidence::Known(b)) if a==b));
        }
        Ok(AppliedEffect {
            remote_id: remote.id.clone(),
            local,
            remote: Some(remote),
            verified,
        })
    }
    async fn upload(
        &self,
        session: &GoogleSession,
        r: &ExecuteOperationRequest,
        fs: &Filesystem,
        metadata: GoogleFileWrite,
        etag: Option<&str>,
    ) -> Result<GoogleFile, OperatorError> {
        let path = r
            .operation
            .local_path
            .as_deref()
            .ok_or_else(filesystem::invalid)?;
        let mut input = fs.read(path)?;
        let stamp = filesystem::stamp(&input.metadata().map_err(filesystem::error)?);
        let total = stamp.2;
        let upload = if r.operation.action == OperationAction::UpdateRemoteContent {
            self.client
                .start_resumable_update_conditional(
                    session,
                    &FileId(
                        r.operation
                            .remote_id
                            .as_ref()
                            .ok_or_else(filesystem::invalid)?
                            .0
                            .clone(),
                    ),
                    metadata,
                    FileUpdateOptions {
                        fields: Some(FieldMask(FIELDS.into())),
                        supports_all_drives: Some(true),
                        ..Default::default()
                    },
                    Some(total),
                    etag.ok_or_else(filesystem::stale)?,
                )
                .await
        } else {
            self.client
                .start_resumable_create(
                    session,
                    metadata,
                    FileCreateOptions {
                        fields: Some(FieldMask(FIELDS.into())),
                        supports_all_drives: Some(true),
                        ..Default::default()
                    },
                    Some(total),
                )
                .await
        }
        .map_err(mutation_error)?;
        let mut offset = 0;
        loop {
            let remaining = (total - offset).min(256 * 1024) as usize;
            let mut bytes = vec![0; remaining];
            input
                .read_exact(&mut bytes)
                .map_err(|e| filesystem::error(e).uncertain())?;
            if filesystem::stamp(&input.metadata().map_err(filesystem::error)?) != stamp {
                return Err(filesystem::stale().uncertain());
            }
            let chunk = GoogleUploadChunk {
                start: offset,
                bytes,
                total_bytes: Some(total),
            };
            let progress = if let Some(etag) = etag {
                self.client
                    .upload_chunk_conditional(session, &upload, chunk, etag)
                    .await
            } else {
                self.client.upload_chunk(session, &upload, chunk).await
            }
            .map_err(mutation_error)?;
            offset += remaining as u64;
            match progress {
                GoogleUploadProgress::Complete { file } if offset == total => return Ok(file),
                GoogleUploadProgress::Incomplete {
                    received_range: Some(range),
                } if range.start == 0
                    && range.end_inclusive == offset.checked_sub(1)
                    && offset < total => {}
                _ => return Err(filesystem::stale().uncertain()),
            }
        }
    }
}
impl RemoteExecutor for GoogleOperator {
    fn execute<'a>(
        &'a self,
        config: &'a OperatorConfig,
        r: &'a ExecuteOperationRequest,
        fs: &'a Filesystem,
    ) -> EffectFuture<'a> {
        Box::pin(async move {
            let account = self
                .accounts
                .get(crate::accounts::GetAccountRequest {
                    account_id: config.account_id.clone(),
                })
                .map_err(|_| OperatorError::new(OperatorErrorKind::PermissionDenied))?
                .account;
            if account.provider != config.provider_id {
                return Err(OperatorError::new(OperatorErrorKind::PermissionDenied));
            }
            let session = self
                .accounts
                .acquire_session(AcquireSessionRequest {
                    account_id: config.account_id.clone(),
                })
                .await
                .map_err(|_| OperatorError::new(OperatorErrorKind::PermissionDenied))?;
            self.run(config, r, fs, session.session.google()).await
        })
    }
}
fn read_error(error: GoogleApiFailure) -> OperatorError {
    let code = match error.kind {
        GoogleApiError::PermissionDenied
        | GoogleApiError::AuthenticationRequired
        | GoogleApiError::ReauthorizationRequired => OperatorErrorKind::PermissionDenied,
        GoogleApiError::PreconditionFailed => OperatorErrorKind::PreconditionFailed,
        GoogleApiError::UnsupportedOperation | GoogleApiError::UnsupportedMimeType => {
            OperatorErrorKind::UnsupportedOperation
        }
        _ => OperatorErrorKind::Unavailable,
    };
    OperatorError {
        code,
        retryable: error.retryable,
        effect: ExecutionEffect::NotApplied,
    }
}
fn mutation_error(error: GoogleApiFailure) -> OperatorError {
    let known = matches!(
        error.kind,
        GoogleApiError::PreconditionFailed
            | GoogleApiError::PermissionDenied
            | GoogleApiError::InvalidArgument
            | GoogleApiError::AuthenticationRequired
            | GoogleApiError::ReauthorizationRequired
    );
    let error = read_error(error);
    if known {
        error
    } else {
        error.uncertain()
    }
}
fn evidence<T>(value: Option<T>) -> Evidence<T> {
    value.map(Evidence::Known).unwrap_or(Evidence::NotCollected)
}
fn normalize(file: &GoogleFile) -> RemoteState {
    RemoteState {
        state: FileState::Present,
        id: file.id.as_ref().map(|id| RemoteId(id.0.clone())),
        name: evidence(file.name.clone()),
        parents: evidence(
            file.parents
                .as_ref()
                .map(|p| p.iter().map(|id| RemoteId(id.0.clone())).collect()),
        ),
        mime_type: evidence(file.mime_type.as_ref().map(|m| m.0.clone())),
        size: evidence(file.size.as_ref().and_then(|v| v.parse().ok())),
        checksum: evidence(file.sha256_checksum.as_ref().map(|v| Checksum {
            algorithm: "sha256".into(),
            value: v.clone(),
        })),
        version: evidence(file.version.as_ref().and_then(|v| v.parse().ok())),
        modified_time: Evidence::NotCollected,
        trashed: evidence(file.trashed),
        capabilities: evidence(file.capabilities.as_ref().map(|c| RemoteCapabilities {
            can_download: evidence(c.can_download),
            can_modify_content: evidence(c.can_edit),
            can_rename: evidence(c.can_rename),
            can_move_item_within_drive: evidence(c.can_move_item_within_drive),
            can_trash: evidence(c.can_trash),
        })),
    }
}

#[cfg(test)]
mod tests;
