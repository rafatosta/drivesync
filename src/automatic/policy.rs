//! Deterministic scheduling rules; timestamps are monotonic seconds supplied by
//! the caller. A deadline never authorizes a second concurrent cycle.
use drivesync_engine::{AuthError, DownloadReport, EngineError};

pub(super) const INTERVAL: u64 = 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Unconfigured,
    Waiting,
    Running,
    UpToDate,
    Attention,
    Offline,
    Retrying,
    Paused,
    Blocked,
}

/// Ephemeral policy; durable progress remains exclusively in the engine journal.
#[derive(Debug)]
pub(super) struct Schedule {
    pub phase: Phase,
    pub due: Option<u64>,
    pub running: bool,
    enabled: bool,
    failures: u32,
}
impl Schedule {
    pub fn new(enabled: bool, now: u64) -> Self {
        Self {
            phase: if enabled {
                Phase::Waiting
            } else {
                Phase::Unconfigured
            },
            due: enabled.then_some(now + 3),
            running: false,
            enabled,
            failures: 0,
        }
    }
    /// Reconfiguration cannot release the running guard before worker completion.
    pub fn configure(&mut self, enabled: bool, now: u64) {
        let running = self.running;
        *self = Self::new(enabled, now);
        self.running = running;
    }
    pub fn start(&mut self, now: u64, online: bool, manual: bool) -> bool {
        if self.running || !self.enabled || (!manual && self.due.is_none_or(|due| now < due)) {
            return false;
        }
        if !online {
            self.phase = Phase::Offline;
            self.due = Some(now + 30);
            return false;
        }
        self.running = true;
        self.phase = Phase::Running;
        self.due = None;
        true
    }
    pub fn finish(&mut self, result: &Result<DownloadReport, EngineError>, now: u64) {
        self.running = false;
        if !self.enabled {
            self.phase = Phase::Unconfigured;
            self.due = None;
            return;
        }
        let errors: Vec<_> = match result {
            Ok(report) => report.issues.iter().map(|issue| &issue.error).collect(),
            Err(error) => vec![error],
        };
        if errors
            .iter()
            .any(|error| matches!(error, EngineError::Authentication(AuthError::Cancelled)))
        {
            self.phase = Phase::Paused;
            self.due = None;
        } else if errors.iter().any(|error| blocking(error)) {
            self.phase = Phase::Blocked;
            self.due = None;
        } else if let Some(error) = errors.iter().find(|error| transient(error)) {
            let delay = 30 * (1u64 << self.failures.min(4));
            self.failures = self.failures.saturating_add(1);
            self.due = Some(now + delay.min(300));
            self.phase = if matches!(error, EngineError::Authentication(AuthError::Network)) {
                Phase::Offline
            } else {
                Phase::Retrying
            };
        } else if result.is_err() {
            self.phase = Phase::Blocked;
            self.due = None;
        } else {
            self.failures = 0;
            self.phase =
                if errors.is_empty() && result.as_ref().is_ok_and(|report| report.skipped == 0) {
                    Phase::UpToDate
                } else {
                    Phase::Attention
                };
            self.due = Some(now + INTERVAL);
        }
    }
    /// A positive connectivity hint retries network failures promptly, but never
    /// bypasses rate-limit backoff, user pause or a blocking local failure.
    pub fn network_changed(&mut self, online: bool, now: u64) {
        if self.running || !self.enabled || matches!(self.phase, Phase::Paused | Phase::Blocked) {
            return;
        }
        if online && self.phase == Phase::Offline {
            self.due = Some(now);
        }
        if !online && self.phase != Phase::Retrying {
            self.phase = Phase::Offline;
            self.due = Some(now + 30);
        }
    }
}
fn transient(error: &EngineError) -> bool {
    matches!(
        error,
        EngineError::Authentication(
            AuthError::Network
                | AuthError::ProviderUnavailable
                | AuthError::TimedOut
                | AuthError::Busy
        ) | EngineError::RemoteRateLimited
            | EngineError::SyncBusy
            | EngineError::RemoteChanged
    )
}
fn blocking(error: &EngineError) -> bool {
    !transient(error)
        && !matches!(
            error,
            EngineError::DownloadConflict | EngineError::RemoteItemUnavailable
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn network() -> Result<DownloadReport, EngineError> {
        Err(EngineError::Authentication(AuthError::Network))
    }
    #[test]
    fn periodic_cycles_are_single_flight_and_timed_from_completion() {
        let mut s = Schedule::new(true, 0);
        assert!(!s.start(2, true, false));
        assert!(s.start(3, true, false));
        assert!(!s.start(999, true, true));
        s.finish(&Ok(DownloadReport::default()), 1000);
        assert_eq!(s.phase, Phase::UpToDate);
        assert!(!s.start(1059, true, false));
        assert!(s.start(1060, true, false));
    }
    #[test]
    fn retries_are_bounded_and_success_resets_backoff() {
        let mut s = Schedule::new(true, 0);
        let mut now = 3;
        for delay in [30, 60, 120, 240, 300, 300] {
            assert!(s.start(now, true, false));
            s.finish(&network(), now);
            assert_eq!(s.phase, Phase::Offline);
            assert_eq!(s.due, Some(now + delay));
            assert!(!s.start(now + delay - 1, true, false));
            now += delay;
        }
        assert!(s.start(now, true, false));
        s.finish(&Ok(DownloadReport::default()), now);
        assert!(s.start(now + 60, true, false));
        s.finish(&network(), now + 60);
        assert_eq!(s.due, Some(now + 90));
    }
    #[test]
    fn cancellation_pauses_and_only_explicit_sync_resumes() {
        let mut s = Schedule::new(true, 0);
        assert!(s.start(3, true, false));
        s.finish(&Err(EngineError::Authentication(AuthError::Cancelled)), 4);
        s.network_changed(false, 5);
        s.network_changed(true, 6);
        assert!(!s.start(10000, true, false));
        assert!(s.start(10001, true, true));
    }
    #[test]
    fn network_hints_resume_offline_but_not_rate_limits_or_blocked_errors() {
        let mut s = Schedule::new(true, 0);
        assert!(!s.start(3, false, false));
        s.network_changed(true, 4);
        assert!(s.start(4, true, false));
        s.finish(&Err(EngineError::RemoteRateLimited), 5);
        s.network_changed(false, 6);
        s.network_changed(true, 7);
        assert_eq!(s.due, Some(35));
        assert!(s.start(35, true, false));
        s.finish(&Err(EngineError::StateStorage), 36);
        s.network_changed(true, 100);
        assert!(!s.start(10000, true, false));
        assert_eq!(s.phase, Phase::Blocked);
    }
    #[test]
    fn per_item_transient_failures_retry_and_conflicts_still_poll_safely() {
        let mut s = Schedule::new(true, 0);
        let report = |error| {
            Ok(DownloadReport {
                issues: vec![drivesync_engine::DownloadIssue {
                    relative_path: Some("a.txt".into()),
                    error,
                }],
                ..Default::default()
            })
        };
        s.finish(&report(EngineError::RemoteRateLimited), 10);
        assert_eq!(s.due, Some(40));
        s.finish(&report(EngineError::DownloadConflict), 50);
        assert_eq!(s.phase, Phase::Attention);
        assert_eq!(s.due, Some(110));
        s.finish(&report(EngineError::DownloadIo), 120);
        assert_eq!(s.phase, Phase::Blocked);
    }
    #[test]
    fn permanent_issues_take_priority_over_transient_errors() {
        let mut s = Schedule::new(true, 0);
        let report = DownloadReport {
            issues: vec![
                drivesync_engine::DownloadIssue {
                    relative_path: None,
                    error: EngineError::RemoteRateLimited,
                },
                drivesync_engine::DownloadIssue {
                    relative_path: None,
                    error: EngineError::DownloadIo,
                },
            ],
            ..Default::default()
        };
        s.finish(&Ok(report), 5);
        assert_eq!(s.phase, Phase::Blocked);
        assert_eq!(s.due, None);
        s.configure(true, 10);
        s.finish(
            &Ok(DownloadReport {
                skipped: 1,
                ..Default::default()
            }),
            20,
        );
        assert_eq!(s.phase, Phase::Attention);
        assert_eq!(s.due, Some(80));
    }
    #[test]
    fn reconfiguration_and_removal_never_release_an_active_worker() {
        let mut s = Schedule::new(true, 0);
        assert!(s.start(3, true, false));
        s.configure(true, 4);
        assert!(!s.start(100, true, true));
        s.configure(false, 5);
        s.finish(&network(), 6);
        assert_eq!(s.phase, Phase::Unconfigured);
        assert!(!s.start(100, true, true));
    }
}
