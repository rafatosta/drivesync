//! Immutable execution identity and recovery evidence, without scheduling or I/O.
//! A checkpoint records observed facts; it never authorizes a command or baseline.
use super::*;
use crate::models::{LocalEntryKind, LocalFile, MetadataValue, RemoteEntryKind, RemoteFile};
use serde::{Deserialize, Serialize};
use std::os::unix::ffi::OsStrExt;

/// Caller-generated unique key for one logical execution. Retries keep this key;
/// distinct intentional calls must use distinct keys even for identical commands.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ExecutionId(String);
impl ExecutionId {
    /// Accepts 1..=128 ASCII letters, digits, hyphens and underscores. No key is
    /// generated from a filename, resource identity, content or clock value.
    pub fn new(value: String) -> Result<Self, &'static str> {
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("invalid execution identity");
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for ExecutionId {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<ExecutionId> for String {
    fn from(value: ExecutionId) -> Self {
        value.0
    }
}

/// An explicitly qualified SHA-256 digest, not an unlabelled provider checksum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Sha256Digest(String);
impl Sha256Digest {
    pub fn new(hex: String) -> Result<Self, &'static str> {
        if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("invalid SHA-256 digest");
        }
        Ok(Self(hex.to_ascii_lowercase()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for Sha256Digest {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<Sha256Digest> for String {
    fn from(value: Sha256Digest) -> Self {
        value.0
    }
}

/// Exact source observation selected by the guarded plan, not a new observation
/// silently substituted at execution time. Unknown fields remain unknown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceEvidence {
    Local {
        metadata: Box<LocalFile>,
        sha256: Option<Sha256Digest>,
    },
    Remote {
        metadata: Box<RemoteFile>,
        revision: Option<String>,
    },
}

/// Required on every call, independently of optional progress/cancellation.
/// Sources backed by resources require evidence; inline bytes carry their own
/// immutable content in the command. Delete must refuse before inspecting this.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub id: ExecutionId,
    pub source: Option<SourceEvidence>,
}

/// Durable scope uses raw Unix root bytes, preserving non-UTF-8 paths. Matching
/// these values is not proof of filesystem confinement or remote ancestry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionScope {
    pub sync_id: crate::models::SyncId,
    pub account_id: crate::models::AccountId,
    pub local_root: Vec<u8>,
    pub remote_root_id: crate::models::RemoteFileId,
}
impl From<&OperatorScope> for ExecutionScope {
    fn from(scope: &OperatorScope) -> Self {
        Self {
            sync_id: scope.sync_id.clone(),
            account_id: scope.account_id.clone(),
            local_root: scope.local_root.as_os_str().as_bytes().to_vec(),
            remote_root_id: scope.remote_root_id.clone(),
        }
    }
}

/// Write-once binding. Reusing an ID with another scope, command, destination or
/// source observation is a conflict, never a new operation or implicit replan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionIntent {
    pub context: ExecutionContext,
    pub scope: ExecutionScope,
    pub command: OperationCommand,
}
impl ExecutionIntent {
    /// Checks resource-backed source binding and sufficient version evidence.
    /// Executors must still revalidate live scope, source, destination, capabilities
    /// and target preconditions immediately before effects/publication.
    pub fn validate_source(&self) -> bool {
        if matches!(self.command, OperationCommand::CreateDirectory { .. }) {
            return match &self.context.source {
                None => true,
                Some(SourceEvidence::Local {
                    metadata,
                    sha256: None,
                }) => {
                    metadata.validate().is_ok()
                        && metadata.kind == MetadataValue::Known(LocalEntryKind::Directory)
                        && matches!(metadata.storage.inode, MetadataValue::Known(_))
                        && matches!(metadata.storage.filesystem_device, MetadataValue::Known(_))
                }
                Some(SourceEvidence::Remote {
                    metadata,
                    revision: None,
                }) => {
                    metadata.validate().is_ok()
                        && metadata.kind == MetadataValue::Known(RemoteEntryKind::Directory)
                        && metadata.trashed == MetadataValue::Known(false)
                        && matches!(&metadata.version, MetadataValue::Known(v) if !v.is_empty())
                }
                _ => false,
            };
        }
        if let OperationCommand::Rename { target, .. }
        | OperationCommand::Move { target, .. }
        | OperationCommand::Trash { target, .. }
        | OperationCommand::Restore { target, .. } = &self.command
        {
            return match (target, &self.context.source) {
                (
                    ResourceRef::Local {
                        relative_path,
                        local_id: Some(id),
                    },
                    Some(SourceEvidence::Local { metadata, .. }),
                ) => {
                    metadata.validate().is_ok()
                        && metadata.relative_path == *relative_path
                        && metadata.local_id.as_ref() == Some(id)
                        && matches!(
                            metadata.kind,
                            MetadataValue::Known(
                                LocalEntryKind::Regular | LocalEntryKind::Directory
                            )
                        )
                        && matches!(metadata.storage.inode, MetadataValue::Known(_))
                        && matches!(metadata.storage.filesystem_device, MetadataValue::Known(_))
                }
                (
                    ResourceRef::Remote {
                        provider_id,
                        reference,
                    },
                    Some(SourceEvidence::Remote { metadata, .. }),
                ) => {
                    metadata.validate().is_ok()
                        && metadata.provider == *provider_id
                        && metadata.reference == *reference
                        && matches!(
                            metadata.kind,
                            MetadataValue::Known(
                                RemoteEntryKind::File | RemoteEntryKind::Directory
                            )
                        )
                        && matches!(&metadata.version,MetadataValue::Known(v) if !v.is_empty())
                }
                _ => false,
            };
        }
        let source = match &self.command {
            OperationCommand::Upload { source, .. } | OperationCommand::Download { source, .. } => {
                Some(source.clone())
            }
            OperationCommand::CreateFile { content, .. } => {
                content.as_ref().and_then(content_resource)
            }
            OperationCommand::UpdateContent { source, .. } => content_resource(source),
            _ => None,
        };
        match (source, &self.context.source) {
            (None, None) => true,
            (
                Some(ResourceRef::Local {
                    relative_path,
                    local_id,
                }),
                Some(SourceEvidence::Local { metadata, .. }),
            ) => {
                metadata.relative_path == relative_path
                    && metadata.local_id == local_id
                    && stable_local(metadata)
            }
            (
                Some(ResourceRef::Remote {
                    provider_id,
                    reference,
                }),
                Some(SourceEvidence::Remote { metadata, revision }),
            ) => {
                metadata.validate().is_ok()
                    && metadata.reference == reference
                    && metadata.provider == provider_id
                    && metadata.kind == MetadataValue::Known(RemoteEntryKind::File)
                    && metadata.trashed == MetadataValue::Known(false)
                    && matches!(metadata.size, MetadataValue::Known(_))
                    && (matches!(&metadata.version, MetadataValue::Known(v) if !v.trim().is_empty())
                        || revision.as_ref().is_some_and(|v| !v.trim().is_empty()))
                    && revision.as_ref().is_none_or(|v| !v.trim().is_empty())
            }
            _ => false,
        }
    }
}
fn content_resource(source: &ContentSource) -> Option<ResourceRef> {
    match source {
        ContentSource::LocalFile {
            relative_path,
            local_id,
        } => Some(ResourceRef::Local {
            relative_path: relative_path.clone(),
            local_id: local_id.clone(),
        }),
        ContentSource::RemoteFile { resource } => Some(resource.clone()),
        ContentSource::Bytes { .. } => None,
    }
}
fn stable_local(file: &LocalFile) -> bool {
    file.validate().is_ok()
        && file.kind == MetadataValue::Known(LocalEntryKind::Regular)
        && file.storage.hard_link_count == MetadataValue::Known(1)
        && matches!(file.storage.size_bytes, MetadataValue::Known(_))
        && matches!(file.storage.inode, MetadataValue::Known(_))
        && matches!(file.storage.filesystem_device, MetadataValue::Known(_))
        && matches!(file.times.modified_at, MetadataValue::Known(_))
        && matches!(file.times.changed_at, MetadataValue::Known(_))
}

/// Verified private staging, never the authority to replace a destination.
/// A filesystem publication may change the inode: the result must report the
/// identity actually installed, not blindly copy this or the previous local ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StagingEvidence {
    pub metadata: Box<LocalFile>,
    pub sha256: Sha256Digest,
}

