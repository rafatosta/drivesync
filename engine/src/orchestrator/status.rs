//! Publishes complete engine-owned snapshots for preparation and observation flow.
use super::{PreparationFailure, PreparationState, PreparationStatus};
use crate::synchronization::{
    Publisher, SynchronizationAction, SynchronizationActivity, SynchronizationIssue,
    SynchronizationIssueKind, SynchronizationState, SynchronizationStatus,
    SynchronizationWaitReason, SynchronizationWork, SynchronizationWorkDetail,
};
use std::sync::{Arc, LockResult, Mutex, MutexGuard, PoisonError};

pub(super) struct Snapshot {
    value: Mutex<PreparationStatus>,
    publisher: Arc<Publisher>,
}
impl Snapshot {
    pub fn new(value: PreparationStatus, publisher: Arc<Publisher>) -> Self {
        Self {
            value: Mutex::new(value),
            publisher,
        }
    }
    pub fn lock(&self) -> LockResult<Guard<'_>> {
        match self.value.lock() {
            Ok(value) => Ok(Guard {
                value,
                publisher: &self.publisher,
                previous: None,
            }),
            Err(e) => Err(PoisonError::new(Guard {
                value: e.into_inner(),
                publisher: &self.publisher,
                previous: None,
            })),
        }
        .map(|mut guard| {
            guard.previous = Some(guard.value.state);
            guard
        })
    }
    pub fn update_status(&self, update: impl FnOnce(&mut SynchronizationStatus)) {
        self.publisher.update(update);
    }
    pub fn publish_detector(&self, detector: &crate::detectors::DetectorStatus) {
        let sources = sources(detector);
        let problem = source_issue(&sources);
        self.publisher.update(|status| {
            status.sources = Some(sources.clone());
            if detector.state == crate::detectors::DetectorState::Stopped {
                status.state = SynchronizationState::Stopped;
                status.activity = SynchronizationActivity::Stopped;
                status.work = None;
                status.progress = None;
            } else if detector.state == crate::detectors::DetectorState::Failed {
                status.state = SynchronizationState::Failed;
                status.activity = SynchronizationActivity::Failed;
            }
            let existing_is_source_issue = status.issue.as_ref().is_some_and(|issue| {
                matches!(
                    issue.kind,
                    SynchronizationIssueKind::LocalSource(_)
                        | SynchronizationIssueKind::RemoteSource(_)
                        | SynchronizationIssueKind::Queue(_)
                        | SynchronizationIssueKind::Detector(_)
                )
            });
            if problem.is_some() || existing_is_source_issue {
                status.issue = problem.clone();
            }
            if problem.is_some() && status.state == SynchronizationState::Waiting {
                status.work = Some(SynchronizationWork {
                    action: SynchronizationAction::Reconcile,
                    item: None,
                    location: None,
                    detail: SynchronizationWorkDetail::Waiting(
                        SynchronizationWaitReason::SourceRecovery,
                    ),
                    event: None,
                });
            } else if problem.is_none()
                && existing_is_source_issue
                && status.state == SynchronizationState::Waiting
            {
                status.work = Some(SynchronizationWork {
                    action: SynchronizationAction::Reconcile,
                    item: None,
                    location: None,
                    detail: SynchronizationWorkDetail::Waiting(
                        SynchronizationWaitReason::ForChanges,
                    ),
                    event: None,
                });
            }
        });
    }
}

fn sources(
    detector: &crate::detectors::DetectorStatus,
) -> crate::synchronization::SynchronizationSources {
    use crate::synchronization::{
        SynchronizationLocalSource, SynchronizationRemoteSource, SynchronizationSources,
    };
    let local = detector.local.local.as_ref();
    let remote = detector.remote.remote.as_ref();
    SynchronizationSources {
        detector_state: Some(detector.state),
        delivery: Some(detector.delivery),
        pending_events: detector.queue.pending,
        leased_events: detector.queue.leased,
        confirmed_events: detector.queue.confirmed,
        detected_events: detector.queue.detected,
        queue_error: detector.queue.last_error,
        local: SynchronizationLocalSource {
            state: local.map(|value| value.state),
            activity: local.map(|value| value.activity),
            observed_in_current_round: local.map_or(0, |value| value.observed_in_current_round),
            emitted_in_current_round: local.map_or(0, |value| value.emitted_in_current_round),
            last_observation_at: local.and_then(|value| value.last_observation_at),
            coverage: local.and_then(|value| value.last_coverage),
            last_error: local.and_then(|value| value.last_error),
            source_error: detector.local.last_error,
        },
        remote: SynchronizationRemoteSource {
            state: remote.map(|value| value.state),
            activity: remote.map(|value| value.activity),
            coverage: remote.map(|value| value.coverage),
            polling_interval: remote.map(|value| value.polling_interval),
            observed_in_current_round: remote.map_or(0, |value| value.observed_in_current_round),
            emitted_in_current_round: remote.map_or(0, |value| value.emitted_in_current_round),
            last_observed_at: remote.and_then(|value| value.last_observed_at),
            last_poll_started_at: remote.and_then(|value| value.last_poll_started_at),
            next_poll_at: remote.and_then(|value| value.next_poll_at),
            last_poll_duration: remote.and_then(|value| value.last_poll_duration),
            consecutive_failures: remote.map_or(0, |value| value.consecutive_failures),
            recovery_attempts: remote.map_or(0, |value| value.recovery_attempts),
            last_error: remote.and_then(|value| value.last_error),
            source_error: detector.remote.last_error,
        },
    }
}

