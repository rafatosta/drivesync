//! Private original remote observations, independent of SQL and delivery adaptation.
use crate::models::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Original JSON item plus stable collection context, never normalized metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawRemoteObservation {
    pub id: FileId,
    pub sync_id: SyncId,
    pub observed_at: Timestamp,
    pub account_id: AccountId,
    pub provider: ProviderId,
    pub resource_id: RemoteFileId,
    pub source: RemoteObservationSource,
    pub original: Value,
}
impl RawRemoteObservation {
    /// Checks collection identity and object shape without constructing RemoteFile.
    pub(crate) fn validate(&self) -> ModelResult<()> {
        if self.id.0.trim().is_empty()
            || self.sync_id.0.trim().is_empty()
            || self.account_id.0.trim().is_empty()
            || self.provider.0.trim().is_empty()
            || self.resource_id.0.trim().is_empty()
            || !self.original.is_object()
        {
            return Err(ModelError::InconsistentEvent);
        }
        crate::detectors::remote::validate_original(self)
    }
}
/// Durable record: common local/technical occurrence or original remote item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ObservationRecord {
    Common(Box<File>),
    Remote(Box<RawRemoteObservation>),
}
impl ObservationRecord {
    pub(crate) fn validate(&self) -> ModelResult<()> {
        match self {
            Self::Common(file) => file.validate(),
            Self::Remote(raw) => raw.validate(),
        }
    }
    pub(crate) fn id(&self) -> &FileId {
        match self {
            Self::Common(file) => &file.id,
            Self::Remote(raw) => &raw.id,
        }
    }
    pub(crate) fn sync_id(&self) -> &SyncId {
        match self {
            Self::Common(file) => &file.sync_id,
            Self::Remote(raw) => &raw.sync_id,
        }
    }
    /// Called only for delivery or diagnostics after persistence in the production queue.
    pub(crate) fn into_file(self) -> Result<File, crate::detectors::RemoteFailure> {
        match self {
            Self::Common(file) => Ok(*file),
            Self::Remote(raw) => crate::detectors::remote::extract(&raw),
        }
    }
}
