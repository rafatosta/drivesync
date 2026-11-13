//! Shared event envelope; local implementation does not invent a remote resource model.
use super::*;
use std::convert::Infallible;

/// Provider resource identity, always qualified by its owning account.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RemoteFileId(pub String);
/// Remote identity context only, without remote attributes or provider access.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RemoteFileRef {
    pub account_id: AccountId,
    pub file_id: RemoteFileId,
}
/// Classification of an observation, never a synchronization command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileEventKind {
    Created,
    Modified,
    Deleted,
    Renamed,
    Moved,
    MetadataChanged,
    RecheckRequired,
    ScopeUnavailable,
}
/// Known location evidence, separate from metadata snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileLocation {
    Local(LocalRelativePath),
    Remote {
        reference: RemoteFileRef,
        parent_id: Option<RemoteFileId>,
        name: Option<String>,
    },
}
/// Why a producer cannot deliver a fully classified occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationUncertainty {
    CaptureStarted,
    CaptureRebuilt,
    CoverageIncomplete,
    NativeOverflow,
    ResourceLimit,
    UnpairedMove,
    UnclassifiedSignal,
    ObservationUnstable,
    WriteInProgress,
    ConflictingSequence,
}
/// Location context before and after an observed occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEvent {
    pub kind: FileEventKind,
    /// Required for RecheckRequired; absent for classified or unavailable events.
    pub uncertainty: Option<ObservationUncertainty>,
    pub previous: Option<FileLocation>,
    pub current: Option<FileLocation>,
}
/// Read coverage of the declared scope, not proof of deletion or content integrity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationCoverage {
    Complete,
    Partial,
    Unavailable,
}
/// Resource or scope requiring re-evaluation, including an unavailable whole root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileScope {
    Local(LocalRelativePath),
    Remote(RemoteFileRef),
    WholeSync,
}
/// One complete metadata object. R is supplied only after its remote model is reviewed.
/// The default Infallible makes remote data unconstructible in this local-only stage.
#[allow(
    clippy::large_enum_variant,
    reason = "Preserve the documented by-value LocalFile payload without projecting its fields"
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileData<R = Infallible> {
    Local(LocalFile),
    Remote(R),
}
/// Self-contained occurrence, keeping identity and payload unchanged during retry.
///
/// The seven fields and two data variants are shared with the documented remote
/// flow. A generic remote payload avoids a fictitious RemoteFile or an alternate
/// local event representation. No persistence format or remote validator is implied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File<R = Infallible> {
    pub id: FileId,
    pub sync_id: SyncId,
    pub event: FileEvent,
    pub observed_at: Timestamp,
    pub coverage: ObservationCoverage,
    pub scope: FileScope,
    pub data: Option<FileData<R>>,
}
impl File {
    /// Validates the local envelope without I/O, inference, mutation or ID generation.
    /// A future reviewed remote payload requires its own typed validation.
    pub fn validate(&self) -> ModelResult<()> {
        if self.id.0.trim().is_empty() || self.sync_id.0.trim().is_empty() {
            return Err(ModelError::EmptyIdentity);
        }
        let invalid = || Err(ModelError::InconsistentEvent);
        if (self.event.kind == FileEventKind::RecheckRequired) != self.event.uncertainty.is_some() {
            return invalid();
        }
        let data = match &self.data {
            Some(FileData::Local(file)) => {
                file.validate()?;
                Some(file)
            }
            Some(FileData::Remote(value)) => match *value {},
            None => None,
        };
        match &self.scope {
            FileScope::WholeSync => {
                if data.is_some()
                    || self.event.previous.is_some()
                    || self.event.current.is_some()
                    || !matches!(
                        self.event.kind,
                        FileEventKind::RecheckRequired | FileEventKind::ScopeUnavailable
                    )
                {
                    return invalid();
                }
            }
            FileScope::Remote(_) => return invalid(),
            FileScope::Local(path) => {
                if data.is_some_and(|f| &f.relative_path != path) {
                    return invalid();
                }
                match &self.event.current {
                    Some(FileLocation::Local(current)) if current == path => {}
                    None => {}
                    _ => return invalid(),
                }
                if matches!(self.event.previous, Some(FileLocation::Remote { .. })) {
                    return invalid();
                }
                if self.event.current.is_none()
                    && self
                        .event
                        .previous
                        .as_ref()
                        .is_some_and(|location| location != &FileLocation::Local(path.clone()))
                {
                    return invalid();
                }
            }
        }
        match self.event.kind {
            FileEventKind::Created
            | FileEventKind::Modified
            | FileEventKind::MetadataChanged
            | FileEventKind::Renamed
            | FileEventKind::Moved => {
                if data.is_none()
                    || self.event.current.is_none()
                    || self.coverage == ObservationCoverage::Unavailable
                {
                    return invalid();
                }
                if matches!(
                    self.event.kind,
                    FileEventKind::Renamed | FileEventKind::Moved
                ) && (self.event.previous.is_none() || self.event.previous == self.event.current)
                {
                    return invalid();
                }
            }
            FileEventKind::Deleted => {
                if data.is_some()
                    || self.event.previous.is_none()
                    || self.event.current.is_some()
                    || self.coverage == ObservationCoverage::Unavailable
                {
                    return invalid();
                }
            }
            FileEventKind::ScopeUnavailable => {
                if data.is_some()
                    || self.event.current.is_some()
                    || self.coverage != ObservationCoverage::Unavailable
                {
                    return invalid();
                }
            }
            FileEventKind::RecheckRequired => {
                if data.is_some()
                    && (self.event.current.is_none()
                        || self.coverage == ObservationCoverage::Unavailable)
                {
                    return invalid();
                }
            }
        }
        if self.coverage == ObservationCoverage::Complete
            && data.is_some_and(|f| !f.basic_profile_complete())
        {
            return invalid();
        }
        Ok(())
    }
}
