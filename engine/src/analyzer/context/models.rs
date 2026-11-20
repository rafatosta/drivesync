//! Input artifact and inventory snapshots produced by the context stage.

use crate::{
    inventory::{AnalysisEvidence, PresenceState, SyncItem},
    models::File,
};

/// Complete input to Decision for one detector delivery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalysisContext {
    /// Original detector object, never rewritten.
    pub file: File,
    pub association: ContextAssociation,
    /// Facts proved by this incoming File only. Inventory facts remain separate.
    pub observation: ObservationContext,
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