/// Descriptor-observed identity; birth time prevents inode reuse when available.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhysicalIdentity {
    pub device: u64,
    pub inode: u64,
    pub birth: Option<crate::models::LocalTimestamp>,
}
/// Physical directories pinned before staging or publication, and compared on retry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalExecutionScope {
    pub root: PhysicalIdentity,
    pub parent: PhysicalIdentity,
    pub staging_directory: PhysicalIdentity,
}

/// Sensitive provider recovery bytes, stored only in the private Inventory DB.
/// Debug is redacted. This is not an OAuth credential or a caller-selected URL;
/// Providers must validate server-issued session destinations on every use.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<u8>", into = "Vec<u8>")]
pub struct ResumeToken(Vec<u8>);
impl ResumeToken {
    pub fn new(bytes: Vec<u8>) -> Result<Self, &'static str> {
        if bytes.is_empty() || bytes.len() > 8192 {
            return Err("invalid resume token");
        }
        Ok(Self(bytes))
    }
    /// Only the selected provider specialist may interpret these sensitive bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}
impl std::fmt::Debug for ResumeToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ResumeToken([REDACTED])")
    }
}
impl TryFrom<Vec<u8>> for ResumeToken {
    type Error = &'static str;
    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<ResumeToken> for Vec<u8> {
    fn from(value: ResumeToken) -> Self {
        value.0
    }
}

/// Persist Executing/Unknown before dispatching any potentially mutating call.
/// Opening a checkpoint never resets that uncertainty or automatically retries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckpointPhase {
    Prepared,
    Executing,
    Verifying,
    Completed,
}

