//! Google specialization using the preserved provider API and private Accounts sessions.
mod conflicts;
mod projections;
mod recovery;
mod resume;
mod transfer;
mod trash;
use transfer::receive_verified;
struct UploadCursor {
    session: GoogleUploadSession,
    offset: u64,
}
use super::activity::{
    self, Algorithm, Bytes, Effect, HashProgress, Outcome, Phase, Progress, Span, State, Unit, Wait,
};
use super::journal::ExecutionJournal;
use super::{
    filesystem::{self, Filesystem},
    interfaces::{EffectFuture, RemoteExecutor},
    models::*,
};
use crate::inventory::models::{RecoveryCheckpoint, RecoveryRecord, TransferReference};
use crate::{
    accounts::{Accounts, AcquireSessionRequest},
    inventory::models::RemoteState,
    models::*,
    providers::google::*,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, io::Write};
const FIELDS: &str =
    "id,name,mimeType,parents,size,sha256Checksum,md5Checksum,headRevisionId,version,trashed,capabilities,webViewLink,appProperties,properties";
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
        let mut snapshot = self.snapshot_metadata(session, id).await?;
        let mut download = Span::default();
        let mut verification = None;
        let mut revalidate = Span::default();
        let mut bytes = Bytes::default();
        let verified = self
            .client
            .fill_content_sha256_observed(session, &mut snapshot.file, None, |event| match event {
                ContentVerification::Request(total) => {
                    download = Span::start(Phase::Download, State::Waiting(Wait::Response));
                    verification = Some(HashProgress::new(
                        Phase::VerifyContent,
                        Some(total),
                        Algorithm::Md5AndSha256,
                    ));
                    bytes = Bytes {
                        received: Some(0),
                        total: Some(total),
                        ..Bytes::default()
                    };
                }
                ContentVerification::Chunk(count) => {
                    bytes.received = Some(bytes.received.unwrap_or(0) + count as u64);
                    download.progress(Progress::Bytes(bytes), false);
                    if let Some(hash) = &mut verification {
                        hash.processed(count);
                    }
                }
                ContentVerification::Received => {
                    download.progress(Progress::Bytes(bytes), true);
                    download.finish(Outcome::Complete);
                }
                ContentVerification::Revalidate => {
                    if let Some(mut hash) = verification.take() {
                        hash.finish(Outcome::Complete);
                    }
                    revalidate =
                        Span::start(Phase::ValidateVersion, State::Waiting(Wait::Provider));
                }
            })
            .await;
        if download.enabled() {
            download.progress(Progress::Bytes(bytes), true);
            download.result(&verified);
            if let Some(mut hash) = verification.take() {
                hash.finish(Outcome::Cancelled);
            }
        }
        if let Some(mut hash) = verification {
            hash.result(&verified);
        }
        if revalidate.enabled() {
            revalidate.progress(
                Progress::Count {
                    unit: Unit::Requests,
                    completed: 1,
                    total: Some(1),
                },
                true,
            );
            revalidate.result(&verified);
        }
        verified.map_err(read_error)?;
        Ok(snapshot)
    }
    async fn verify_effect(
        &self,
        session: &GoogleSession,
        id: &RemoteId,
    ) -> Result<GoogleFileSnapshot, OperatorError> {
        activity::waiting(
            Phase::VerifyEffect,
            Wait::Provider,
            self.snapshot(session, id),
        )
        .await
    }

    async fn snapshot_metadata(
        &self,
        session: &GoogleSession,
        id: &RemoteId,
    ) -> Result<GoogleFileSnapshot, OperatorError> {
        let snapshot = activity::request(
            Phase::ReadMetadata,
            self.client.files_get_snapshot(
                session,
                &FileId(id.0.clone()),
                FileGetOptions {
                    fields: Some(FieldMask(FIELDS.into())),
                    supports_all_drives: Some(true),
                    ..Default::default()
                },
            ),
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
        let mut span = Span::start(Phase::ResolveAncestors, State::Running);
        let mut checked = 0;
        let result = span
            .within(async {
                let mut next = id.clone();
                let mut visited = BTreeSet::new();
                for _ in 0..128 {
                    if &next == root {
                        return Ok(());
                    }
                    if !visited.insert(next.clone()) {
                        return Err(filesystem::stale());
                    }
                    let file = self.snapshot_metadata(session, &next).await?.file;
                    checked += 1;
                    span.progress(
                        Progress::Count {
                            unit: Unit::Ancestors,
                            completed: checked,
                            total: None,
                        },
                        false,
                    );
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
            })
            .await;
        span.progress(
            Progress::Count {
                unit: Unit::Ancestors,
                completed: checked,
                total: None,
            },
            true,
        );
        span.result(&result);
        result
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
        let matches = activity::page(
            self.client.files_list(
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
            ),
            |page| page.files.len(),
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
    /// Pins ancestor location evidence; child mutations may advance a folder version without relocating it.
    async fn ancestry(
        &self,
        config: &OperatorConfig,
        session: &GoogleSession,
        parents: Vec<RemoteId>,
    ) -> Result<Vec<RemoteState>, OperatorError> {
        let mut span = Span::start(Phase::ResolveAncestors, State::Running);
        let mut checked = 0;
        let result = span
            .within(async {
                let mut seen = BTreeSet::new();
                let mut ancestors = Vec::new();
                for mut next in parents {
                    for _ in 0..128 {
                        if next == config.remote_root_id || seen.contains(&next) {
                            break;
                        }
                        seen.insert(next.clone());
                        let state = normalize(&self.snapshot(session, &next).await?.file);
                        checked += 1;
                        span.progress(
                            Progress::Count {
                                unit: Unit::Ancestors,
                                completed: checked,
                                total: None,
                            },
                            false,
                        );
                        if state.trashed != Evidence::Known(false)
                            || state.mime_type != Evidence::Known(FOLDER.into())
                        {
                            return Err(filesystem::stale());
                        }
                        let Evidence::Known(ids) = &state.parents else {
                            return Err(filesystem::stale());
                        };
                        if ids.len() != 1 {
                            return Err(filesystem::stale());
                        }
                        next = ids[0].clone();
                        ancestors.push(state);
                    }
                    if next != config.remote_root_id && !seen.contains(&next) {
                        return Err(filesystem::stale());
                    }
                }
                Ok(ancestors)
            })
            .await;
        span.progress(
            Progress::Count {
                unit: Unit::Ancestors,
                completed: checked,
                total: None,
            },
            true,
        );
        span.result(&result);
        result
    }
    async fn check_ancestry(
        &self,
        session: &GoogleSession,
        ancestors: &[RemoteState],
    ) -> Result<(), OperatorError> {
        let mut span = Span::start(Phase::ResolveAncestors, State::Running);
        let mut checked = 0;
        let result = span
            .within(async {
                for expected in ancestors {
                    let current = normalize(
                        &self
                            .snapshot(session, expected.id.as_ref().ok_or_else(filesystem::stale)?)
                            .await?
                            .file,
                    );
                    checked += 1;
                    span.progress(
                        Progress::Count {
                            unit: Unit::Ancestors,
                            completed: checked,
                            total: Some(ancestors.len() as u64),
                        },
                        false,
                    );
                    if current.id != expected.id
                        || current.name != expected.name
                        || current.parents != expected.parents
                        || current.mime_type != expected.mime_type
                        || current.trashed != expected.trashed
                    {
                        return Err(filesystem::stale());
                    }
                }
                Ok(())
            })
            .await;
        span.progress(
            Progress::Count {
                unit: Unit::Ancestors,
                completed: checked,
                total: Some(ancestors.len() as u64),
            },
            true,
        );
        span.result(&result);
        result
    }
    async fn run_with_journal(
        &self,
        config: &OperatorConfig,
        r: &ExecuteOperationRequest,
        fs: &Filesystem,
        session: &GoogleSession,
        journal: Option<&dyn ExecutionJournal>,
    ) -> Result<AppliedEffect, OperatorError> {
        let op = &r.operation;
        let pre = op.preconditions.as_ref().ok_or_else(filesystem::stale)?;
        let snapshot = if let Some(id) = &op.remote_id {
            self.within(session, id, &config.remote_root_id).await?;
            let snapshot = if matches!(
                op.action,
                OperationAction::Download | OperationAction::UpdateLocalContent
            ) {
                self.snapshot_metadata(session, id).await?
            } else {
                self.snapshot(session, id).await?
            };
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
        if let Some(current) = &snapshot {
            let remote = normalize(&current.file);
            if crate::projections::workspace(&remote) {
                if !matches!(
                    op.action,
                    OperationAction::RegisterIdentity
                        | OperationAction::CreateLocalShortcut
                        | OperationAction::RenameLocal
                        | OperationAction::MoveLocal
                        | OperationAction::ConfirmBaseline
                ) {
                    return Err(filesystem::stale());
                }
                self.projection_destination(config, session, &remote)
                    .await?;
                if r.local.state == FileState::Present
                    && !remote
                        .id
                        .as_ref()
                        .is_some_and(|id| crate::projections::content_matches(&r.local, id))
                {
                    return Err(filesystem::stale());
                }
            } else if matches!(&remote.mime_type,Evidence::Known(m) if matches!(crate::projections::kind(m),crate::projections::RemoteKind::RemoteShortcut|crate::projections::RemoteKind::Unsupported))
            {
                return Err(OperatorError::new(OperatorErrorKind::UnsupportedOperation));
            }
        }
        let tree_action = matches!(
            op.action,
            OperationAction::RenameLocal
                | OperationAction::MoveLocal
                | OperationAction::RenameRemote
                | OperationAction::MoveRemote
                | OperationAction::TrashRemote
                | OperationAction::RemoveLocal
        );
        let ancestry = if tree_action {
            let mut parents = match &r.remote.parents {
                Evidence::Known(ids) => ids.clone(),
                _ => return Err(filesystem::stale()),
            };
            parents.extend(op.parent_id.iter().cloned());
            let ancestors = self.ancestry(config, session, parents).await?;
            if let Some(journal) = journal {
                journal.checkpoint(RecoveryCheckpoint::RemoteAncestry(ancestors.clone()))?;
            }
            ancestors
        } else {
            Vec::new()
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
        if matches!(
            op.action,
            OperationAction::TrashRemote | OperationAction::RemoveLocal
        ) {
            self.trash_guard(config, r, fs, session).await?;
        }
        self.preserve_conflict(r, fs, session).await?;
        if let (Some(id), Some(journal)) = (&op.conflict_id, journal) {
            journal.checkpoint(RecoveryCheckpoint::ConflictArchive(id.clone()))?;
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
                if crate::projections::workspace(&normalize(&after.file))
                    && r.local.state == FileState::Present
                {
                    local = Some(fs.check(&r.local)?);
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
                self.check_ancestry(session, &ancestry).await?;
                if op.action == OperationAction::RemoveLocal {
                    self.trash_guard(config, r, fs, session).await?;
                }
                let mut effect = fs.execute(r, journal)?;
                self.check_ancestry(session, &ancestry)
                    .await
                    .map_err(OperatorError::uncertain)?;
                {
                    let after = self
                        .verify_effect(
                            session,
                            op.remote_id.as_ref().ok_or_else(filesystem::stale)?,
                        )
                        .await
                        .map_err(OperatorError::uncertain)?;
                    if after.file.version != source.file.version {
                        return Err(filesystem::stale().uncertain());
                    }
                }
                if op.action == OperationAction::RemoveLocal {
                    self.trash_root(config, session)
                        .await
                        .map_err(OperatorError::uncertain)?;
                    effect.local = Some(
                        fs.trashed(&journal.ok_or_else(filesystem::stale)?.record())
                            .map_err(OperatorError::uncertain)?
                            .ok_or_else(|| filesystem::stale().uncertain())?,
                    );
                }
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
                let record = journal.ok_or_else(filesystem::stale)?.record();
                let reserved = record
                    .reserved_remote_id
                    .as_ref()
                    .ok_or_else(filesystem::stale)?;
                let metadata = GoogleFileWrite {
                    id: Some(FileId(reserved.0.clone())),
                    app_properties: GooglePatch::Set(std::collections::BTreeMap::from([(
                        "drivesyncExecution".into(),
                        r.execution_id.0.clone(),
                    )])),
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
                    activity::mutation(
                        Phase::Dispatch,
                        self.client.files_create(
                            session,
                            metadata,
                            FileCreateOptions {
                                fields: Some(FieldMask(FIELDS.into())),
                                supports_all_drives: Some(true),
                                ..Default::default()
                            },
                        ),
                    )
                    .await
                    .map_err(mutation_error)?
                } else {
                    self.upload(session, r, fs, metadata, None, journal).await?
                };
                let id = file
                    .id
                    .as_ref()
                    .map(|id| RemoteId(id.0.clone()))
                    .ok_or_else(|| filesystem::stale().uncertain())?;
                if id != *reserved {
                    return Err(filesystem::stale().uncertain());
                }
                let observed = self
                    .verify_effect(session, &id)
                    .await
                    .map_err(OperatorError::uncertain)?
                    .file;
                if observed
                    .app_properties
                    .as_ref()
                    .and_then(|values| values.get("drivesyncExecution"))
                    != Some(&r.execution_id.0)
                    || observed.name.as_deref() != Some(name)
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
                        if caps.can_trash != Some(true) || caps.can_untrash != Some(true) {
                            return Err(filesystem::stale());
                        }
                        metadata.trashed = GooglePatch::Set(true);
                    }
                }
                if matches!(
                    op.action,
                    OperationAction::RenameRemote
                        | OperationAction::MoveRemote
                        | OperationAction::TrashRemote
                ) {
                    let mut properties = snapshot.file.app_properties.clone().unwrap_or_default();
                    properties.insert("drivesyncExecution".into(), r.execution_id.0.clone());
                    metadata.app_properties = GooglePatch::Set(properties);
                    metadata.properties = snapshot
                        .file
                        .properties
                        .clone()
                        .map(GooglePatch::Set)
                        .unwrap_or_default();
                }
                self.check_ancestry(session, &ancestry).await?;
                // The source and its path ancestors must still be the captured ones after destination reads.
                if r.local.state == FileState::Present {
                    fs.check(&r.local)?;
                }
                if let Some(journal) = journal {
                    let record = journal.record();
                    if record.parent_identity
                        != Some(fs.parent_identity(
                            op.local_path.as_deref().ok_or_else(filesystem::stale)?,
                        )?)
                    {
                        return Err(filesystem::stale());
                    }
                }
                if op.action == OperationAction::TrashRemote {
                    self.trash_guard(config, r, fs, session).await?;
                }
                activity::mutation(
                    if op.action == OperationAction::TrashRemote {
                        Phase::Trash
                    } else {
                        Phase::Dispatch
                    },
                    self.client.files_update_conditional(
                        session,
                        &FileId(id.0.clone()),
                        metadata,
                        options,
                        etag,
                    ),
                )
                .await
                .map_err(mutation_error)?;
                let after = self
                    .verify_effect(session, id)
                    .await
                    .map_err(OperatorError::uncertain)?
                    .file;
                self.check_ancestry(session, &ancestry)
                    .await
                    .map_err(OperatorError::uncertain)?;
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
                if op.action == OperationAction::TrashRemote {
                    self.trash_root(config, session)
                        .await
                        .map_err(OperatorError::uncertain)?;
                    let current = normalize(&after);
                    let expected = normalize(&snapshot.file);
                    if current.name != expected.name
                        || current.parents != expected.parents
                        || current.mime_type != expected.mime_type
                        || current.size != expected.size
                        || current.checksum != expected.checksum
                        || !matches!((&current.version,&expected.version),(Evidence::Known(a),Evidence::Known(b)) if a>b)
                    {
                        return Err(filesystem::stale().uncertain());
                    }
                    if r.local.kind == Evidence::Known(FileKind::Directory) {
                        self.require_remote_empty(session, id)
                            .await
                            .map_err(OperatorError::uncertain)?;
                    }
                    fs.confirm_missing_incarnation(&r.local, &config.sync_root_id)
                        .map_err(OperatorError::uncertain)?;
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
                let mut properties = snapshot.file.app_properties.clone().unwrap_or_default();
                properties.insert("drivesyncExecution".into(), r.execution_id.0.clone());
                self.upload(
                    session,
                    r,
                    fs,
                    GoogleFileWrite {
                        app_properties: GooglePatch::Set(properties),
                        properties: snapshot
                            .file
                            .properties
                            .clone()
                            .map(GooglePatch::Set)
                            .unwrap_or(GooglePatch::Unchanged),
                        ..Default::default()
                    },
                    snapshot.etag.as_deref(),
                    journal,
                )
                .await?;
                self.verify_effect(session, op.remote_id.as_ref().unwrap())
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
                    .or(match &r.remote.checksum {
                        Evidence::Known(checksum) if checksum.algorithm == "sha256" => {
                            Some(checksum.value.as_str())
                        }
                        _ => None,
                    })
                    .ok_or_else(filesystem::stale)?;
                let size = snapshot
                    .file
                    .size
                    .as_ref()
                    .and_then(|s| s.parse::<u64>().ok())
                    .ok_or_else(filesystem::stale)?;
                let mut stream = activity::request(
                    Phase::DownloadHeaders,
                    self.client.get_file_bytes(
                        session,
                        &FileId(op.remote_id.as_ref().unwrap().0.clone()),
                        None,
                    ),
                )
                .await
                .map_err(read_error)?;
                let mut temp = fs.temporary()?;
                receive_verified(&mut stream.body, &mut temp, size, expected).await?;
                let mut after = activity::waiting(
                    Phase::ValidateVersion,
                    Wait::Provider,
                    self.snapshot_metadata(session, op.remote_id.as_ref().unwrap()),
                )
                .await?;
                if after.file.version != snapshot.file.version
                    || after.file.head_revision_id != snapshot.file.head_revision_id
                    || after.file.size != snapshot.file.size
                    || after.file.md5_checksum != snapshot.file.md5_checksum
                    || after.file.trashed != Some(false)
                    || after
                        .file
                        .sha256_checksum
                        .as_deref()
                        .is_some_and(|sha| !sha.eq_ignore_ascii_case(expected))
                {
                    return Err(filesystem::stale());
                }
                after.file.sha256_checksum = Some(expected.to_owned());
                local = Some(fs.install(r, &mut temp, journal)?);
                after.file
            }
            OperationAction::CreateLocalShortcut => {
                let snapshot = snapshot.as_ref().ok_or_else(filesystem::stale)?;
                let remote = normalize(&snapshot.file);
                if !crate::projections::workspace(&remote)
                    || remote.trashed != Evidence::Known(false)
                {
                    return Err(filesystem::stale());
                }
                let id = op.remote_id.as_ref().ok_or_else(filesystem::stale)?;
                let expected = match (&remote.name, &remote.mime_type) {
                    (Evidence::Known(name), Evidence::Known(mime)) => {
                        crate::projections::local_name(name, mime)
                    }
                    _ => None,
                };
                if op
                    .local_path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .and_then(|s| s.to_str())
                    != expected.as_deref()
                {
                    return Err(filesystem::stale());
                }
                let mut temp = fs.temporary()?;
                temp.write_all(&crate::projections::link_bytes(id))
                    .map_err(filesystem::error)?;
                local = Some(fs.install(r, &mut temp, journal)?);
                let after = self
                    .verify_effect(session, id)
                    .await
                    .map_err(OperatorError::uncertain)?;
                if after.file.version != snapshot.file.version {
                    return Err(filesystem::stale().uncertain());
                }
                self.projection_destination(config, session, &normalize(&after.file))
                    .await
                    .map_err(OperatorError::uncertain)?;
                after.file
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
        let mut verification = Span::start(Phase::VerifyEffect, State::Running);
        let mut verified = true;
        if matches!(
            op.action,
            OperationAction::Upload | OperationAction::UpdateRemoteContent
        ) {
            verified = local
                .as_ref()
                .is_some_and(|l| crate::orchestrator::evidence::equivalent(l, &remote));
        }
        verification.effect(
            if verified {
                Effect::Verified
            } else {
                Effect::Applied
            },
            false,
        );
        verification.finish(if verified {
            Outcome::Complete
        } else {
            Outcome::Unconfirmed
        });
        Ok(AppliedEffect {
            remote_id: remote.id.clone(),
            local,
            remote: Some(remote),
            verified,
        })
    }
}

impl RemoteExecutor for GoogleOperator {
    fn resume_upload<'a>(
        &'a self,
        config: &'a OperatorConfig,
        record: &'a RecoveryRecord,
        fs: &'a Filesystem,
        journal: &'a dyn ExecutionJournal,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = RecoveryAssessment> + Send + 'a>> {
        Box::pin(async move {
            let session = match activity::waiting(
                Phase::AcquireSession,
                Wait::Authorization,
                self.accounts.acquire_session(AcquireSessionRequest {
                    account_id: config.account_id.clone(),
                }),
            )
            .await
            {
                Ok(session) => session,
                Err(_) => {
                    return RecoveryAssessment::unknown(
                        crate::inventory::models::RecoveryIssue::RemoteInconclusive,
                    )
                }
            };
            self.continue_upload(config, record, fs, journal, session.session.google())
                .await
        })
    }
    fn reserve<'a>(
        &'a self,
        config: &'a OperatorConfig,
        r: &'a ExecuteOperationRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Option<RemoteId>, OperatorError>> + Send + 'a>,
    > {
        Box::pin(async move {
            if !matches!(
                r.operation.action,
                OperationAction::Upload | OperationAction::CreateRemoteDirectory
            ) {
                return Ok(None);
            }
            let session = activity::waiting(
                Phase::AcquireSession,
                Wait::Authorization,
                self.accounts.acquire_session(AcquireSessionRequest {
                    account_id: config.account_id.clone(),
                }),
            )
            .await
            .map_err(|_| OperatorError::new(OperatorErrorKind::PermissionDenied))?;
            let ids = activity::request(
                Phase::Prepare,
                self.client.files_generate_ids(
                    session.session.google(),
                    GenerateIdsOptions {
                        count: Some(1),
                        space: Some("drive".into()),
                        type_: Some("files".into()),
                    },
                ),
            )
            .await
            .map_err(read_error)?
            .ids
            .ok_or_else(filesystem::stale)?;
            if ids.len() != 1 || ids[0].0.is_empty() {
                return Err(filesystem::stale());
            }
            Ok(Some(RemoteId(ids[0].0.clone())))
        })
    }
    fn reconcile<'a>(
        &'a self,
        config: &'a OperatorConfig,
        record: &'a RecoveryRecord,
        fs: &'a Filesystem,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = RecoveryAssessment> + Send + 'a>> {
        Box::pin(async move {
            let session = match activity::waiting(
                Phase::AcquireSession,
                Wait::Authorization,
                self.accounts.acquire_session(AcquireSessionRequest {
                    account_id: config.account_id.clone(),
                }),
            )
            .await
            {
                Ok(session) => session,
                Err(_) => {
                    return RecoveryAssessment::unknown(
                        crate::inventory::models::RecoveryIssue::RemoteInconclusive,
                    )
                }
            };
            self.probe_recovery(config, record, fs, session.session.google())
                .await
        })
    }
    fn execute<'a>(
        &'a self,
        config: &'a OperatorConfig,
        r: &'a ExecuteOperationRequest,
        fs: &'a Filesystem,
        journal: Option<&'a dyn ExecutionJournal>,
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
            let session = activity::waiting(
                Phase::AcquireSession,
                Wait::Authorization,
                self.accounts.acquire_session(AcquireSessionRequest {
                    account_id: config.account_id.clone(),
                }),
            )
            .await
            .map_err(|_| OperatorError::new(OperatorErrorKind::PermissionDenied))?;
            self.run_with_journal(config, r, fs, session.session.google(), journal)
                .await
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
    let nonbinary = file
        .mime_type
        .as_ref()
        .is_some_and(|m| m.0.starts_with("application/vnd.google-apps."));
    RemoteState {
        state: if file.trashed == Some(true) {
            FileState::Absent
        } else {
            FileState::Present
        },
        id: file.id.as_ref().map(|id| RemoteId(id.0.clone())),
        name: evidence(file.name.clone()),
        parents: evidence(
            file.parents
                .as_ref()
                .map(|p| p.iter().map(|id| RemoteId(id.0.clone())).collect()),
        ),
        mime_type: evidence(file.mime_type.as_ref().map(|m| m.0.clone())),
        size: if nonbinary {
            Evidence::NotApplicable
        } else {
            evidence(file.size.as_ref().and_then(|v| v.parse().ok()))
        },
        checksum: if nonbinary {
            Evidence::NotApplicable
        } else {
            evidence(file.sha256_checksum.as_ref().map(|v| Checksum {
                algorithm: "sha256".into(),
                value: v.to_ascii_lowercase(),
            }))
        },
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

#[cfg(test)]
mod recovery_tests;

#[cfg(test)]
mod live_recovery_tests;

#[cfg(test)]
mod initial_download_tests;

#[cfg(test)]
mod initial_upload_tests;

#[cfg(test)]
mod live_upload_recovery_tests;

#[cfg(test)]
mod conflict_tests;
