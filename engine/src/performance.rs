//! Opt-in timings for comparing synchronization workloads without logging account data.
use std::sync::OnceLock;
use std::time::Instant;

pub(crate) struct Sample {
    phase: &'static str,
    start: Option<Instant>,
}

impl Sample {
    pub(crate) fn begin(phase: &'static str) -> Self {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        let enabled = *ENABLED.get_or_init(|| {
            std::env::var_os("DRIVESYNC_MEASURE_SYNC").is_some_and(|value| value == "1")
        });
        Self {
            phase,
            start: enabled.then(Instant::now),
        }
    }
}

impl Drop for Sample {
    fn drop(&mut self) {
        if let Some(start) = self.start {
            eprintln!(
                "drivesync-perf {} {}",
                self.phase,
                start.elapsed().as_nanos()
            );
        }
    }
}
