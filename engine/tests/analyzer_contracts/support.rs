//! Shared value fixtures for Analyzer artifact contract tests.

use drivesync_engine::{
    analyzer::{AnalysisItemContext, ObservationContext, ObservationFreshness},
    inventory::{
        AnalysisEvidence, EvidenceRelation, ItemKind, PresenceState, SyncItem, SyncItemId,
        SyncStatus,
    },
    models::{
        File, FileEvent, FileEventAction, FileId, FileScope, ObservationCoverage,
        ObservationUncertainty, SyncId, Timestamp,
    },
};

/// Builds a detector envelope with stable identity and caller-selected event scope.
pub(crate) fn file(action: FileEventAction, scope: FileScope) -> File {
    File {
        id: FileId("detector-file-1".into()),
        causal_order: Some(7),
        sync_id: SyncId("sync-1".into()),
        event: FileEvent {
            local: None,
            local_presence: None,
            remote: None,
            action,
            uncertainty: (action == FileEventAction::RecheckRequired)
                .then_some(ObservationUncertainty::CoverageIncomplete),
            previous: None,
            current: None,
        },
        observed_at: Timestamp { unix_millis: 100 },
        coverage: ObservationCoverage::Complete,
        scope,
        data: None,
    }
}

/// Builds a minimal inventory snapshot whose revision can be varied per case.
pub(crate) fn item_context(revision: u64) -> AnalysisItemContext {
    AnalysisItemContext {
        item: SyncItem {
            id: SyncItemId("inventory-item-1".into()),
            kind: ItemKind::File,
            local_file_id: None,
            remote_ref: None,
            local: None,
            remote: None,
            local_latest: None,
            remote_latest: None,
            baseline: None,
            local_presence: PresenceState::Unknown,
            remote_presence: PresenceState::Unknown,
            status: SyncStatus::PendingReconciliation,
            conflict_reason: None,
            revision,
            last_checked_at: None,
            last_synced_at: None,
        },
        evidence: unknown_evidence(),
        freshness: ObservationFreshness::Incomparable,
    }
}

pub(crate) fn unknown_evidence() -> AnalysisEvidence {
    AnalysisEvidence {
        baseline_confirmed: false,
        local_scope_complete: false,
        remote_scope_complete: false,
        local_content: EvidenceRelation::Unknown,
        remote_content: EvidenceRelation::Unknown,
        current_content: EvidenceRelation::Unknown,
        local_parent: None,
        current_parents: EvidenceRelation::Unknown,
    }
}

pub(crate) fn unknown_observation() -> ObservationContext {
    ObservationContext {
        local_presence: PresenceState::Unknown,
        remote_presence: PresenceState::Unknown,
        evidence: unknown_evidence(),
    }
}
