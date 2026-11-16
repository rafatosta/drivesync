//! Durable technical checkpoints, deliberately disconnected from queue eligibility.
use super::*;
use crate::operations::*;
use rusqlite::{params, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};

fn encode<T: Serialize>(value: &T) -> InventoryResult<String> {
    serde_json::to_string(value).map_err(|_| failure(InventoryError::SerializationFailed))
}
fn decode<T: DeserializeOwned>(value: &str) -> InventoryResult<T> {
    serde_json::from_str(value).map_err(|_| failure(InventoryError::CorruptDatabase))
}
fn scope(db: &Connection, intent: &ExecutionIntent) -> InventoryResult<()> {
    let info = sync_info(db)?;
    let root: Vec<u8> = db
        .query_row("SELECT local_root FROM sync_info WHERE id=1", [], |r| {
            r.get(0)
        })
        .map_err(sql)?;
    if intent.scope.sync_id != info.sync_id
        || intent.scope.account_id != info.account_id
        || intent.scope.remote_root_id.0 != info.remote_root_id
        || intent.scope.local_root != root
    {
        return Err(failure(InventoryError::InvalidIdentity));
    }
    if let OperationCommand::Restore {
        target: ResourceRef::Local { relative_path, .. },
        ..
    } = &intent.command
    {
        if local_trash_path(relative_path, None) {
            if let OperationCommand::Restore {
                destination: Some(destination),
                ..
            } = &intent.command
            {
                super::planning::command_scope(
                    &OperationCommand::CreateDirectory {
                        parent: destination.clone(),
                        name: ItemName::Local("restore-validation".into()),
                    },
                    &info.account_id,
                )?;
            }
            return Ok(());
        }
    }
    super::planning::command_scope(&intent.command, &info.account_id)
}
fn local_trash_path(
    path: &crate::models::LocalRelativePath,
    expected: Option<&ExecutionId>,
) -> bool {
    let p: Vec<_> = path.as_bytes().split(|b| *b == b'/').collect();
    p.len() == 4
        && p[0] == b".drivesync"
        && p[1] == b"trash"
        && p[3] == b"item"
        && std::str::from_utf8(p[2])
            .ok()
            .and_then(|id| ExecutionId::new(id.to_owned()).ok())
            .is_some_and(|id| expected.is_none_or(|expected| *expected == id))
}
fn validate_intent(intent: &ExecutionIntent) -> InventoryResult<()> {
    let direction = match &intent.command {
        OperationCommand::Upload {
            source,
            destination,
            name,
        } => matches!(
            (source, destination, name),
            (
                ResourceRef::Local { .. },
                DirectoryRef::Remote { .. },
                ItemName::Remote(_)
            )
        ),
        OperationCommand::Download {
            source,
            destination,
            name,
        } => matches!(
            (source, destination, name),
            (
                ResourceRef::Remote { .. },
                DirectoryRef::LocalRoot | DirectoryRef::Local { .. },
                ItemName::Local(_)
            )
        ),
        OperationCommand::CreateFile {
            content: Some(ContentSource::RemoteFile { resource }),
            ..
        }
        | OperationCommand::UpdateContent {
            source: ContentSource::RemoteFile { resource },
            ..
        } => matches!(resource, ResourceRef::Remote { .. }),
        _ => true,
    };
    if !direction || !intent.validate_source() {
        return Err(failure(InventoryError::InvalidArgument));
    }
    Ok(())
}
fn validate_state(intent: &ExecutionIntent, state: &ExecutionState) -> InventoryResult<()> {
    if !state.is_valid() {
        return Err(failure(InventoryError::InvalidTransition));
    }
    for resource in state
        .reserved_remote
        .iter()
        .chain(state.result.iter().map(|r| &r.item().resource))
    {
        if matches!(intent.command, OperationCommand::Trash { .. })
            && matches!(resource,ResourceRef::Local {relative_path,..} if local_trash_path(relative_path,Some(&intent.context.id)))
        {
            continue;
        }
        super::planning::command_scope(
            &OperationCommand::Trash {
                target: resource.clone(),
                preconditions: Preconditions::default(),
            },
            &intent.scope.account_id,
        )?;
    }
    if let (Some(reserved), Some(result)) = (&state.reserved_remote, &state.result) {
        if result.item().resource != *reserved {
            return Err(failure(InventoryError::InvalidIdentity));
        }
    }
    let remote_destination = match &intent.command {
        OperationCommand::Upload { destination, .. }
        | OperationCommand::CreateFile {
            parent: destination,
            ..
        }
        | OperationCommand::CreateDirectory {
            parent: destination,
            ..
        } => match destination {
            DirectoryRef::Remote { provider_id, .. } => Some(provider_id),
            _ => None,
        },
        _ => None,
    };
    if let Some(reserved) = &state.reserved_remote {
        let ResourceRef::Remote { provider_id, .. } = reserved else {
            return Err(failure(InventoryError::InvalidIdentity));
        };
        let same_target = matches!(&intent.command, OperationCommand::UpdateContent { target, .. } if target == reserved);
        if remote_destination != Some(provider_id) && !same_target {
            return Err(failure(InventoryError::InvalidIdentity));
        }
    }
    if state.phase != CheckpointPhase::Prepared
        && remote_destination.is_some()
        && state.reserved_remote.is_none()
    {
        return Err(failure(InventoryError::InvalidTransition));
    }
    if state.phase == CheckpointPhase::Prepared
        && (state.resume_token.is_some() || state.confirmed_offset != 0)
    {
        return Err(failure(InventoryError::InvalidTransition));
    }
    let source_size = match &intent.context.source {
        Some(SourceEvidence::Local { metadata, .. }) => match metadata.storage.size_bytes {
            MetadataValue::Known(n) => Some(n),
            _ => None,
        },
        Some(SourceEvidence::Remote { metadata, .. }) => match metadata.size {
            MetadataValue::Known(n) => Some(n),
            _ => None,
        },
        None => match &intent.command {
            OperationCommand::CreateFile { content: None, .. } => Some(0),
            OperationCommand::CreateFile {
                content: Some(ContentSource::Bytes { data }),
                ..
            }
            | OperationCommand::UpdateContent {
                source: ContentSource::Bytes { data },
                ..
            } => Some(data.len() as u64),
            _ => None,
        },
    };
    if state.confirmed_offset > source_size.unwrap_or(0) {
        return Err(failure(InventoryError::InvalidArgument));
    }
    if matches!(intent.context.source, Some(SourceEvidence::Local { .. }))
        && matches!(
            intent.command,
            OperationCommand::CreateFile { .. }
                | OperationCommand::Upload { .. }
                | OperationCommand::Download { .. }
                | OperationCommand::UpdateContent { .. }
        )
        && state.phase != CheckpointPhase::Prepared
        && state.staging.is_none()
    {
        return Err(failure(InventoryError::InvalidTransition));
    }
    if let Some(staging) = &state.staging {
        if staging.metadata.storage.size_bytes
            != source_size
                .map(MetadataValue::Known)
                .unwrap_or(MetadataValue::NotCollected)
        {
            return Err(failure(InventoryError::InvalidArgument));
        }
        if let Some(SourceEvidence::Local {
            sha256: Some(expected),
            ..
        }) = &intent.context.source
        {
            if staging.sha256 != *expected {
                return Err(failure(InventoryError::InvalidArgument));
            }
        }
    }
    if let Some(result) = &state.result {
        if let Some(metadata) = &result.item().metadata {
            let consistent = match (&result.item().resource, metadata) {
                (
                    ResourceRef::Local {
                        relative_path,
                        local_id,
                    },
                    FileData::Local(file),
                ) => {
                    file.validate().is_ok()
                        && file.relative_path == *relative_path
                        && file.local_id == *local_id
                }
                (
                    ResourceRef::Remote {
                        provider_id,
                        reference,
                    },
                    FileData::Remote(file),
                ) => {
                    file.validate().is_ok()
                        && file.provider == *provider_id
                        && file.reference == *reference
                }
                _ => false,
            };
            if !consistent {
                return Err(failure(InventoryError::InvalidIdentity));
            }
        }
        let transfer = matches!(
            intent.command,
            OperationCommand::Upload { .. }
                | OperationCommand::Download { .. }
                | OperationCommand::UpdateContent { .. }
        );
        if transfer != matches!(result, ExecutionResult::Transfer(_)) {
            return Err(failure(InventoryError::InvalidArgument));
        }
        if let OperationCommand::Download {
            destination, name, ..
        } = &intent.command
        {
            let ResourceRef::Local { relative_path, .. } = &result.item().resource else {
                return Err(failure(InventoryError::InvalidIdentity));
            };
            let mut expected = match destination {
                DirectoryRef::LocalRoot => vec![],
                DirectoryRef::Local { relative_path, .. } => {
                    let mut path = relative_path.as_bytes().to_vec();
                    path.push(b'/');
                    path
                }
                _ => return Err(failure(InventoryError::InvalidIdentity)),
            };
            use std::os::unix::ffi::OsStrExt;
            let ItemName::Local(name) = name else {
                return Err(failure(InventoryError::InvalidIdentity));
            };
            expected.extend_from_slice(name.as_bytes());
            if relative_path.as_bytes() != expected {
                return Err(failure(InventoryError::InvalidIdentity));
            }
        }
        if let OperationCommand::UpdateContent { target, .. } = &intent.command {
            let same = match (target, &result.item().resource) {
                (
                    ResourceRef::Local {
                        relative_path: before,
                        ..
                    },
                    ResourceRef::Local {
                        relative_path: after,
                        ..
                    },
                ) => before == after,
                (before, after) => before == after,
            };
            if !same {
                return Err(failure(InventoryError::InvalidIdentity));
            }
        }
    }
    Ok(())
}
pub(super) fn load(db: &Connection, id: &ExecutionId) -> InventoryResult<ExecutionCheckpoint> {
    let intent: String = db
        .query_row(
            "SELECT intent FROM execution_intents WHERE id=?1",
            [id.as_str()],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?
        .ok_or_else(|| failure(InventoryError::NotFound))?;
    let intent: ExecutionIntent = decode(&intent)?;
    let checkpoint = db.query_row(
        "SELECT revision,phase,effect,staging,reserved_remote,resume_token,confirmed_offset,result,local_scope,session_generation FROM execution_checkpoints WHERE execution=?1", [id.as_str()], |r| {
            Ok((r.get::<_, u64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, Option<String>>(3)?, r.get::<_, Option<String>>(4)?, r.get::<_, Option<Vec<u8>>>(5)?, r.get::<_, String>(6)?, r.get::<_, Option<String>>(7)?, r.get::<_, Option<String>>(8)?, r.get::<_, u32>(9)?))
        }).optional().map_err(sql)?.ok_or_else(|| failure(InventoryError::CorruptDatabase))?;
    let corrupt = || failure(InventoryError::CorruptDatabase);
    let state = ExecutionState {
        local_scope: checkpoint.8.as_deref().map(decode).transpose()?,
        session_generation: checkpoint.9,
        phase: decode(&checkpoint.1)?,
        effect: decode(&checkpoint.2)?,
        staging: checkpoint.3.as_deref().map(decode).transpose()?,
        reserved_remote: checkpoint.4.as_deref().map(decode).transpose()?,
        resume_token: checkpoint
            .5
            .map(ResumeToken::new)
            .transpose()
            .map_err(|_| corrupt())?,
        confirmed_offset: checkpoint.6.parse().map_err(|_| corrupt())?,
        result: checkpoint.7.as_deref().map(decode).transpose()?,
    };
    if intent.context.id != *id
        || checkpoint.0 == 0
        || scope(db, &intent).is_err()
        || validate_intent(&intent).is_err()
        || validate_state(&intent, &state).is_err()
    {
        return Err(corrupt());
    }
    Ok(ExecutionCheckpoint {
        intent,
        revision: checkpoint.0,
        state,
    })
}
fn write(
    db: &Connection,
    id: &ExecutionId,
    revision: u64,
    state: &ExecutionState,
) -> InventoryResult<()> {
    db.execute("INSERT INTO execution_checkpoints(execution,revision,phase,effect,staging,reserved_remote,resume_token,confirmed_offset,result,local_scope,session_generation) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11) ON CONFLICT(execution) DO UPDATE SET revision=excluded.revision,phase=excluded.phase,effect=excluded.effect,staging=excluded.staging,reserved_remote=excluded.reserved_remote,resume_token=excluded.resume_token,confirmed_offset=excluded.confirmed_offset,result=excluded.result,local_scope=excluded.local_scope,session_generation=excluded.session_generation",
        params![id.as_str(), revision, encode(&state.phase)?, encode(&state.effect)?, state.staging.as_ref().map(encode).transpose()?, state.reserved_remote.as_ref().map(encode).transpose()?, state.resume_token.as_ref().map(|t| t.as_bytes()), state.confirmed_offset.to_string(), state.result.as_ref().map(encode).transpose()?, state.local_scope.as_ref().map(encode).transpose()?, state.session_generation]).map_err(sql)?;
    Ok(())
}
impl InventoryManager {
    /// Atomically creates an immutable execution binding and revision-1 checkpoint.
    /// An exact retry returns the current checkpoint, including uncertain effects.
    /// Does not enqueue, claim, release, authorize or execute any operation.
    pub fn prepare_execution(
        &self,
        request: PrepareExecutionRequest,
    ) -> InventoryResult<ExecutionCheckpoint> {
        self.run(true, |db| prepare(db, request))
    }

    /// Retires only an expired create-upload session after explicit remote
    /// reconciliation. The reserved ID, snapshot and Unknown effect survive.
    /// A lost response is recovered by get_execution, never by guessing an offset.
    pub fn reset_expired_upload(
        &self,
        request: ResetExpiredUploadRequest,
    ) -> InventoryResult<ExecutionCheckpoint> {
        self.run(true, |db| {
            let mut current = load(db, &request.id)?;
            if current.revision != request.expected_revision {
                return Err(failure(InventoryError::VersionMismatch));
            }
            if !matches!(
                current.intent.command,
                OperationCommand::Upload { .. }
                    | OperationCommand::CreateFile {
                        parent: DirectoryRef::Remote { .. },
                        ..
                    }
                    | OperationCommand::UpdateContent {
                        target: ResourceRef::Remote { .. },
                        ..
                    }
            ) || current.state.phase != CheckpointPhase::Executing
                || current.state.effect != ExecutionEffect::Unknown
                || current.state.resume_token.as_ref() != Some(&request.expired_session)
                || current.state.reserved_remote.as_ref() != Some(&request.reserved_remote)
            {
                return Err(failure(InventoryError::InvalidTransition));
            }
            current.state.resume_token = None;
            current.state.confirmed_offset = 0;
            current.state.session_generation = current
                .state
                .session_generation
                .checked_add(1)
                .ok_or_else(|| failure(InventoryError::InvalidTransition))?;
            current.revision = current
                .revision
                .checked_add(1)
                .filter(|n| *n <= i64::MAX as u64)
                .ok_or_else(|| failure(InventoryError::InvalidTransition))?;
            validate_state(&current.intent, &current.state)?;
            write(db, &request.id, current.revision, &current.state)?;
            Ok(current)
        })
    }
    /// Reads recovery facts without changing state, offset, revision or eligibility.
    /// Sensitive resume bytes are available only through this typed recovery API.
    pub fn get_execution(
        &self,
        request: GetExecutionRequest,
    ) -> InventoryResult<ExecutionCheckpoint> {
        self.run(false, |db| load(db, &request.id))
    }
    /// Compare-and-swap checkpoint update. Retrying the immediately preceding
    /// identical write returns its committed result; stale divergent writes fail.
    /// Completed records are immutable. Failure never permits dependent effects.
    pub fn save_execution(
        &self,
        request: SaveExecutionRequest,
    ) -> InventoryResult<ExecutionCheckpoint> {
        self.run(true, |db| {
            let mut current = load(db, &request.id)?;
            if current.revision != request.expected_revision {
                if request.expected_revision.checked_add(1) == Some(current.revision)
                    && current.state == request.state
                {
                    return Ok(current);
                }
                return Err(failure(InventoryError::VersionMismatch));
            }
            validate_state(&current.intent, &request.state)?;
            if !request.state.can_follow(&current.state) {
                return Err(failure(InventoryError::InvalidTransition));
            }
            let revision = current
                .revision
                .checked_add(1)
                .filter(|n| *n <= i64::MAX as u64)
                .ok_or_else(|| failure(InventoryError::InvalidTransition))?;
            write(db, &request.id, revision, &request.state)?;
            current.revision = revision;
            current.state = request.state;
            Ok(current)
        })
    }
}
fn port_error(error: InventoryFailure) -> OperatorFailure {
    let kind = match error.kind {
        InventoryError::AlreadyExists | InventoryError::VersionMismatch => {
            OperatorError::CheckpointConflict
        }
        InventoryError::InvalidIdentity => OperatorError::WrongScope,
        InventoryError::InvalidArgument | InventoryError::InvalidTransition => {
            OperatorError::InvalidRequest
        }
        InventoryError::NotFound => OperatorError::NotFoundOrInaccessible,
        _ => OperatorError::CheckpointUnavailable,
    };
    // A journal failure describes storage, not proof about a file effect. Even
    // a failed save may follow an applied command; callers must reconcile.
    OperatorFailure {
        kind,
        effect: ExecutionEffect::Unknown,
        resource: None,
    }
}
impl ExecutionJournal for InventoryManager {
    fn reset_expired_upload(
        &self,
        request: ResetExpiredUploadRequest,
    ) -> OperatorResult<ExecutionCheckpoint> {
        InventoryManager::reset_expired_upload(self, request).map_err(port_error)
    }

    fn prepare_execution(
        &self,
        request: PrepareExecutionRequest,
    ) -> OperatorResult<ExecutionCheckpoint> {
        InventoryManager::prepare_execution(self, request).map_err(port_error)
    }
    fn get_execution(&self, request: GetExecutionRequest) -> OperatorResult<ExecutionCheckpoint> {
        InventoryManager::get_execution(self, request).map_err(port_error)
    }
    fn save_execution(&self, request: SaveExecutionRequest) -> OperatorResult<ExecutionCheckpoint> {
        InventoryManager::save_execution(self, request).map_err(port_error)
    }
}

pub(super) fn prepare(
    db: &Connection,
    request: PrepareExecutionRequest,
) -> InventoryResult<ExecutionCheckpoint> {
    scope(db, &request.intent)?;
    validate_intent(&request.intent)?;
    let id = &request.intent.context.id;
    let exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM execution_intents WHERE id=?1)",
            [id.as_str()],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if exists {
        let previous = load(db, id)?;
        if previous.intent != request.intent {
            return Err(failure(InventoryError::AlreadyExists));
        }
        return Ok(previous);
    }
    db.execute(
        "INSERT INTO execution_intents(id,intent) VALUES(?1,?2)",
        params![id.as_str(), encode(&request.intent)?],
    )
    .map_err(sql)?;
    let state = ExecutionState::prepared();
    write(db, id, 1, &state)?;
    Ok(ExecutionCheckpoint {
        intent: request.intent,
        revision: 1,
        state,
    })
}