/// The verified result of the original execution, including transfer integrity.
/// Reading it does not transfer those bytes again: a future retry response must
/// distinguish this historical record from bytes moved by the current call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionResult {
    Item(ItemResult),
    Transfer(TransferResult),
}
impl ExecutionResult {
    pub fn item(&self) -> &ItemResult {
        match self {
            Self::Item(item) => item,
            Self::Transfer(transfer) => &transfer.item,
        }
    }
}

/// Recovery facts, distinct from UI progress and from queue state. Offsets are
/// server-confirmed cumulative bytes, not bytes transferred by the current call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionState {
    pub local_scope: Option<LocalExecutionScope>,
    /// Changes only through explicit expired-session reconciliation in Inventory.
    pub session_generation: u32,
    pub phase: CheckpointPhase,
    pub effect: ExecutionEffect,
    pub staging: Option<StagingEvidence>,
    pub reserved_remote: Option<ResourceRef>,
    pub resume_token: Option<ResumeToken>,
    pub confirmed_offset: u64,
    pub result: Option<ExecutionResult>,
}
impl ExecutionState {
    pub fn prepared() -> Self {
        Self {
            local_scope: None,
            session_generation: 0,
            phase: CheckpointPhase::Prepared,
            effect: ExecutionEffect::NotApplied,
            staging: None,
            reserved_remote: None,
            resume_token: None,
            confirmed_offset: 0,
            result: None,
        }
    }
    /// Structural invariants only. Recorded verification is supplied by the
    /// future executor; storage cannot prove remote or filesystem observations.
    pub fn is_valid(&self) -> bool {
        let phase = match self.phase {
            CheckpointPhase::Prepared => {
                self.effect == ExecutionEffect::NotApplied && self.result.is_none()
            }
            CheckpointPhase::Executing => {
                self.effect == ExecutionEffect::Unknown && self.result.is_none()
            }
            CheckpointPhase::Verifying => {
                self.effect != ExecutionEffect::NotApplied && self.result.is_none()
            }
            CheckpointPhase::Completed => {
                self.effect == ExecutionEffect::Applied
                    && self
                        .result
                        .as_ref()
                        .is_some_and(|r| r.item().verification == VerificationStatus::Verified)
            }
        };
        phase
            && (self.confirmed_offset == 0 || self.resume_token.is_some())
            && (self.resume_token.is_none() || self.reserved_remote.is_some())
            && self
                .reserved_remote
                .as_ref()
                .is_none_or(|r| matches!(r, ResourceRef::Remote { .. }))
            && self
                .staging
                .as_ref()
                .is_none_or(|s| stable_local(&s.metadata))
    }
    /// Conservative monotonic transition: uncertainty cannot be erased by retry.
    /// Replacing an expired session requires reset_expired_upload; generic saves
    /// deliberately cannot reset an existing session, generation or offset.
    pub fn can_follow(&self, previous: &Self) -> bool {
        use CheckpointPhase::*;
        self.is_valid()
            && previous.is_valid()
            && matches!(
                (previous.phase, self.phase),
                (Prepared, Prepared | Executing)
                    | (Executing, Executing | Verifying)
                    | (Verifying, Verifying | Completed)
            )
            && (previous.effect != ExecutionEffect::Applied
                || self.effect == ExecutionEffect::Applied)
            && previous
                .staging
                .as_ref()
                .is_none_or(|s| self.staging.as_ref() == Some(s))
            && previous
                .reserved_remote
                .as_ref()
                .is_none_or(|r| self.reserved_remote.as_ref() == Some(r))
            && previous
                .resume_token
                .as_ref()
                .is_none_or(|t| self.resume_token.as_ref() == Some(t))
            && self.confirmed_offset >= previous.confirmed_offset
            && self.session_generation == previous.session_generation
            && previous
                .local_scope
                .as_ref()
                .is_none_or(|s| self.local_scope.as_ref() == Some(s))
    }
}

