//! Shared activity sampling, bounded retention and off-thread diagnostic output.

use std::time::{Duration, Instant};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

use crate::models::{AccountId, SyncRootId};

/// Identity within one activity inbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ActivityId(pub(crate) u64);

/// Producer namespace for activity references crossing module inboxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActivityModule {
    Detectors,
    Operations,
    Inventory,
}

/// Unambiguous activity identity; IDs alone are local to their producer inbox.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ActivityReference {
    pub(crate) module: ActivityModule,
    pub(crate) producer: u64,
    pub(crate) session: u64,
    pub(crate) account: AccountId,
    pub(crate) root: Option<SyncRootId>,
    pub(crate) id: ActivityId,
}

impl std::fmt::Debug for ActivityReference {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ActivityReference")
            .field("module", &self.module)
            .field("producer", &self.producer)
            .field("session", &self.session)
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// Correlation is a related flow reference, not a parent-child relationship.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CorrelationReference(pub(crate) ActivityReference);

/// Optional caller context that Inventory can validate against its fixed scope.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ActivityContext {
    pub(crate) parent: Option<ActivityReference>,
    pub(crate) correlation: Option<CorrelationReference>,
}

/// Fixed synchronization scope, independent of work generations and capture epochs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Scope {
    pub(crate) account: AccountId,
    pub(crate) root: SyncRootId,
}

pub(crate) const HASH_PROGRESS_MIN: u64 = 1024 * 1024;

const INTERVAL: Duration = Duration::from_millis(200);

/// The receiver uses the sequence to reject samples delivered out of order.
pub(crate) struct Sample<T> {
    pub(crate) sequence: u64,
    pub(crate) at: Instant,
    pub(crate) value: T,
    pub(crate) coalesced: u64,
}

#[derive(Default)]
struct Sampling {
    last: Option<Instant>,
    sequence: u64,
    coalesced: u64,
}

#[derive(Default)]
pub(crate) struct ActivityReporter(Mutex<Sampling>);

impl ActivityReporter {
    pub(crate) fn sample<T>(
        &self,
        immediate: bool,
        value: impl FnOnce() -> T,
    ) -> Option<Sample<T>> {
        self.next(immediate, true, value)
    }

    /// A mandatory wait transition does not postpone the next sampled progress update.
    pub(crate) fn transition<T>(&self, value: T) -> Sample<T> {
        self.next(true, false, || value)
            .expect("transitions are never sampled out")
    }

    fn next<T>(
        &self,
        immediate: bool,
        reset_progress_interval: bool,
        value: impl FnOnce() -> T,
    ) -> Option<Sample<T>> {
        let (sequence, at, coalesced) = {
            let mut sampling = self.0.lock().unwrap();
            let at = Instant::now();
            if !immediate && sampling.last.is_some_and(|last| at - last < INTERVAL) {
                sampling.coalesced += 1;
                return None;
            }
            if reset_progress_interval {
                sampling.last = Some(at);
            }
            sampling.sequence += 1;
            (
                sampling.sequence,
                at,
                std::mem::take(&mut sampling.coalesced),
            )
        };
        Some(Sample {
            sequence,
            at,
            value: value(),
            coalesced,
        })
    }

    pub(crate) fn take_coalesced(&self) -> u64 {
        std::mem::take(&mut self.0.lock().unwrap().coalesced)
    }
}

/// A recent-history buffer whose producer never waits for a consumer to make room.
pub(crate) struct Recent<T> {
    capacity: usize,
    entries: VecDeque<T>,
}

impl<T> Recent<T> {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: VecDeque::with_capacity(capacity),
        }
    }

    pub(crate) fn push(&mut self, entry: T) -> bool {
        let full = self.entries.len() == self.capacity;
        if full {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
        full
    }

    pub(crate) fn back_mut(&mut self) -> Option<&mut T> {
        self.entries.back_mut()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(crate) fn take(&mut self) -> Vec<T> {
        self.entries.drain(..).collect()
    }

    pub(crate) fn snapshot(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.entries.iter().cloned().collect()
    }
}

/// Bounds outstanding stderr work across scopes even when the output blocks.
pub(crate) struct DiagnosticWriter(AtomicBool);

impl DiagnosticWriter {
    pub(crate) const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    pub(crate) fn spawn(
        &'static self,
        write: impl FnOnce() + Send + 'static,
        wake: impl FnOnce() + Send + 'static,
    ) {
        if self
            .0
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return;
        }
        struct Lease(&'static AtomicBool);
        impl Drop for Lease {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release);
            }
        }
        let lease = Lease(&self.0);
        tokio::task::spawn_blocking(move || {
            write();
            drop(lease);
            wake();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_are_sequenced_and_constructed_after_the_sampling_lock_is_released() {
        let reporter = ActivityReporter::default();
        let first = reporter
            .sample(false, || {
                assert!(reporter.0.try_lock().is_ok());
                1
            })
            .unwrap();
        reporter.0.lock().unwrap().last = Some(Instant::now() + Duration::from_secs(60));
        assert!(reporter
            .sample(false, || panic!("discarded samples should be lazy"))
            .is_none());
        let second = reporter.sample(true, || 2).unwrap();
        assert_eq!((first.sequence, first.value), (1, 1));
        assert_eq!((second.sequence, second.value), (2, 2));
        assert!(first.at <= second.at);
    }

    #[test]
    fn activity_reference_debug_output_does_not_expose_scope_identifiers() {
        let reference = ActivityReference {
            module: ActivityModule::Inventory,
            producer: 3,
            session: 4,
            account: AccountId("private-account-value".into()),
            root: Some(SyncRootId("private-root-value".into())),
            id: ActivityId(9),
        };
        let debug = format!("{reference:?}");
        assert!(!debug.contains("private-account-value"));
        assert!(!debug.contains("private-root-value"));
        assert!(debug.contains("Inventory"));
        assert!(debug.contains("ActivityId(9)"));
    }
}
