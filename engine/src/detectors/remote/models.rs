//! Data types returned by the remote detector interface.

use crate::models::{
    AccountId, Checksum, FileEvent, FileState as SharedFileState, RemoteFileId, Timestamp,
};
use std::collections::BTreeMap;

/// Provider resource identifier used by remote observations.
pub type RemoteId = RemoteFileId;

/// Presence state reported for a remote item.
pub type FileState = SharedFileState;

/// Event context associated with a remote read.
pub type ObservationEvent = FileEvent;

/// Capabilities reported by the remote provider for an item.
///
/// Keys identify provider capabilities and values record whether each is
/// available. This type does not infer capabilities from account permissions.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RemoteCapabilities {
    pub values: BTreeMap<String, bool>,
}

/// Remote item state and the provider evidence collected for one observation.
///
/// This is a data contract only. It contains no baseline or local data and does
/// not perform requests, enrichment, validation, or synchronization decisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteObservation {
    pub account_id: AccountId,
    pub state: FileState,
    pub remote_id: Option<RemoteId>,
    pub name: Option<String>,
    pub parents: Option<Vec<RemoteId>>,
    pub mime_type: Option<String>,
    pub size: Option<u64>,
    pub checksum: Option<Checksum>,
    pub version: Option<u64>,
    pub modified_time: Option<Timestamp>,
    pub trashed: Option<bool>,
    pub capabilities: Option<RemoteCapabilities>,
    pub observed_at: Timestamp,
    pub coverage_complete: bool,
    pub page_token: Option<String>,
    pub event: Option<ObservationEvent>,
}
