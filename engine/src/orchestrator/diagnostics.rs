//! Cached summaries projected from the unified durable event queue.
use crate::{detectors::*, models::*};
/// Event context for inspection; no file contents, credentials or metadata payload.
#[derive(Clone, Debug)]
pub struct DetectorEventSummary {
    pub id: FileId,
    pub observed_at: Timestamp,
    pub event: FileEvent,
    pub scope: FileScope,
    /// Observed type only; absent metadata never implies a regular file.
    pub entry_kind: Option<LocalEntryKind>,
}
/// Per-account local diagnostics projected from the unified queue.
#[derive(Clone, Debug)]
pub struct DetectorDiagnostics {
    pub account_id: AccountId,
    pub sync_id: SyncId,
    pub events_detected: u64,
    pub initial_observations: u64,
    pub last_event_at: Option<Timestamp>,
    pub pending_events: Option<u64>,
    pub leased_events: Option<u64>,
    pub confirmed_events: Option<u64>,
    pub delivery: Option<DeliveryState>,
    pub queue_error: Option<QueueError>,
    pub events: Vec<DetectorEventSummary>,
}
pub(super) fn summarize(file: File) -> DetectorEventSummary {
    DetectorEventSummary {
        id: file.id,
        observed_at: file.observed_at,
        event: file.event,
        scope: file.scope,
        entry_kind: match file.data {
            Some(FileData::Local(local)) => match local.kind {
                MetadataValue::Known(kind) => Some(kind),
                _ => None,
            },
            _ => None,
        },
    }
}
