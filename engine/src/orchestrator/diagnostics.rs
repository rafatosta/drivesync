//! Bounded observational history, distinct from the future durable event queue.
use crate::{detectors::*, models::*};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
const HISTORY_LIMIT: usize = 100;
/// Event context for inspection; no file contents, credentials or metadata payload.
#[derive(Clone, Debug)]
pub struct DetectorEventSummary {
    pub id: FileId,
    pub observed_at: Timestamp,
    pub event: FileEvent,
    pub scope: FileScope,
}
/// Per-instance diagnostics. Pending durable work is unknown until queue integration.
#[derive(Clone, Debug)]
pub struct DetectorDiagnostics {
    pub account_id: AccountId,
    pub sync_id: SyncId,
    pub events_detected: u64,
    pub last_event_at: Option<Timestamp>,
    pub pending_events: Option<u64>,
    pub events: Vec<DetectorEventSummary>,
}
pub(super) struct History {
    pub account: AccountId,
    pub sync: SyncId,
    pub detected: u64,
    pub last_id: Option<FileId>,
    pub last_time: Option<Timestamp>,
    pub events: VecDeque<DetectorEventSummary>,
}
impl History {
    pub fn new(scope: &LocalDetectorScope) -> Self {
        Self {
            account: scope.account_id.clone(),
            sync: scope.sync_id.clone(),
            detected: 0,
            last_id: None,
            last_time: None,
            events: VecDeque::new(),
        }
    }
    pub fn snapshot(&self) -> DetectorDiagnostics {
        DetectorDiagnostics {
            account_id: self.account.clone(),
            sync_id: self.sync.clone(),
            events_detected: self.detected,
            last_event_at: self.last_time,
            pending_events: None,
            events: self.events.iter().cloned().collect(),
        }
    }
}
pub(super) struct RecordingSink {
    pub history: Arc<Mutex<History>>,
    pub receiver: Arc<dyn LocalNotificationSink>,
}
impl LocalNotificationSink for RecordingSink {
    fn notify(&self, file: File) -> LocalDetectorResult<()> {
        {
            let mut history = self.history.lock().unwrap_or_else(|e| e.into_inner());
            // The producer retries only its current occurrence, retaining its ID.
            if history.last_id.as_ref() != Some(&file.id) {
                history.last_id = Some(file.id.clone());
                history.last_time = Some(file.observed_at);
                history.detected = history.detected.saturating_add(1);
                if history.events.len() == HISTORY_LIMIT {
                    history.events.pop_front();
                }
                history.events.push_back(DetectorEventSummary {
                    id: file.id.clone(),
                    observed_at: file.observed_at,
                    event: file.event.clone(),
                    scope: file.scope.clone(),
                });
            }
        }
        self.receiver.notify(file)
    }
}
/// Terminal diagnostic consumer; no synchronization work is accepted or queued.
pub(super) struct ObservationOnly;
impl LocalNotificationSink for ObservationOnly {
    fn notify(&self, _: File) -> LocalDetectorResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retries_keep_one_occurrence_and_history_has_a_finite_tail() {
        struct Reject;
        impl LocalNotificationSink for Reject {
            fn notify(&self, _: File) -> LocalDetectorResult<()> {
                Err(LocalDetectorFailure {
                    kind: LocalDetectorError::NotificationFailed,
                    message: "retry".into(),
                    retryable: true,
                    progress: None,
                })
            }
        }
        let history = Arc::new(Mutex::new(History::new(&LocalDetectorScope {
            account_id: AccountId("account".into()),
            sync_id: SyncId("sync".into()),
            local_root: "/tmp".into(),
        })));
        let sink = RecordingSink {
            history: history.clone(),
            receiver: Arc::new(Reject),
        };
        let event = |id| File {
            id: FileId(format!("event-{id}")),
            sync_id: SyncId("sync".into()),
            event: FileEvent {
                kind: FileEventKind::RecheckRequired,
                previous: None,
                current: None,
            },
            observed_at: Timestamp { unix_millis: id },
            coverage: ObservationCoverage::Partial,
            scope: FileScope::WholeSync,
            data: None,
        };
        for _ in 0..3 {
            assert!(sink.notify(event(0)).unwrap_err().retryable);
        }
        assert_eq!(history.lock().unwrap().snapshot().events_detected, 1);
        for id in 1..150 {
            let _ = sink.notify(event(id));
        }
        let snapshot = history.lock().unwrap().snapshot();
        assert_eq!(snapshot.events_detected, 150);
        assert_eq!(snapshot.events.len(), 100);
        assert_eq!(
            snapshot.events.first().unwrap().id,
            FileId("event-50".into())
        );
        assert_eq!(
            snapshot.events.last().unwrap().id,
            FileId("event-149".into())
        );
        assert_eq!(snapshot.pending_events, None);
    }
}