/// Revision is a compare-and-swap token, not a worker lease or execution permit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionCheckpoint {
    pub intent: ExecutionIntent,
    pub revision: u64,
    pub state: ExecutionState,
}
/// No recovery path treats an unknown effect as an invitation to dispatch again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    ValidateBeforeStart,
    ReconcileEffect,
    ReturnVerifiedResult,
}
impl ExecutionCheckpoint {
    pub fn recovery_action(&self) -> RecoveryAction {
        match self.state.phase {
            CheckpointPhase::Prepared => RecoveryAction::ValidateBeforeStart,
            CheckpointPhase::Completed => RecoveryAction::ReturnVerifiedResult,
            _ => RecoveryAction::ReconcileEffect,
        }
    }
}
#[derive(Debug, Clone)]
pub struct PrepareExecutionRequest {
    pub intent: ExecutionIntent,
}
#[derive(Debug, Clone)]
pub struct GetExecutionRequest {
    pub id: ExecutionId,
}
#[derive(Debug, Clone)]
pub struct SaveExecutionRequest {
    pub id: ExecutionId,
    pub expected_revision: u64,
    pub state: ExecutionState,
}

/// Supplied only after Providers confirmed this exact session expired AND a
/// fresh lookup did not find the reserved creation ID. No new remote ID is made.
/// Missing access is never sufficient evidence for recreating an existing item.
#[derive(Debug, Clone)]
pub struct ResetExpiredUploadRequest {
    pub id: ExecutionId,
    pub expected_revision: u64,
    pub expired_session: ResumeToken,
    pub reserved_remote: ResourceRef,
}

/// Blocking technical persistence port, implemented by Inventory. Run off GTK
/// and async runtime threads. No queue, SQL, worker, authorization or baseline
/// belongs to the Operator. A failed save forbids dependent effects. Matching
/// retries return the durable checkpoint; conflicting writes must fail closed.
pub trait ExecutionJournal: Send + Sync {
    fn prepare_execution(
        &self,
        request: PrepareExecutionRequest,
    ) -> OperatorResult<ExecutionCheckpoint>;
    fn get_execution(&self, request: GetExecutionRequest) -> OperatorResult<ExecutionCheckpoint>;
    fn save_execution(&self, request: SaveExecutionRequest) -> OperatorResult<ExecutionCheckpoint>;
    fn reset_expired_upload(
        &self,
        request: ResetExpiredUploadRequest,
    ) -> OperatorResult<ExecutionCheckpoint>;
}
