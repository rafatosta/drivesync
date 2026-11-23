//! Analyzer input assembled by Context and the scope needed to construct it.

use crate::{
    detectors::observation::ObservationCoverage,
    inventory::models::{
        Baseline, ConflictState, InventoryLookupStatus, LocalState, PendingOperation, RemoteState,
    },
    models::{
        AccountId, DateTime, Evidence, ObservationEvent, Path, ProviderId, RemoteId, SyncRootId,
    },
};
use std::path::PathBuf;

/// Absolute local synchronization root configured for this Context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PathRoot(pub PathBuf);

/// Static synchronization scope and environment information used while consolidating items.
pub(crate) struct ContextConfig {
    /// Private directory that contains per-account Inventory data.
    pub inventory_directory: PathBuf,
    /// Account identity for the configured synchronization.
    pub account_id: AccountId,
    /// Provider selected for the account.
    pub provider_id: ProviderId,
    /// Synchronization root to which all accepted observations must belong.
    pub sync_root_id: SyncRootId,
    /// Configured absolute local synchronization root.
    pub local_root: PathRoot,
    /// Configured remote root identity.
    pub remote_root_id: RemoteId,
    /// Whether the local root is currently accessible.
    pub local_root_available: bool,
    /// Whether the provider can currently be queried.
    pub remote_available: bool,
    /// Explicit authorization to associate an initially untracked item.
    pub initial_association_authorized: bool,
}

impl ContextConfig {
    pub(super) fn environment(&self) -> Environment {
        Environment {
            local_root: self.local_root.clone(),
            remote_root_id: self.remote_root_id.clone(),
            local_root_available: self.local_root_available,
            remote_available: self.remote_available,
            destination_local_parent: Evidence::NotCollected,
            destination_remote_parent_id: Evidence::NotCollected,
            destination_remote_matches: Evidence::NotCollected,
            destination_local_occupied: Evidence::NotCollected,
            local_replacement: Evidence::NotCollected,
            projection_issue: None,
        }
    }
}

/// Current execution environment facts; unknown destinations remain explicit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Environment {
    /// Proven per-item policy refusal, independent of scope completeness.
    pub projection_issue: Option<String>,
    /// A new regular inode occupies the already synchronized path; the prior inode is absent.
    pub local_replacement: Evidence<bool>,
    /// Configured absolute local synchronization root.
    pub local_root: PathRoot,
    /// Configured remote root identity.
    pub remote_root_id: RemoteId,
    /// Whether the local root is accessible now.
    pub local_root_available: bool,
    /// Whether the remote provider is available now.
    pub remote_available: bool,
    /// Resolved local destination parent, when supplied by a later context source.
    pub destination_local_parent: Evidence<Path>,
    /// Resolved remote destination parent, when known.
    pub destination_remote_parent_id: Evidence<RemoteId>,
    /// Known remote destination collisions.
    pub destination_remote_matches: Evidence<Vec<RemoteId>>,
    /// Whether the local destination is occupied, when checked.
    pub destination_local_occupied: Evidence<bool>,
}

/// Provenance and coverage retained for the most recent observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Observations {
    /// Epoch sampled before the local capture started, never at delivery time.
    pub local_epoch: Option<u64>,
    /// Epoch sampled before the remote capture started, never at delivery time.
    pub remote_epoch: Option<u64>,
    /// Remote capture sequence, independent of provider cursors and wall-clock time.
    pub remote_generation: Option<u64>,
    /// Normalized events explicitly supplied by observation sources.
    pub events: Vec<ObservationEvent>,
    /// Source of the observation most recently consolidated.
    pub latest_origin: crate::models::ObservationOrigin,
    /// Local scope coverage of the most recent observation.
    pub local_coverage: ObservationCoverage,
    /// Remote scope coverage of the most recent observation.
    pub remote_coverage: ObservationCoverage,
    /// Local scan generation when a producer supplies one.
    pub local_generation: Option<u64>,
    /// Opaque provider pagination token, when supplied.
    pub remote_page_token: Option<String>,
    /// Time of the consolidated observation.
    pub observed_at: DateTime,
}

/// Complete, provider-neutral facts for one logical item, ready for pure analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct File {
    /// Account, sync root, and known item identities.
    pub identity: crate::models::FileIdentity,
    /// Opaque proposed ID for an initial Operations write; not a persisted identity.
    pub registration_id: Option<crate::models::ItemId>,
    /// Current local observation; unobserved attributes stay explicit.
    pub local: LocalState,
    /// Current remote observation; unobserved attributes stay explicit.
    pub remote: RemoteState,
    /// Last baseline confirmed by an operation, or explicit availability state.
    pub baseline: Baseline,
    /// Observation provenance and scope coverage.
    pub observations: Observations,
    /// Current configured execution environment.
    pub environment: Environment,
    /// Explicit result of the account/root-scoped Inventory lookup.
    pub inventory_lookup: InventoryLookupStatus,
    /// Persisted operations relevant to recovery and analysis.
    pub pending_operations: Vec<PendingOperation>,
    /// Previously recorded unresolved conflict, if any.
    pub prior_conflict: Option<ConflictState>,
    /// Whether policy explicitly allows an initial association.
    pub initial_association_authorized: bool,
    /// Inventory revision used for future optimistic writes.
    pub inventory_revision: Option<u64>,
    /// Possible initial counterparts, never proof of a persisted identity or authorization.
    pub association_candidates: Vec<AssociationCandidate>,
}

/// Location correspondence gathered from current captures, without assigning identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AssociationCandidate {
    pub local: LocalState,
    pub remote: RemoteState,
    /// Directory associations, not projected names alone, establish corresponding parents.
    pub parents_correspond: Evidence<bool>,
}
