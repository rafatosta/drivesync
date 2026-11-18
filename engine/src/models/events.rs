//! Shared local and remote observation envelopes with pure validation.
use super::*;
use serde::{Deserialize, Serialize};

/// Provider resource identity, always qualified by its owning account.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteFileId(pub String);
/// Remote identity context only, without remote attributes or provider access.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteFileRef {
    pub account_id: AccountId,
    pub file_id: RemoteFileId,
}
/// Action reported by an observation source, never a synchronization command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileEventAction {
    /// A source signal was received but did not match one supported action.
    Unclassified,
    /// An initial listing or capture supplied a snapshot without asserting creation.
    SnapshotObserved,
    /// A provider reported a state change without identifying its dimension.
    StateChanged,
    Created,
    Modified,
    Deleted,
    /// The item left this observed parent; a matching destination may be unknown.
    MoveFrom,
    /// The item entered this observed parent; a matching source may be unknown.
    MoveTo,
    Renamed,
    Moved,
    MetadataChanged,
    /// A provider reports that the item left the caller's visible scope.
    RemovedFromScope,
    RecheckRequired,
    ScopeUnavailable,
}
/// Known location evidence, separate from metadata snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileLocation {
    Local(LocalRelativePath),
    Remote {
        reference: RemoteFileRef,
        parent_id: Option<RemoteFileId>,
        name: Option<String>,
    },
}
/// Why a producer cannot deliver a fully classified occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
/// Linux notification evidence, without pairing or action classification.
/// The cookie is meaningful only within this capture session; zero means unpaired.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalObservation {
    pub capture_id: String,
    pub mask: u32,
    pub cookie: u32,
}
/// Collection operation that supplied a remote observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteObservationSource {
    Capture,
    Changes,
}
/// Provider change evidence, preserved without classifying a filesystem action.
/// Missing fields remain absent; removed is not proof of deletion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteObservation {
    pub source: RemoteObservationSource,
    pub removed: Option<bool>,
    pub time: Option<String>,
    pub change_type: Option<String>,
    pub drive_id: Option<String>,
}
/// Location context before and after an observed occurrence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileEvent {
    /// Original local notification, present only for native local observations.
    pub local: Option<LocalObservation>,
    /// Raw collection evidence, present only for uninterpreted remote observations.
    pub remote: Option<RemoteObservation>,
    pub action: FileEventAction,
    /// Required for RecheckRequired; absent for classified or unavailable events.
    pub uncertainty: Option<ObservationUncertainty>,
    pub previous: Option<FileLocation>,
    pub current: Option<FileLocation>,
}
/// Read coverage of the declared scope, not proof of deletion or content integrity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationCoverage {
    Complete,
    Partial,
    Unavailable,
}
/// Resource or scope requiring re-evaluation, including an unavailable whole root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileScope {
    Local(LocalRelativePath),
    Remote(RemoteFileRef),
    WholeSync,
}
/// One complete metadata object, with the reviewed remote observation profile.
#[allow(
    clippy::large_enum_variant,
    reason = "Preserve the documented by-value LocalFile payload without projecting its fields"
)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileData<R = RemoteFile> {
    Local(LocalFile),
    Remote(R),
}
/// Self-contained occurrence, keeping identity and payload unchanged during retry.
///
/// Local and remote producers share identity, context and complete metadata payloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct File<R = RemoteFile> {
    pub id: FileId,
    pub sync_id: SyncId,
    pub event: FileEvent,
    pub observed_at: Timestamp,
    pub coverage: ObservationCoverage,
    pub scope: FileScope,
    pub data: Option<FileData<R>>,
}
impl File {
    /// Validates identities and observation evidence without I/O or inferred attributes.
    pub fn validate(&self) -> ModelResult<()> {
        if self.id.0.trim().is_empty() || self.sync_id.0.trim().is_empty() {
            return Err(ModelError::EmptyIdentity);
        }
        let invalid = || Err(ModelError::InconsistentEvent);
        if (self.event.action == FileEventAction::RecheckRequired)
            != self.event.uncertainty.is_some()
        {
            return invalid();
        }
        let native = self.event.local.is_some() || self.event.remote.is_some();
        if (self.event.action == FileEventAction::Unclassified && !native)
            || (self.event.local.is_some() && self.event.remote.is_some())
            || (self.event.local.is_some() && !Self::local_action(self.event.action))
            || (self.event.remote.is_some() && !Self::remote_action(self.event.action))
        {
            return invalid();
        }
        if self.event.action == FileEventAction::Unclassified
            && (self.event.previous.is_some()
                || self.coverage != ObservationCoverage::Partial
                || matches!(self.scope, FileScope::WholeSync))
        {
            return invalid();
        }
        if self.event.action == FileEventAction::RemovedFromScope
            && self.event.remote.as_ref().is_none_or(|event| {
                event.source != RemoteObservationSource::Changes || event.removed != Some(true)
            })
        {
            return invalid();
        }
        if self
            .event
            .local
            .as_ref()
            .is_some_and(|signal| signal.capture_id.trim().is_empty() || signal.mask == 0)
        {
            return invalid();
        }
        let data = match &self.data {
            Some(FileData::Local(file)) => {
                file.validate()?;
                Some(file)
            }
            Some(FileData::Remote(remote)) => {
                remote.validate()?;
                return self.validate_remote(remote);
            }
            None => None,
        };
        match &self.scope {
            FileScope::WholeSync => {
                if data.is_some()
                    || self.event.previous.is_some()
                    || self.event.current.is_some()
                    || !matches!(
                        self.event.action,
                        FileEventAction::RecheckRequired | FileEventAction::ScopeUnavailable
                    )
                {
                    return invalid();
                }
            }
            FileScope::Remote(reference) => {
                if reference.account_id.0.trim().is_empty()
                    || reference.file_id.0.trim().is_empty()
                    || self.data.is_some()
                    || !matches!(
                        self.event.action,
                        FileEventAction::Unclassified
                            | FileEventAction::RemovedFromScope
                            | FileEventAction::RecheckRequired
                            | FileEventAction::ScopeUnavailable
                    )
                    || self.event.current.is_some()
                    || self.event.previous.is_some()
                    || (self.event.action == FileEventAction::RemovedFromScope
                        && self.event.remote.as_ref().is_none_or(|event| {
                            event.source != RemoteObservationSource::Changes
                                || event.removed != Some(true)
                        }))
                {
                    return invalid();
                }
            }
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
        match self.event.action {
            FileEventAction::Unclassified
            | FileEventAction::SnapshotObserved
            | FileEventAction::StateChanged
            | FileEventAction::RemovedFromScope => {}
            FileEventAction::Created
            | FileEventAction::Modified
            | FileEventAction::MetadataChanged
            | FileEventAction::Renamed
            | FileEventAction::Moved
            | FileEventAction::MoveTo => {
                if self.event.current.is_none() || self.coverage == ObservationCoverage::Unavailable
                {
                    return invalid();
                }
                if matches!(
                    self.event.action,
                    FileEventAction::Renamed | FileEventAction::Moved
                ) && (self.event.previous.is_none() || self.event.previous == self.event.current)
                {
                    return invalid();
                }
            }
            FileEventAction::Deleted => {
                if self.event.previous.is_none()
                    || self.coverage == ObservationCoverage::Unavailable
                {
                    return invalid();
                }
            }
            FileEventAction::MoveFrom => {
                if self.event.previous.is_none()
                    || self.coverage == ObservationCoverage::Unavailable
                {
                    return invalid();
                }
            }
            FileEventAction::ScopeUnavailable => {
                if data.is_some()
                    || self.event.current.is_some()
                    || self.coverage != ObservationCoverage::Unavailable
                {
                    return invalid();
                }
            }
            FileEventAction::RecheckRequired => {
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

    fn local_action(action: FileEventAction) -> bool {
        matches!(
            action,
            FileEventAction::Unclassified
                | FileEventAction::Created
                | FileEventAction::Modified
                | FileEventAction::Deleted
                | FileEventAction::MoveFrom
                | FileEventAction::MoveTo
                | FileEventAction::Renamed
                | FileEventAction::Moved
                | FileEventAction::MetadataChanged
        )
    }

    fn remote_action(action: FileEventAction) -> bool {
        matches!(
            action,
            FileEventAction::Unclassified
                | FileEventAction::SnapshotObserved
                | FileEventAction::StateChanged
                | FileEventAction::RemovedFromScope
                | FileEventAction::Created
                | FileEventAction::Modified
                | FileEventAction::Deleted
                | FileEventAction::Renamed
                | FileEventAction::Moved
                | FileEventAction::MetadataChanged
        )
    }
}

impl File {
    fn validate_remote(&self, remote: &RemoteFile) -> ModelResult<()> {
        let invalid = || Err(ModelError::InconsistentEvent);
        if self.scope != FileScope::Remote(remote.reference.clone())
            || self.coverage == ObservationCoverage::Unavailable
            || matches!(
                self.event.action,
                FileEventAction::Deleted
                    | FileEventAction::RemovedFromScope
                    | FileEventAction::ScopeUnavailable
            )
        {
            return invalid();
        }
        match self.event.action {
            FileEventAction::SnapshotObserved
                if self
                    .event
                    .remote
                    .as_ref()
                    .is_none_or(|event| event.source != RemoteObservationSource::Capture) =>
            {
                return invalid();
            }
            FileEventAction::StateChanged
                if self.event.remote.as_ref().is_none_or(|event| {
                    event.source != RemoteObservationSource::Changes || event.removed == Some(true)
                }) =>
            {
                return invalid();
            }
            FileEventAction::RemovedFromScope => return invalid(),
            _ => {}
        }
        for location in [&self.event.previous, &self.event.current]
            .into_iter()
            .flatten()
        {
            match location {
                FileLocation::Remote {
                    reference,
                    parent_id,
                    ..
                } if reference == &remote.reference
                    && parent_id.as_ref().is_none_or(|id| !id.0.trim().is_empty()) => {}
                _ => return invalid(),
            }
        }
        let Some(FileLocation::Remote {
            name, parent_id, ..
        }) = &self.event.current
        else {
            return invalid();
        };
        if let MetadataValue::Known(expected) = &remote.name {
            if name.as_ref() != Some(expected) {
                return invalid();
            }
        }
        if let (Some(parent), MetadataValue::Known(parents)) = (parent_id, &remote.parent_ids) {
            if !parents.contains(parent) {
                return invalid();
            }
        }
        if matches!(
            self.event.action,
            FileEventAction::Renamed | FileEventAction::Moved
        ) && (self.event.previous.is_none() || self.event.previous == self.event.current)
        {
            return invalid();
        }
        Ok(())
    }
}