fn source_issue(
    sources: &crate::synchronization::SynchronizationSources,
) -> Option<SynchronizationIssue> {
    use crate::detectors::{DetectorSourceError, QueueError};
    use crate::synchronization::SynchronizationIssueKind as Kind;
    let (kind, retryable) = if let Some(error) = sources.local.source_error {
        Some(match error {
            DetectorSourceError::Local(error) => (
                Kind::LocalSource(error),
                !matches!(
                    error,
                    crate::detectors::LocalDetectorError::InvalidScope
                        | crate::detectors::LocalDetectorError::ModelInvalid
                        | crate::detectors::LocalDetectorError::InvalidState
                ),
            ),
            DetectorSourceError::Remote(error) => {
                (Kind::RemoteSource(error), remote_retryable(error))
            }
        })
    } else if let Some(error) = sources.remote.source_error {
        Some(match error {
            DetectorSourceError::Local(error) => (Kind::LocalSource(error), true),
            DetectorSourceError::Remote(error) => {
                (Kind::RemoteSource(error), remote_retryable(error))
            }
        })
    } else if let Some(error) = sources.local.last_error {
        Some((Kind::LocalSource(error), true))
    } else if let Some(error) = sources.remote.last_error {
        Some((Kind::RemoteSource(error), remote_retryable(error)))
    } else if let Some(error) = sources.queue_error {
        Some((
            Kind::Queue(error),
            matches!(
                error,
                QueueError::Unavailable | QueueError::CapacityExceeded | QueueError::LeaseExpired
            ),
        ))
    } else if sources.detector_state == Some(crate::detectors::DetectorState::Degraded) {
        Some((
            Kind::Detector(crate::detectors::DetectorError::SourceUnavailable),
            true,
        ))
    } else if sources.detector_state == Some(crate::detectors::DetectorState::Failed) {
        Some((
            Kind::Detector(crate::detectors::DetectorError::Internal),
            false,
        ))
    } else {
        None
    }?;
    Some(SynchronizationIssue {
        kind,
        retryable,
        item: None,
        count: 1,
    })
}

fn remote_retryable(error: crate::detectors::RemoteError) -> bool {
    matches!(
        error,
        crate::detectors::RemoteError::RemoteUnavailable
            | crate::detectors::RemoteError::PageIncomplete
            | crate::detectors::RemoteError::SinkUnavailable
            | crate::detectors::RemoteError::CheckpointUnavailable
            | crate::detectors::RemoteError::CursorExpired
    )
}
pub(super) struct Guard<'a> {
    value: MutexGuard<'a, PreparationStatus>,
    publisher: &'a Arc<Publisher>,
    previous: Option<PreparationState>,
}
impl std::ops::Deref for Guard<'_> {
    type Target = PreparationStatus;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
impl std::ops::DerefMut for Guard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}
impl Drop for Guard<'_> {
    fn drop(&mut self) {
        if self.previous == Some(self.value.state) {
            return;
        }
        let state = self.value.state;
        let error = self.value.error.clone();
        self.publisher
            .update(|status| apply_state(status, state, error.as_ref()));
    }
}

