//! Monotonic sampling with lazy event construction and no sink lock coupling.

use std::sync::Mutex;
use std::time::{Duration, Instant};

const INTERVAL: Duration = Duration::from_millis(200);

/// The receiver uses the sequence to reject samples delivered out of order.
pub(super) struct Sample<T> {
    pub(super) sequence: u64,
    pub(super) at: Instant,
    pub(super) value: T,
}

#[derive(Default)]
struct Sampling {
    last: Option<Instant>,
    sequence: u64,
}

#[derive(Default)]
pub(super) struct ActivityReporter(Mutex<Sampling>);

impl ActivityReporter {
    pub(super) fn sample<T>(
        &self,
        immediate: bool,
        value: impl FnOnce() -> T,
    ) -> Option<Sample<T>> {
        let (sequence, at) = {
            let mut sampling = self.0.lock().unwrap();
            let at = Instant::now();
            if !immediate && sampling.last.is_some_and(|last| at - last < INTERVAL) {
                return None;
            }
            sampling.last = Some(at);
            sampling.sequence += 1;
            (sampling.sequence, at)
        };
        Some(Sample {
            sequence,
            at,
            value: value(),
        })
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
        assert!(reporter
            .sample(false, || panic!("discarded samples should be lazy"))
            .is_none());
        let second = reporter.sample(true, || 2).unwrap();
        assert_eq!((first.sequence, first.value), (1, 1));
        assert_eq!((second.sequence, second.value), (2, 2));
        assert!(first.at <= second.at);
    }
}
