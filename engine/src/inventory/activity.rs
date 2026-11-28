//! Account and root scoped reporting for Inventory work.

use crate::{
    activity::{ActivityId, DiagnosticWriter, Recent},
    models::{AccountId, SyncRootId},
};
use std::{
    io::Write,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::sync::Notify;

const ACTIVE_LIMIT: usize = 128;
const UPDATE_LIMIT: usize = 256;
const LOOKUP_NOTIFICATION_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Origin {
    Initialization,
    Detectors,
    Operations,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Open,
    Access,
    Lookup,
    Enumerate,
    Write(Persistence),
    Lease,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Persistence {
    Identity,
    Baseline,
    PendingOperation,
    RecoveryCheckpoint,
    RecoveryAssessment,
    OperationEffect,
    OperationConfirmation,
    OperationFailure,
    Conflict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LookupOutcome {
    Found,
    NotFound,
    Rejected,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum Phase {
    OpenStorage,
    Prepare,
    Acquire,
    Query,
    ReadRecords,
    Validate,
    Transaction,
    Apply,
    Commit,
    Rollback,
    Lease,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum State {
    Running,
    WaitingForMutex,
    Complete,
    NotFound,
    Rejected,
    Unavailable,
    Failed,
    RolledBack,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Progress {
    pub processed: u64,
    pub total: Option<u64>,
}

#[derive(Debug, Clone)]
pub(crate) struct Activity {
    pub id: ActivityId,
    pub parent: Option<ActivityId>,
    pub correlation: Option<ActivityId>,
    pub origin: Origin,
    pub kind: Kind,
    pub phase: Phase,
    pub state: State,
    pub progress: Option<Progress>,
    pub sequence: u64,
    published_order: u64,
    pub started: Instant,
    pub updated: Instant,
}

#[derive(Debug, Clone)]
pub(crate) struct Scope {
    pub account: AccountId,
    pub root: Option<SyncRootId>,
}

struct Latest {
    closed: bool,
    next: u64,
    active: Vec<Activity>,
    updates: Recent<Activity>,
    omitted: u64,
    overwritten: u64,
    lookups: LookupSummary,
    lookups_notified_at: Option<Instant>,
    next_update: u64,
    diagnostic_cursor: u64,
    diagnostic_lookups: LookupSummary,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct LookupSummary {
    pub total: u64,
    pub found: u64,
    pub not_found: u64,
    pub rejected: u64,
    pub unavailable: u64,
}

impl LookupSummary {
    fn record(&mut self, outcome: LookupOutcome) {
        self.total = self.total.saturating_add(1);
        let count = match outcome {
            LookupOutcome::Found => &mut self.found,
            LookupOutcome::NotFound => &mut self.not_found,
            LookupOutcome::Rejected => &mut self.rejected,
            LookupOutcome::Unavailable => &mut self.unavailable,
        };
        *count = count.saturating_add(1);
    }
}

impl Latest {
    fn publish(&mut self, mut activity: Activity) {
        self.next_update = self.next_update.saturating_add(1);
        activity.published_order = self.next_update;
        let overwritten = self.updates.push(activity);
        self.overwritten = self.overwritten.saturating_add(u64::from(overwritten));
    }
}

struct Channel {
    scope: Arc<Scope>,
    label: u64,
    trace: bool,
    latest: Mutex<Latest>,
    changed: Notify,
}

#[derive(Clone)]
pub(crate) struct ActivityInbox(Arc<Channel>);

impl ActivityInbox {
    pub(crate) fn new(scope: Scope) -> Self {
        static NEXT_SCOPE: AtomicU64 = AtomicU64::new(1);
        Self(Arc::new(Channel {
            scope: Arc::new(scope),
            label: NEXT_SCOPE.fetch_add(1, Ordering::Relaxed),
            trace: std::env::var_os("DRIVESYNC_TRACE_INVENTORY").is_some_and(|value| value == "1"),
            latest: Mutex::new(Latest {
                closed: false,
                next: 0,
                active: Vec::new(),
                updates: Recent::new(UPDATE_LIMIT),
                omitted: 0,
                overwritten: 0,
                lookups: LookupSummary::default(),
                lookups_notified_at: None,
                next_update: 0,
                diagnostic_cursor: 0,
                diagnostic_lookups: LookupSummary::default(),
            }),
            changed: Notify::new(),
        }))
    }

    pub(crate) fn scope(&self) -> &Scope {
        &self.0.scope
    }

    pub(crate) async fn changed(&self) {
        self.0.changed.notified().await;
    }

    pub(crate) fn current(&self) -> Vec<Activity> {
        self.0
            .latest
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .active
            .clone()
    }

    pub(crate) fn recent(&self) -> Vec<Activity> {
        self.0
            .latest
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .updates
            .snapshot()
    }

    pub(crate) fn lookup_summary(&self) -> LookupSummary {
        let latest = self.0.latest.lock().unwrap_or_else(|e| e.into_inner());
        LookupSummary {
            total: latest.lookups.total,
            found: latest.lookups.found,
            not_found: latest.lookups.not_found,
            rejected: latest.lookups.rejected,
            unavailable: latest.lookups.unavailable,
        }
    }

    pub(crate) fn close(&self) {
        let mut latest = self.0.latest.lock().unwrap_or_else(|e| e.into_inner());
        latest.closed = true;
        let active = std::mem::take(&mut latest.active);
        for mut activity in active {
            activity.state = State::Cancelled;
            activity.sequence = activity.sequence.saturating_add(1);
            activity.updated = Instant::now();
            latest.publish(activity);
        }
        drop(latest);
        self.0.changed.notify_one();
    }

    pub(crate) fn consume(&self) {
        if !self.0.trace {
            return;
        }
        static WRITER: DiagnosticWriter = DiagnosticWriter::new();
        let channel = self.0.clone();
        let wake = self.0.clone();
        WRITER.spawn(move || channel.flush(), move || wake.changed.notify_one());
    }

    fn begin(&self, origin: Origin, kind: Kind, phase: Phase) -> Option<Guard> {
        self.begin_at(origin, kind, phase, State::Running, Instant::now())
    }

    fn begin_at(
        &self,
        origin: Origin,
        kind: Kind,
        phase: Phase,
        state: State,
        started: Instant,
    ) -> Option<Guard> {
        let mut latest = self.0.latest.lock().unwrap_or_else(|e| e.into_inner());
        if latest.closed || latest.active.len() >= ACTIVE_LIMIT {
            latest.omitted = latest.omitted.saturating_add(1);
            return None;
        }
        let now = Instant::now();
        let Some(next) = latest.next.checked_add(1) else {
            latest.omitted = latest.omitted.saturating_add(1);
            return None;
        };
        let id = ActivityId(next);
        latest.next = next;
        let activity = Activity {
            id,
            parent: None,
            correlation: None,
            origin,
            kind,
            phase,
            state,
            progress: None,
            sequence: 0,
            published_order: 0,
            started,
            updated: now,
        };
        latest.active.push(activity.clone());
        latest.publish(activity);
        drop(latest);
        self.0.changed.notify_one();
        Some(Guard {
            inbox: self.clone(),
            id,
            phase,
            reporter: crate::activity::ActivityReporter::default(),
            closed: false,
        })
    }

    pub(crate) fn open(&self, origin: Origin) -> Option<Guard> {
        self.begin(origin, Kind::Open, Phase::OpenStorage)
    }
    pub(crate) fn slow_lookup(&self, origin: Origin, started: Instant) -> Option<Guard> {
        self.begin_at(origin, Kind::Lookup, Phase::Query, State::Running, started)
    }
    pub(crate) fn enumerate(&self, origin: Origin) -> Option<Guard> {
        self.begin(origin, Kind::Enumerate, Phase::ReadRecords)
    }
    pub(crate) fn write(&self, origin: Origin, persistence: Persistence) -> Option<Guard> {
        self.begin(origin, Kind::Write(persistence), Phase::Validate)
    }
    pub(crate) fn lease(&self, origin: Origin) -> Option<Guard> {
        self.begin(origin, Kind::Lease, Phase::Acquire)
    }

    pub(crate) fn record_lookup(&self, outcome: LookupOutcome) {
        let mut latest = self.0.latest.lock().unwrap_or_else(|e| e.into_inner());
        latest.lookups.record(outcome);
        let now = Instant::now();
        let notify = latest
            .lookups_notified_at
            .is_none_or(|last| now.saturating_duration_since(last) >= LOOKUP_NOTIFICATION_INTERVAL);
        if notify {
            latest.lookups_notified_at = Some(now);
        }
        drop(latest);
        if notify {
            self.0.changed.notify_one();
        }
    }

    pub(crate) fn mutex_wait(&self, origin: Origin, elapsed: Duration) {
        let started = Instant::now()
            .checked_sub(elapsed)
            .unwrap_or_else(Instant::now);
        if let Some(activity) = self.begin_at(
            origin,
            Kind::Access,
            Phase::Acquire,
            State::WaitingForMutex,
            started,
        ) {
            activity.finish(State::Complete);
        }
    }
}

impl Channel {
    fn finish(&self, id: ActivityId, state: State, phase: Phase, progress: Option<Progress>) {
        let mut latest = self.latest.lock().unwrap_or_else(|e| e.into_inner());
        let Some(index) = latest.active.iter().position(|activity| activity.id == id) else {
            return;
        };
        let mut activity = latest.active.remove(index);
        activity.phase = phase;
        activity.state = state;
        activity.progress = progress;
        activity.sequence = activity.sequence.saturating_add(1);
        activity.updated = Instant::now();
        latest.publish(activity);
        drop(latest);
        self.changed.notify_one();
    }

    fn update(
        &self,
        id: ActivityId,
        sample: crate::activity::Sample<(Phase, State, Option<Progress>)>,
    ) {
        let mut latest = self.latest.lock().unwrap_or_else(|e| e.into_inner());
        let Some(activity) = latest.active.iter_mut().find(|activity| activity.id == id) else {
            return;
        };
        if sample.sequence <= activity.sequence {
            return;
        }
        (activity.phase, activity.state, activity.progress) = sample.value;
        activity.sequence = sample.sequence;
        activity.updated = sample.at;
        let event = activity.clone();
        latest.publish(event);
        drop(latest);
        self.changed.notify_one();
    }

    fn flush(&self) {
        let (records, omitted, lookups, diagnostic_lookups, overwritten, cursor) = {
            let mut latest = self.latest.lock().unwrap_or_else(|e| e.into_inner());
            (
                latest.updates.snapshot(),
                std::mem::take(&mut latest.omitted),
                latest.lookups,
                latest.diagnostic_lookups,
                std::mem::take(&mut latest.overwritten),
                latest.diagnostic_cursor,
            )
        };
        let mut stderr = std::io::stderr().lock();
        let mut last_written = cursor;
        for activity in records
            .into_iter()
            .filter(|activity| activity.published_order > cursor)
        {
            let elapsed = activity.updated.saturating_duration_since(activity.started);
            let _ = writeln!(stderr, "drivesync-inventory scope={} account={:016x} root={} activity={} parent={:?} correlation={:?} origin={:?} kind={:?} phase={:?} state={:?} sequence={} elapsed_ms={} progress={:?}", self.label, hash_label(&self.scope.account.0), self.scope.root.as_ref().map(|root| format!("{:016x}", hash_label(&root.0))).unwrap_or_else(|| "account".into()), activity.id.0, activity.parent, activity.correlation, activity.origin, activity.kind, activity.phase, activity.state, activity.sequence, elapsed.as_millis(), activity.progress);
            last_written = last_written.max(activity.published_order);
        }
        let lookup_delta = LookupSummary {
            total: lookups.total.saturating_sub(diagnostic_lookups.total),
            found: lookups.found.saturating_sub(diagnostic_lookups.found),
            not_found: lookups
                .not_found
                .saturating_sub(diagnostic_lookups.not_found),
            rejected: lookups.rejected.saturating_sub(diagnostic_lookups.rejected),
            unavailable: lookups
                .unavailable
                .saturating_sub(diagnostic_lookups.unavailable),
        };
        if omitted != 0 || lookup_delta.total != 0 || overwritten != 0 {
            let _ = writeln!(
                stderr,
                "drivesync-inventory scope={} omitted={} overwritten={} lookups={} found={} not_found={} rejected={} unavailable={}",
                self.label, omitted, overwritten, lookup_delta.total, lookup_delta.found, lookup_delta.not_found, lookup_delta.rejected, lookup_delta.unavailable
            );
        }
        drop(stderr);
        let mut latest = self.latest.lock().unwrap_or_else(|e| e.into_inner());
        latest.diagnostic_cursor = latest.diagnostic_cursor.max(last_written);
        latest.diagnostic_lookups = lookups;
    }
}

fn hash_label(value: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

pub(crate) struct Guard {
    inbox: ActivityInbox,
    id: ActivityId,
    phase: Phase,
    reporter: crate::activity::ActivityReporter,
    closed: bool,
}

impl Guard {
    pub(crate) fn phase(&mut self, phase: Phase, state: State) {
        self.phase = phase;
        let sample = self.reporter.transition((phase, state, None));
        self.inbox.0.update(self.id, sample);
    }

    pub(crate) fn progress(&mut self, processed: u64, total: Option<u64>) {
        let progress = Some(Progress { processed, total });
        if let Some(sample) = self
            .reporter
            .sample(false, || (self.phase, State::Running, progress))
        {
            self.inbox.0.update(self.id, sample);
        }
    }

    pub(crate) fn finish(mut self, state: State) {
        self.inbox.0.finish(self.id, state, self.phase, None);
        self.closed = true;
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if !self.closed {
            self.inbox
                .0
                .finish(self.id, State::Cancelled, self.phase, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inbox() -> ActivityInbox {
        ActivityInbox::new(Scope {
            account: AccountId("account-test".into()),
            root: Some(SyncRootId("root-test".into())),
        })
    }

    #[test]
    fn lifecycle_is_ordered_and_terminal_activity_is_removed() {
        let inbox = inbox();
        let mut activity = inbox.enumerate(Origin::Detectors).unwrap();
        activity.phase(Phase::ReadRecords, State::Running);
        activity.progress(3, Some(8));
        activity.finish(State::Complete);
        assert!(inbox.current().is_empty());
        let updates = inbox.0.latest.lock().unwrap().updates.take();
        assert!(updates.windows(2).all(|pair| pair[0].id <= pair[1].id));
        assert_eq!(updates.last().unwrap().state, State::Complete);
        assert_eq!(updates.last().unwrap().origin, Origin::Detectors);
    }

    #[test]
    fn closing_retires_active_work_and_rejects_late_publications() {
        let inbox = inbox();
        let activity = inbox
            .write(Origin::Operations, Persistence::PendingOperation)
            .unwrap();
        inbox.close();
        assert!(inbox.current().is_empty());
        activity.finish(State::Complete);
        let updates = inbox.0.latest.lock().unwrap().updates.take();
        assert_eq!(updates.last().unwrap().state, State::Cancelled);
        assert!(inbox
            .write(Origin::Operations, Persistence::PendingOperation)
            .is_none());
    }

    #[test]
    fn active_and_recent_storage_are_bounded() {
        let inbox = inbox();
        let active = (0..ACTIVE_LIMIT)
            .map(|_| inbox.lease(Origin::Operations).unwrap())
            .collect::<Vec<_>>();
        assert!(inbox.lease(Origin::Operations).is_none());
        assert_eq!(inbox.current().len(), ACTIVE_LIMIT);
        drop(active);
        let mut recent = inbox.0.latest.lock().unwrap();
        assert_eq!(recent.active.len(), 0);
        assert!(recent.omitted > 0);
        assert!(recent.updates.take().len() <= UPDATE_LIMIT);
    }

    #[test]
    fn scopes_allow_account_only_and_distinguish_roots() {
        let account = ActivityInbox::new(Scope {
            account: AccountId("account-test".into()),
            root: None,
        });
        let rooted = inbox();
        assert!(account.scope().root.is_none());
        assert_ne!(account.scope().root, rooted.scope().root);
        assert_ne!(account.scope().account, AccountId("other".into()));
    }

    #[test]
    fn concurrent_producers_keep_per_activity_sequence_monotonic() {
        let inbox = inbox();
        let mut workers = Vec::new();
        for _ in 0..8 {
            let inbox = inbox.clone();
            workers.push(std::thread::spawn(move || {
                for _ in 0..40 {
                    let activity = inbox.lease(Origin::Operations).unwrap();
                    activity.finish(State::Complete);
                }
            }));
        }
        for worker in workers {
            worker.join().unwrap();
        }
        assert!(inbox.current().is_empty());
        let updates = inbox.recent();
        let mut sequences = std::collections::BTreeMap::new();
        for update in updates {
            if let Some(previous) = sequences.get(&update.id) {
                assert!(update.sequence > *previous);
            }
            sequences.insert(update.id, update.sequence);
        }
    }

    #[test]
    fn lookup_outcomes_are_consolidated_without_per_query_activities() {
        let inbox = inbox();
        for outcome in [
            LookupOutcome::Found,
            LookupOutcome::NotFound,
            LookupOutcome::Rejected,
            LookupOutcome::Unavailable,
        ] {
            inbox.record_lookup(outcome);
        }
        assert!(inbox.current().is_empty());
        let summary = inbox.lookup_summary();
        assert_eq!(summary.total, 4);
        assert_eq!(summary.found, 1);
        assert_eq!(summary.not_found, 1);
        assert_eq!(summary.rejected, 1);
        assert_eq!(summary.unavailable, 1);
    }

    #[tokio::test]
    async fn lookup_summary_notifies_without_per_query_activity_records() {
        let inbox = inbox();
        let changed = inbox.changed();
        inbox.record_lookup(LookupOutcome::NotFound);
        tokio::time::timeout(Duration::from_millis(50), changed)
            .await
            .unwrap();
        assert!(inbox.recent().is_empty());
        assert_eq!(inbox.lookup_summary().not_found, 1);
    }

    #[test]
    fn repeated_short_lookup_cost_with_and_without_activity_reporting() {
        use crate::inventory::{
            models::{InventoryLookup, InventoryLookupStatus},
            Inventory, InventoryConfig,
        };
        use crate::models::{Evidence, RemoteId};

        const ITERATIONS: u32 = 10_000;
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "drivesync-inventory-observability-cost-{}-{}",
            std::process::id(),
            nonce
        ));
        let account = AccountId("inventory-cost-account".into());
        let config = || InventoryConfig {
            directory: directory.clone(),
            account_id: account.clone(),
        };
        let plain = Inventory::open(config()).unwrap();
        let inbox = ActivityInbox::new(Scope {
            account: account.clone(),
            root: Some(SyncRootId("inventory-cost-root".into())),
        });
        let observed =
            Inventory::open_observed(config(), inbox.clone(), Origin::Detectors).unwrap();
        let request = || InventoryLookup {
            account_id: account.clone(),
            sync_root_id: SyncRootId("inventory-cost-root".into()),
            item_id: None,
            remote_id: Some(RemoteId("missing-record".into())),
            local_identity: Evidence::NotCollected,
            local_path: None,
        };
        for _ in 0..1_000 {
            assert_eq!(
                plain.lookup(request()).status,
                InventoryLookupStatus::NotFound
            );
            assert_eq!(
                observed.lookup(request()).status,
                InventoryLookupStatus::NotFound
            );
        }
        let measure = |inventory: &Inventory| {
            let started = Instant::now();
            for _ in 0..ITERATIONS {
                std::hint::black_box(inventory.lookup(request()).status);
            }
            started.elapsed().as_nanos() / u128::from(ITERATIONS)
        };
        let (mut plain_samples, mut observed_samples, mut paired_deltas) =
            (Vec::new(), Vec::new(), Vec::new());
        for round in 0..7 {
            if round % 2 == 0 {
                let plain_sample = measure(&plain);
                let observed_sample = measure(&observed);
                plain_samples.push(plain_sample);
                observed_samples.push(observed_sample);
                paired_deltas.push(observed_sample as i128 - plain_sample as i128);
            } else {
                let observed_sample = measure(&observed);
                let plain_sample = measure(&plain);
                observed_samples.push(observed_sample);
                plain_samples.push(plain_sample);
                paired_deltas.push(observed_sample as i128 - plain_sample as i128);
            }
        }
        plain_samples.sort_unstable();
        observed_samples.sort_unstable();
        paired_deltas.sort_unstable();
        let plain_median = plain_samples[plain_samples.len() / 2];
        let observed_median = observed_samples[observed_samples.len() / 2];
        let paired_delta_median = paired_deltas[paired_deltas.len() / 2];
        eprintln!(
            "inventory-observability-cost rounds=7 lookups_per_round={ITERATIONS} plain_median_ns_per_lookup={plain_median} observed_median_ns_per_lookup={observed_median} paired_median_delta_ns_per_lookup={paired_delta_median}"
        );
        assert_eq!(
            plain.lookup(request()).status,
            observed.lookup(request()).status
        );
        drop((plain, observed, inbox));
        std::fs::remove_dir_all(directory).unwrap();
    }
}