fn apply_state(
    status: &mut SynchronizationStatus,
    state: PreparationState,
    error: Option<&PreparationFailure>,
) {
    match state {
        PreparationState::Idle => *status = SynchronizationStatus::default(),
        PreparationState::Starting => {
            *status = SynchronizationStatus {
                state: SynchronizationState::Running,
                activity: SynchronizationActivity::Starting,
                work: Some(SynchronizationWork {
                    action: SynchronizationAction::ObserveLocal,
                    item: None,
                    location: None,
                    detail: SynchronizationWorkDetail::Preparation,
                    event: None,
                }),
                ..Default::default()
            };
        }
        PreparationState::ProcessingEvents => {
            status.state = SynchronizationState::Running;
            status.activity = SynchronizationActivity::ProcessingEvents;
            status.work = Some(SynchronizationWork {
                action: SynchronizationAction::Reconcile,
                item: None,
                location: None,
                detail: SynchronizationWorkDetail::Preparation,
                event: None,
            });
            status.progress = None;
        }
        PreparationState::ReadyForAnalyzer => {
            status.state = SynchronizationState::Waiting;
            status.activity = SynchronizationActivity::Waiting;
            status.work = Some(SynchronizationWork {
                action: SynchronizationAction::Reconcile,
                item: None,
                location: None,
                detail: SynchronizationWorkDetail::Waiting(SynchronizationWaitReason::ForChanges),
                event: None,
            });
            status.progress = None;
        }
        PreparationState::Failed => {
            status.state = SynchronizationState::Failed;
            status.activity = SynchronizationActivity::Failed;
            status.progress = None;
            if let Some(error) = error {
                let mut problem = issue(error);
                problem.item = status.work.as_ref().and_then(|work| work.item.clone());
                status.issue = Some(problem);
            }
        }
        PreparationState::Cancelled | PreparationState::Stopped => {
            status.state = SynchronizationState::Stopped;
            status.activity = SynchronizationActivity::Stopped;
            status.work = None;
            status.progress = None;
        }
    }
}

fn issue(error: &PreparationFailure) -> SynchronizationIssue {
    let (kind, retryable) = match error {
        PreparationFailure::Detector(error) => {
            let kind = match error.source {
                Some(crate::detectors::DetectorSourceError::Local(source)) => {
                    SynchronizationIssueKind::LocalSource(source)
                }
                Some(crate::detectors::DetectorSourceError::Remote(source)) => {
                    SynchronizationIssueKind::RemoteSource(source)
                }
                None => SynchronizationIssueKind::Detector(error.kind),
            };
            (kind, error.retryable)
        }
        PreparationFailure::Inventory(error) => (
            SynchronizationIssueKind::Inventory(error.kind.clone()),
            error.retryable,
        ),
        PreparationFailure::Analyzer(error) => (
            SynchronizationIssueKind::Analyzer(error.kind),
            matches!(
                error.kind,
                crate::analyzer::AnalysisErrorKind::Inventory
                    | crate::analyzer::AnalysisErrorKind::Evidence
            ),
        ),
        PreparationFailure::Internal => (SynchronizationIssueKind::Internal, false),
        PreparationFailure::AlreadyRunning => (SynchronizationIssueKind::Internal, true),
    };
    SynchronizationIssue {
        kind,
        retryable,
        item: None,
        count: 1,
    }
}

pub(super) fn detector_issue(error: &crate::detectors::DetectorFailure) -> SynchronizationIssue {
    SynchronizationIssue {
        kind: SynchronizationIssueKind::Detector(error.kind),
        retryable: error.retryable,
        item: None,
        count: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preparation_states_publish_start_wait_failure_and_stopped() {
        let publisher = Arc::new(Publisher::new());
        let snapshot = Snapshot::new(
            PreparationStatus {
                state: PreparationState::Idle,
                error: None,
                events_processed: 0,
            },
            publisher.clone(),
        );
        snapshot.lock().unwrap().state = PreparationState::Starting;
        assert_eq!(
            publisher.snapshot().activity,
            SynchronizationActivity::Starting
        );
        snapshot.lock().unwrap().state = PreparationState::ReadyForAnalyzer;
        assert_eq!(publisher.snapshot().state, SynchronizationState::Waiting);
        assert!(matches!(
            publisher.snapshot().work.unwrap().detail,
            SynchronizationWorkDetail::Waiting(SynchronizationWaitReason::ForChanges)
        ));
        snapshot.lock().unwrap().state = PreparationState::ProcessingEvents;
        assert_eq!(
            publisher.snapshot().activity,
            SynchronizationActivity::ProcessingEvents
        );
        snapshot.lock().unwrap().state = PreparationState::Stopped;
        assert_eq!(
            publisher.snapshot().activity,
            SynchronizationActivity::Stopped
        );
    }
}
