//! Input artifact and inventory snapshots produced by the context stage.

use crate::{
    inventory::{AnalysisEvidence, PresenceState, SyncItem},
    models::File,
};

/// Complete input to the decision stage for one detector delivery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalysisContext {
    /// Original validated detector object; no event identity or payload is projected away.
    pub file: File,
    /// Result of identity lookup, independent of physical presence.
    pub association: ContextAssociation,
    /// Facts about the incoming observation, even without an inventory item.
    /// Whole-sync scopes carry unknown scalar presence and per-item facts below.
    pub observation: ObservationContext,
    /// Current inventory subjects related by explicit identities or the declared scope.
    /// Matched has exactly one item; Unmatched has none; Ambiguous contains only
    /// candidates, not established links; Scope may have zero or more items.
    pub items: Vec<AnalysisItemContext>,
}

/// Current snapshot and measured evidence for one inventory subject.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalysisItemContext {
    /// Current logical item as read from Inventory.
    pub item: SyncItem,
    /// Evidence measured for this exact item revision.
    pub evidence: AnalysisEvidence,
}

/// Identity lookup outcome; never a synchronization decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextAssociation {
    Matched,
    Unmatched,
    Ambiguous,
    Scope,
}

/// Observed facts, independent of whether the resource has an inventory row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservationContext {
    pub local_presence: PresenceState,
    pub remote_presence: PresenceState,
    pub evidence: AnalysisEvidence,
}
