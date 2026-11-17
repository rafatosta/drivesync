//! Publishes preparation mutations without exposing diagnostic models to consumers.
use super::{PreparationState, PreparationStatus};
use crate::synchronization::{Publisher, SynchronizationActivity as Activity};
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
}
pub(super) struct Guard<'a> {
    value: MutexGuard<'a, PreparationStatus>,
    publisher: &'a Publisher,
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
        self.publisher.phase(match self.value.state {
            PreparationState::Starting => Activity::Starting,
            PreparationState::Scanning => Activity::Scanning,
            PreparationState::Analyzing => Activity::Analyzing,
            PreparationState::ProcessingEvents => Activity::Reconciling,
            _ => Activity::Idle,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reading_diagnostics_does_not_clear_active_work_and_ready_is_not_success() {
        let publisher = Arc::new(Publisher::new());
        let subscription = publisher.subscribe();
        let snapshot = Snapshot::new(
            PreparationStatus {
                state: PreparationState::Idle,
                scanner: None,
                report: None,
                error: None,
                events_processed: 0,
            },
            publisher.clone(),
        );
        snapshot.lock().unwrap().state = PreparationState::Scanning;
        assert_eq!(subscription.snapshot().activity, Activity::Scanning);
        snapshot.lock().unwrap().state = PreparationState::Analyzing;
        publisher.activity(Some(Activity::Downloading));
        drop(snapshot.lock().unwrap());
        assert_eq!(subscription.snapshot().activity, Activity::Downloading);
        snapshot.lock().unwrap().state = PreparationState::ReadyForAnalyzer;
        assert_eq!(subscription.snapshot().activity, Activity::Idle);
        assert!(!subscription.snapshot().synchronized);
        for state in [
            PreparationState::Partial,
            PreparationState::Failed,
            PreparationState::Cancelled,
            PreparationState::Stopped,
        ] {
            snapshot.lock().unwrap().state = state;
            assert_eq!(subscription.snapshot().activity, Activity::Idle);
            assert!(!subscription.snapshot().synchronized);
        }
    }
}
