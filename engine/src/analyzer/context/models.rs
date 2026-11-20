//! Input artifact and inventory snapshots produced by the context stage.

use crate::{
    inventory::{AnalysisEvidence, PresenceState, SyncItem},
    models::*,
};

/// Complete input to Decision for one detector delivery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalysisContext {
    /// Original detector object, never rewritten.
    pub file: File,
    pub association: ContextAssociation,
    /// Facts proved by this incoming File only. Inventory facts remain separate.
    pub observation: ObservationContext,
    /// Enriched facts distinct from the incoming observation and history.
    pub facts: ContextFacts,
}

/// Inventory snapshot and evidence applicable to its exact revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalysisItemContext {
    pub item: SyncItem,
    pub evidence: AnalysisEvidence,
    /// Comparison of the incoming observation with the stored marker on its side.
    pub freshness: ObservationFreshness,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservationFreshness {
    Older,
    Equal,
    Newer,
    Incomparable,
}

/// Identity lookup outcome; never a synchronization decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContextAssociation {
    Matched(Box<AnalysisItemContext>),
    Unmatched,
    /// Candidates only: no identity link or permission to act.
    Ambiguous(Vec<AnalysisItemContext>),
    Scope(Vec<AnalysisItemContext>),
}

impl ContextAssociation {
    /// Borrow related snapshots without cloning; ambiguous candidates are not links.
    pub fn items(&self) -> &[AnalysisItemContext] {
        match self {
            Self::Matched(item) => std::slice::from_ref(item.as_ref()),
            Self::Unmatched => &[],
            Self::Ambiguous(items) | Self::Scope(items) => items,
        }
    }
}

/// Facts independent of whether the resource has an inventory row.
/// Unobserved sides and whole-sync scalar presence are Unknown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservationContext {
    pub local_presence: PresenceState,
    pub remote_presence: PresenceState,
    pub evidence: AnalysisEvidence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextFacts {
    pub scope: MetadataValue<ContextScope>,
    pub subjects: Vec<SubjectFacts>,
}
impl Default for ContextFacts {
    fn default() -> Self { Self { scope: MetadataValue::NotCollected, subjects: vec![] } }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextScope {
    pub sync_id: SyncId,
    pub account_id: AccountId,
    pub provider: ProviderId,
    pub local_root: Vec<u8>,
    pub remote_root: RemoteFileRef,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubjectKey { Observation(FileId), Item { id: crate::inventory::SyncItemId, revision: u64 } }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurrentSide<T> {
    pub presence: MetadataValue<ObservedPresence>,
    pub metadata: MetadataValue<T>,
    pub evidence: FileEvidence,
    pub causal_order: MetadataValue<u64>,
    pub observed_at: MetadataValue<Timestamp>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Occupancy<T> { Free, Occupied(Vec<T>) }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalDestination {
    pub parent: ObservedParent,
    pub name: Vec<u8>,
    pub occupancy: MetadataValue<Occupancy<LocalFile>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteDestination {
    pub parent: RemoteFileRef,
    pub name: String,
    pub occupancy: MetadataValue<Occupancy<RemoteFile>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubjectFacts {
    pub subject: SubjectKey,
    pub local: CurrentSide<LocalFile>,
    pub remote: CurrentSide<RemoteFile>,
    pub local_destination: MetadataValue<LocalDestination>,
    pub remote_destination: MetadataValue<RemoteDestination>,
    pub parent_relation: crate::inventory::EvidenceRelation,
    pub local_content: crate::inventory::EvidenceRelation,
    pub remote_content: crate::inventory::EvidenceRelation,
    pub current_content: crate::inventory::EvidenceRelation,
    pub deletion_authorized: MetadataValue<bool>,
    /// Path-only absence may select a historical subject but never creates identity.
    pub historical_path_match: MetadataValue<crate::inventory::SyncItemId>,
}
