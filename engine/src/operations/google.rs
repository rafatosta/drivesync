//! Create-only blob transfers through Providers, with Inventory-owned recovery.
use super::{filesystem as fs, *};
use crate::{connection::OperatorAccess, models::*, providers::google as g, Engine};
use std::sync::Arc;

/// Factory resolves the persisted account provider. It never accepts credentials
/// or caller-supplied provider selection and never enables Analyzer queue work.
#[derive(Default)]
pub struct DefaultOperatorFactory;
mod download;
#[cfg(test)]
mod tests;
mod upload;
impl OperatorFactory for DefaultOperatorFactory {
    fn create(&self, config: OperatorConfig) -> OperatorResult<Arc<dyn Operator>> {
        Ok(Arc::new(GoogleOperator::new(config)?))
    }
}
/// Google supports new-file upload/download here. Other mutations explicitly
/// report UnsupportedOperation; permanent deletion always refuses locally.
pub struct GoogleOperator {
    scope: OperatorScope,
    journal: Arc<dyn ExecutionJournal>,
    host: Host,
}
#[derive(Clone)]
enum Host {
    Engine(Engine),
    #[cfg(test)]
    Test {
        runtime: Arc<TestRuntime>,
        client: Arc<g::GoogleDriveClient>,
    },
}
#[cfg(test)]
struct TestRuntime(Option<tokio::runtime::Runtime>);
#[cfg(test)]
impl Drop for TestRuntime {
    fn drop(&mut self) {
        if let Some(r) = self.0.take() {
            r.shutdown_background();
        }
    }
}
impl Host {
    fn runtime(&self) -> tokio::runtime::Handle {
        match self {
            Self::Engine(engine) => engine.operator_runtime(),
            #[cfg(test)]
            Self::Test { runtime, .. } => runtime.0.as_ref().unwrap().handle().clone(),
        }
    }
    async fn access(&self, account: AccountId) -> OperatorResult<OperatorAccess> {
        match self {
            Self::Engine(engine) => engine.operator_access(account).await,
            #[cfg(test)]
            Self::Test { client, .. } => {
                Ok(OperatorAccess::testing(client.as_ref().clone(), account))
            }
        }
    }
}
impl GoogleOperator {
    fn transfer(
        &self,
        context: ExecutionContext,
        command: OperationCommand,
        options: ExecutionOptions,
    ) -> OperationFuture<'_, TransferResult> {
        let scope = self.scope.clone();
        let journal = self.journal.clone();
        let host = self.host.clone();
        let runtime = host.runtime();
        Box::pin(async move {
            // The worker owns host/runtime/locks even if the awaiting future is
            // dropped. Blocking filesystem/SQLite never runs on GTK or Tokio I/O.
            let worker_runtime = runtime.clone();
            runtime
                .spawn_blocking(move || {
                    worker_runtime.block_on(async move {
                        fs::check(&options)?;
                        let workspace = fs::Workspace::open(&scope.local_root)?;
                        let access = host.access(scope.account_id.clone()).await?;
                        let intent = ExecutionIntent {
                            context,
                            scope: ExecutionScope::from(&scope),
                            command,
                        };
                        let checkpoint =
                            journal.prepare_execution(PrepareExecutionRequest { intent })?;
                        let mut worker = Worker {
                            scope,
                            workspace,
                            access,
                            journal,
                            checkpoint,
                            options,
                            transferred: 0,
                            publication_effect: None,
                        };
                        let result = match worker.checkpoint.intent.command {
                            OperationCommand::Download { .. } => worker.download().await,
                            OperationCommand::Upload { .. } => worker.upload().await,
                            _ => Err(fs::fail(OperatorError::UnsupportedOperation)),
                        };
                        result.map_err(|mut e| {
                            if let Some(effect) = worker.publication_effect {
                                e.effect = effect;
                            } else if e.effect == ExecutionEffect::NotApplied
                                || worker.effect() == ExecutionEffect::Applied
                            {
                                e.effect = worker.effect();
                            }
                            e
                        })
                    })
                })
                .await
                .map_err(|_| OperatorFailure {
                    kind: OperatorError::Internal,
                    effect: ExecutionEffect::Unknown,
                    resource: None,
                })?
        })
    }
    #[cfg(test)]
    fn testing(
        scope: OperatorScope,
        journal: Arc<dyn ExecutionJournal>,
        client: g::GoogleDriveClient,
    ) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        Self {
            scope,
            journal,
            host: Host::Test {
                runtime: Arc::new(TestRuntime(Some(runtime))),
                client: Arc::new(client),
            },
        }
    }
}
fn unsupported<T: Send + 'static>() -> OperationFuture<'static, T> {
    Box::pin(async { Err(fs::fail(OperatorError::UnsupportedOperation)) })
}
impl Operator for GoogleOperator {
    fn new(config: OperatorConfig) -> OperatorResult<Self> {
        if !config.scope.local_root.is_absolute()
            || config.scope.sync_id.0.trim().is_empty()
            || config.scope.remote_root_id.0.trim().is_empty()
        {
            return Err(fs::fail(OperatorError::InvalidConfiguration));
        }
        if config
            .connector
            .operator_provider(&config.scope.account_id)?
            .0
            != "google"
        {
            return Err(fs::fail(OperatorError::UnsupportedProvider));
        }
        Ok(Self {
            scope: config.scope,
            journal: config.journal,
            host: Host::Engine(config.connector),
        })
    }
    fn upload(&self, r: UploadRequest) -> OperationFuture<'_, TransferResult> {
        self.transfer(
            r.execution,
            OperationCommand::Upload {
                source: r.source,
                destination: r.destination,
                name: r.name,
            },
            r.options,
        )
    }
    fn download(&self, r: DownloadRequest) -> OperationFuture<'_, TransferResult> {
        self.transfer(
            r.execution,
            OperationCommand::Download {
                source: r.source,
                destination: r.destination,
                name: r.name,
            },
            r.options,
        )
    }
    fn create_file(&self, _: CreateFileRequest) -> OperationFuture<'_, ItemResult> {
        unsupported()
    }
    fn create_directory(&self, _: CreateDirectoryRequest) -> OperationFuture<'_, ItemResult> {
        unsupported()
    }
    fn update_content(&self, _: UpdateContentRequest) -> OperationFuture<'_, TransferResult> {
        unsupported()
    }
    fn rename(&self, _: RenameRequest) -> OperationFuture<'_, ItemResult> {
        unsupported()
    }
    fn move_item(&self, _: MoveRequest) -> OperationFuture<'_, ItemResult> {
        unsupported()
    }
    fn trash(&self, _: TrashRequest) -> OperationFuture<'_, ItemResult> {
        unsupported()
    }
    fn restore(&self, _: RestoreRequest) -> OperationFuture<'_, ItemResult> {
        unsupported()
    }
    fn delete(&self, _: DeleteRequest) -> OperationFuture<'_, DeleteResult> {
        eprintln!("DriveSync: permanent deletion is prohibited. Nothing was executed.");
        Box::pin(async { Err(fs::fail(OperatorError::PermanentDeletionProhibited)) })
    }
}

