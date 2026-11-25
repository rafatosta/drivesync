//! Bounded display telemetry, independent of normalized observations and execution admission.
use crate::models::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub(crate) struct ProgressReporter {
    callback: Arc<dyn Fn(Option<Path>, u64, u64) + Send + Sync>,
    last: Arc<Mutex<Option<Instant>>>,
}
impl std::fmt::Debug for ProgressReporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProgressReporter")
    }
}
impl ProgressReporter {
    pub(crate) fn new(callback: impl Fn(Option<Path>, u64, u64) + Send + Sync + 'static) -> Self {
        Self {
            callback: Arc::new(callback),
            last: Arc::new(Mutex::new(None)),
        }
    }
    pub(crate) fn finish(&self, path: Option<Path>, items: u64, pages: u64) {
        (self.callback)(path, items, pages);
    }
    pub(crate) fn report(&self, path: Option<Path>, items: u64, pages: u64) {
        let mut last = self.last.lock().unwrap();
        if last.is_some_and(|time| time.elapsed() < Duration::from_millis(200)) {
            return;
        }
        *last = Some(Instant::now());
        (self.callback)(path, items, pages);
    }
}
