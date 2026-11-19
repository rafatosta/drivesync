//! Input artifact and inventory snapshots produced by the context stage.

use crate::{
    inventory::{AnalysisEvidence, SyncItem},
    models::File,
};

/// Complete input to the decision stage for one detector delivery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalysisContext {
    /// Original validated detector object; no event identity or payload is projected away.
    pub file: File,
    /// Current inventory subjects related by explicit identities or the declared scope.
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