struct Worker {
    scope: OperatorScope,
    workspace: fs::Workspace,
    access: OperatorAccess,
    journal: Arc<dyn ExecutionJournal>,
    checkpoint: ExecutionCheckpoint,
    options: ExecutionOptions,
    transferred: u64,
    publication_effect: Option<ExecutionEffect>,
}
impl Worker {
    fn effect(&self) -> ExecutionEffect {
        if let Some(effect) = self.publication_effect {
            return effect;
        }
        if matches!(
            self.checkpoint.intent.command,
            OperationCommand::Download { .. }
        ) && matches!(
            self.checkpoint.state.phase,
            CheckpointPhase::Prepared | CheckpointPhase::Executing
        ) {
            ExecutionEffect::NotApplied
        } else {
            self.checkpoint.state.effect
        }
    }
    fn error(&self, kind: OperatorError) -> OperatorFailure {
        OperatorFailure {
            kind,
            effect: self.effect(),
            resource: self.checkpoint.state.reserved_remote.clone(),
        }
    }
    fn check(&self) -> OperatorResult<()> {
        fs::check(&self.options).map_err(|e| self.error(e.kind))?;
        self.workspace.validate().map_err(|e| self.error(e.kind))
    }
    fn save(&mut self, state: ExecutionState) -> OperatorResult<()> {
        self.checkpoint = self.journal.save_execution(SaveExecutionRequest {
            id: self.checkpoint.intent.context.id.clone(),
            expected_revision: self.checkpoint.revision,
            state,
        })?;
        Ok(())
    }
    fn bind_scope(&mut self, parent: &std::fs::File) -> OperatorResult<()> {
        let actual = self.workspace.scope(parent)?;
        if let Some(expected) = &self.checkpoint.state.local_scope {
            if *expected != actual {
                return Err(self.error(OperatorError::WrongScope));
            }
        } else {
            let mut next = self.checkpoint.state.clone();
            next.local_scope = Some(actual);
            self.save(next)?;
        }
        Ok(())
    }
    fn provider_error(&self, e: g::GoogleApiFailure) -> OperatorFailure {
        use g::GoogleApiError as G;
        let kind = match e.kind {
            G::PermissionDenied
            | G::InsufficientScope
            | G::AuthenticationRequired
            | G::ReauthorizationRequired
            | G::InvalidGrant => OperatorError::PermissionDenied,
            G::NotFoundOrInaccessible => OperatorError::NotFoundOrInaccessible,
            G::Network
            | G::Timeout
            | G::RateLimited
            | G::QuotaExceeded
            | G::ProviderUnavailable => OperatorError::Network,
            G::StorageQuotaExceeded => OperatorError::InsufficientSpace,
            G::PreconditionFailed => OperatorError::PreconditionFailed,
            G::UnsupportedOperation | G::UnsupportedMimeType | G::FileNotDownloadable => {
                OperatorError::UnsupportedOperation
            }
            G::IncompleteTransfer => OperatorError::IntegrityMismatch,
            _ => OperatorError::IndeterminateEffect,
        };
        self.error(kind)
    }
    async fn get(&self, id: &str) -> OperatorResult<g::GoogleFile> {
        self.check()?;
        let session = self
            .access
            .session()
            .await
            .map_err(|e| self.error(e.kind))?;
        let result = self
            .access
            .client
            .files_get(
                &session,
                &g::FileId(id.into()),
                g::FileGetOptions {
                    fields: Some(g::FieldMask(FIELDS.into())),
                    supports_all_drives: Some(true),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| self.provider_error(e))?;
        if result.id.as_ref().is_none_or(|r| r.0 != id) {
            return Err(self.error(OperatorError::WrongScope));
        }
        Ok(result)
    }
    async fn within_root(&self, id: &str, directory: bool) -> OperatorResult<g::GoogleFile> {
        let first = self.get(id).await?;
        let mut current = first.clone();
        let mut seen = std::collections::HashSet::new();
        for _ in 0..256 {
            let current_id = current
                .id
                .as_ref()
                .ok_or_else(|| self.error(OperatorError::WrongScope))?;
            if current.trashed != Some(false)
                || directory && first.mime_type.as_ref().map(|m| m.0.as_str()) != Some(FOLDER)
            {
                return Err(self.error(OperatorError::PreconditionFailed));
            }
            if !seen.insert(current_id.0.clone()) {
                return Err(self.error(OperatorError::WrongScope));
            }
            if current_id.0 == self.scope.remote_root_id.0 {
                return Ok(first);
            }
            let parents = current
                .parents
                .as_ref()
                .filter(|p| p.len() == 1)
                .ok_or_else(|| self.error(OperatorError::WrongScope))?;
            current = self.get(&parents[0].0).await?;
            if current.mime_type.as_ref().map(|m| m.0.as_str()) != Some(FOLDER) {
                return Err(self.error(OperatorError::WrongScope));
            }
        }
        Err(self.error(OperatorError::WrongScope))
    }
    fn local_source(&self, expected: &LocalFile) -> OperatorResult<std::fs::File> {
        let (parent, _) = self.workspace.parent(&expected.relative_path)?;
        if Some(self.workspace.scope(&parent)?) != self.checkpoint.state.local_scope {
            return Err(self.error(OperatorError::WrongScope));
        }
        self.workspace.source(expected)
    }
    fn check_remote_source(
        &self,
        actual: &g::GoogleFile,
        expected: &RemoteFile,
    ) -> OperatorResult<()> {
        let valid = actual
            .id
            .as_ref()
            .is_some_and(|id| id.0 == expected.reference.file_id.0)
            && actual
                .version
                .as_ref()
                .is_some_and(|v| expected.version == MetadataValue::Known(v.clone()))
            && actual
                .size
                .as_ref()
                .and_then(|s| s.parse::<u64>().ok())
                .is_some_and(|n| expected.size == MetadataValue::Known(n))
            && actual
                .mime_type
                .as_ref()
                .is_some_and(|m| !m.0.starts_with("application/vnd.google-apps."))
            && actual.capabilities.as_ref().and_then(|c| c.can_download) == Some(true);
        if !valid {
            return Err(self.error(OperatorError::PreconditionFailed));
        }
        if let MetadataValue::Known(name) = &expected.name {
            if actual.name.as_ref() != Some(name) {
                return Err(self.error(OperatorError::PreconditionFailed));
            }
        }
        if let MetadataValue::Known(mime) = &expected.mime_type {
            if actual.mime_type.as_ref().map(|m| &m.0) != Some(mime) {
                return Err(self.error(OperatorError::PreconditionFailed));
            }
        }
        if let MetadataValue::Known(parents) = &expected.parent_ids {
            if actual
                .parents
                .as_ref()
                .map(|p| {
                    p.iter()
                        .map(|id| RemoteFileId(id.0.clone()))
                        .collect::<Vec<_>>()
                })
                .as_ref()
                != Some(parents)
            {
                return Err(self.error(OperatorError::PreconditionFailed));
            }
        }
        for (expected, actual) in [
            (&expected.md5_checksum, &actual.md5_checksum),
            (&expected.sha256_checksum, &actual.sha256_checksum),
        ] {
            if let MetadataValue::Known(value) = expected {
                if !actual
                    .as_ref()
                    .is_some_and(|a| a.eq_ignore_ascii_case(value))
                {
                    return Err(self.error(OperatorError::PreconditionFailed));
                }
            }
        }
        Ok(())
    }
    fn completed_result(&self) -> Option<TransferResult> {
        match &self.checkpoint.state.result {
            Some(ExecutionResult::Transfer(result)) => {
                let mut r = result.clone();
                r.bytes_transferred = 0;
                Some(r)
            }
            _ => None,
        }
    }
    fn finish(&mut self, result: TransferResult) -> OperatorResult<TransferResult> {
        if self.checkpoint.state.phase != CheckpointPhase::Verifying {
            let mut next = self.checkpoint.state.clone();
            next.phase = CheckpointPhase::Verifying;
            next.effect = ExecutionEffect::Applied;
            self.save(next).map_err(|mut e| {
                e.effect = ExecutionEffect::Applied;
                e.resource = Some(result.item.resource.clone());
                e
            })?;
        }
        let mut next = self.checkpoint.state.clone();
        next.phase = CheckpointPhase::Completed;
        next.effect = ExecutionEffect::Applied;
        next.result = Some(ExecutionResult::Transfer(result.clone()));
        self.save(next).map_err(|mut e| {
            e.effect = ExecutionEffect::Applied;
            e.resource = Some(result.item.resource.clone());
            e
        })?;
        Ok(result)
    }
}
const FOLDER: &str = "application/vnd.google-apps.folder";
const FIELDS: &str = "id,name,mimeType,parents,trashed,version,size,md5Checksum,sha256Checksum,createdTime,modifiedTime,headRevisionId,capabilities(canDownload,canAddChildren)";
