//! Sampled activity publication independent of observations and execution admission.
use crate::models::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const INTERVAL: Duration = Duration::from_millis(200);

/// The sink uses this sequence to reject samples delivered out of order.
pub(crate) struct Sample<T> {
    pub sequence: u64,
    pub at: Instant,
    pub value: T,
}

#[derive(Default)]
struct Sampling {
    last: Option<Instant>,
    sequence: u64,
}

pub(crate) struct ActivityReporter<T> {
    callback: Arc<dyn Fn(Sample<T>) + Send + Sync>,
    sampling: Arc<Mutex<Sampling>>,
}

impl<T> Clone for ActivityReporter<T> {
    fn clone(&self) -> Self {
        Self {
            callback: self.callback.clone(),
            sampling: self.sampling.clone(),
        }
    }
}

impl<T> std::fmt::Debug for ActivityReporter<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ActivityReporter")
    }
}

impl<T> ActivityReporter<T> {
    pub(crate) fn new(callback: impl Fn(Sample<T>) + Send + Sync + 'static) -> Self {
        Self {
            callback: Arc::new(callback),
            sampling: Arc::default(),
        }
    }

    pub(crate) fn report(&self, value: impl FnOnce() -> T) {
        self.publish(false, value);
    }

    pub(crate) fn publish(&self, immediate: bool, value: impl FnOnce() -> T) {
        let (sequence, at) = {
            let mut sampling = self.sampling.lock().unwrap();
            let at = Instant::now();
            if !immediate && sampling.last.is_some_and(|last| at - last < INTERVAL) {
                return;
            }
            sampling.last = Some(at);
            sampling.sequence += 1;
            (sampling.sequence, at)
        };
        (self.callback)(Sample {
            sequence,
            at,
            value: value(),
        });
    }
}

/// Fields carried by the existing public scan projection during producer migration.
#[derive(Debug)]
pub(crate) struct ScanProgress {
    pub path: Option<Path>,
    pub items: u64,
    pub pages: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callbacks_run_outside_sampling_lock_and_carry_ordering() {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let target = samples.clone();
        let reporter = ActivityReporter::new(move |sample: Sample<u64>| {
            target
                .lock()
                .unwrap()
                .push((sample.sequence, sample.at, sample.value));
        });
        reporter.report(|| 1);
        reporter.report(|| panic!("a sampled-out value should not be built"));
        reporter.publish(true, || 2);
        let samples = samples.lock().unwrap();
        assert_eq!((samples[0].0, samples[0].2), (1, 1));
        assert_eq!((samples[1].0, samples[1].2), (2, 2));
        assert!(samples[0].1 <= samples[1].1);
    }
}
