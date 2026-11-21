//! Data types returned by the local detector interface.

use crate::models::{
    FileEvent, LocalEntryKind, LocalFileId, LocalPresenceEvidence, LocalRelativePath,
    LocalTimestamp, SyncId, Timestamp,
};
use std::collections::BTreeMap;

/// Synchronization root identity used by local observations.
pub type Id = SyncId;

/// Validated path relative to the synchronization root.
pub type Path = LocalRelativePath;

/// State of local presence observed for an item.
pub type FileState = LocalPresenceEvidence;

/// Local filesystem entry kind.
pub type FileKind = LocalEntryKind;

/// Auxiliary filesystem identity for a local item.
pub type LocalIdentity = LocalFileId;

/// Event context that caused or accompanied the local read.
pub type ObservationEvent = FileEvent;

/// Normalized checksum evidence.
///
/// `algorithm` identifies the digest algorithm and `digest` stores its
/// normalized textual value. This type does not compute or validate digests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checksum {
    pub algorithm: String,
    pub digest: String,
}

/// Capabilities reported for the local item.
///
/// Keys identify capabilities and values record whether each is available.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LocalCapabilities {
    pub values: BTreeMap<String, bool>,
}

/// Local item state and the local evidence collected for one observation.
///
/// This is a data contract only. It contains no baseline or remote data and
/// does not perform collection, enrichment, validation, or filesystem access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalObservation {
    pub sync_root_id: Id,
    pub state: FileState,
    pub path: Option<Path>,
    pub previous_path: Option<Path>,
    pub name: Option<String>,
    pub parent: Option<Path>,
    pub kind: Option<FileKind>,
    pub identity: Option<LocalIdentity>,
    pub size: Option<u64>,
    pub modified_at: Option<LocalTimestamp>,
    pub changed_at: Option<LocalTimestamp>,
    pub checksum: Option<Checksum>,
    pub capabilities: Option<LocalCapabilities>,
    pub observed_at: Timestamp,
    pub coverage_complete: bool,
    pub event: Option<ObservationEvent>,
}
