//! Provider-neutral observation envelopes and source-specific evidence.

use crate::models::{
    AccountId, Checksum, DateTime, Evidence, FileKind, FileState, LocalCapabilities, LocalIdentity,
    ObservationEvent, Path, ProviderId, RemoteCapabilities, RemoteId, SyncRootId,
};
use serde::{Deserialize, Serialize};

/// One observation emitted to the Context, preserving local source provenance.
///
/// This value describes observed facts only. A scanner snapshot does not imply item creation,
/// and no observation variant represents an authorized synchronization command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Observation {
    /// Evidence collected from the local filesystem.
    Local {
        /// Distinguishes initial scanning from continuous watching.
        source: LocalObservationSource,
        /// Local filesystem evidence.
        data: LocalObservation,
    },
    /// Evidence collected from a configured remote provider.
    Remote(RemoteObservation),
}

/// Origin of a local observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum LocalObservationSource {
    /// Initial or deliberate filesystem scan.
    Scanner,
    /// Continuous filesystem watcher.
    Watcher,
}

/// Completeness of the declared scope examined by an observation producer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ObservationCoverage {
    /// The declared scope was examined with sufficient coverage.
    Complete,
    /// Only part of the declared scope was examined.
    Partial,
    /// The declared scope could not be observed.
    Unavailable,
}

/// Local filesystem evidence for one item within a sync root.
///
/// This model contains no baseline, remote state, or synchronization decision. Paths are
/// relative to sync_root_id; previous_path is evidence of a prior location, not an action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LocalObservation {
    /// Synchronization root that scopes this local item.
    pub sync_root_id: SyncRootId,
    /// Current observability state; only confirmed absence uses Absent.
    pub state: FileState,
    /// Current path relative to the synchronization root, when known.
    pub path: Option<Path>,
    /// Previous path relative to the synchronization root, when known.
    pub previous_path: Option<Path>,
    /// Current item name evidence.
    pub name: Evidence<String>,
    /// Parent path relative to the synchronization root.
    pub parent: Evidence<Path>,
    /// Filesystem item kind.
    pub kind: Evidence<FileKind>,
    /// Auxiliary filesystem identity evidence.
    pub identity: Evidence<LocalIdentity>,
    /// File size in bytes when applicable.
    pub size: Evidence<u64>,
    /// Local content modification time (mtime), when available.
    pub modified_at: Evidence<DateTime>,
    /// Local metadata/state change time (ctime), not creation time.
    pub changed_at: Evidence<DateTime>,
    /// Content digest evidence.
    pub checksum: Evidence<Checksum>,
    /// Local operations the filesystem permits, when known.
    pub capabilities: Evidence<LocalCapabilities>,
    /// Time at which this evidence was read.
    pub observed_at: DateTime,
    /// Coverage of the local scope examined for this observation.
    pub coverage: ObservationCoverage,
    /// Optional normalized signal associated with the read.
    pub event: Option<ObservationEvent>,
}

/// Provider-neutral remote evidence for one item within an account and sync root.
///
/// Provider-specific API models do not cross this boundary. A missing remote identifier stays
/// explicit, and pagination tokens never prove that an item is absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RemoteObservation {
    /// Account that owns the remote synchronization.
    pub account_id: AccountId,
    /// Provider identity selected from the account configuration.
    pub provider_id: ProviderId,
    /// Synchronization root that scopes this remote item.
    pub sync_root_id: SyncRootId,
    /// Remote item observability state.
    pub state: FileState,
    /// Provider item identifier when supplied.
    pub remote_id: Option<RemoteId>,
    /// Remote item name evidence.
    pub name: Evidence<String>,
    /// Parent provider identifiers.
    pub parents: Evidence<Vec<RemoteId>>,
    /// Provider MIME type evidence.
    pub mime_type: Evidence<String>,
    /// Item size in bytes when applicable.
    pub size: Evidence<u64>,
    /// Content digest evidence when supported by the provider.
    pub checksum: Evidence<Checksum>,
    /// Provider item version evidence.
    pub version: Evidence<u64>,
    /// Remote content modification time.
    pub modified_time: Evidence<DateTime>,
    /// Whether the provider reports the item in its trash.
    pub trashed: Evidence<bool>,
    /// Remote operations the provider permits, when known.
    pub capabilities: Evidence<RemoteCapabilities>,
    /// Time at which this evidence was read.
    pub observed_at: DateTime,
    /// Coverage of the remote scope relevant to this observation.
    pub coverage: ObservationCoverage,
    /// Opaque provider pagination token associated with this read, if any.
    pub page_token: Option<String>,
    /// Optional normalized signal associated with this read.
    pub event: Option<ObservationEvent>,
}
