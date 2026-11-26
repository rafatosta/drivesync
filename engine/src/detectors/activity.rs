//! Internal account/root-scoped activity contract exposed by the Detectors facade.

use crate::models::{AccountId, Path, SyncRootId};
use std::time::Instant;

/// Identity of an activity within one Orchestrator lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ActivityId(pub(crate) u64);

/// Scope fixed by the synchronization instance, independent of functional capture epochs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Scope {
    pub(crate) account: AccountId,
    pub(crate) root: SyncRootId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Origin {
    Local,
    Remote,
    Shared,
}

/// Normalized work that can be understood without access to a provider response or observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    Initialize,
    Prepare,
    Observe,
    Traverse,
    Recover,
    Checksum,
    CompareSnapshots,
    BuildObservations,
    Deliver,
    Idle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Wait {
    Worker,
    Ownership,
    Authorization,
    Queue,
    Refresh,
    Retry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum State {
    Running,
    Waiting(Wait),
    Complete,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unit {
    Files,
    Observations,
    EntriesVisited,
    EntriesCompleted,
    Directories,
    SnapshotItems,
    ChecksumsReused,
    ItemsCompared,
    PagesReceived,
    Bytes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Progress {
    pub(crate) unit: Unit,
    pub(crate) completed: u64,
    pub(crate) total: Option<u64>,
}

impl Progress {
    pub(crate) fn new(unit: Unit, completed: usize, total: Option<usize>) -> Self {
        Self {
            unit,
            completed: completed as u64,
            total: total.map(|value| value as u64),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reason {
    CoverageLost,
    NamespaceChanged,
    NoCurrentWatch,
    BudgetReached,
    IncompleteObservation,
}

/// Latest known details. Paths are root-relative display positions, with no provider identifiers.
#[derive(Debug, Clone, Default)]
pub(crate) struct Details {
    pub(crate) reason: Option<Reason>,
    pub(crate) item: Option<Path>,
    pub(crate) progress: [Option<Progress>; 5],
}

impl Details {
    pub(crate) fn count(progress: Progress) -> Self {
        Self {
            progress: [Some(progress), None, None, None, None],
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Activity {
    pub(crate) id: ActivityId,
    pub(crate) parent: Option<ActivityId>,
    pub(crate) origin: Origin,
    pub(crate) phase: Phase,
    pub(crate) state: State,
    pub(crate) details: Details,
    pub(crate) sequence: u64,
    pub(crate) started: Instant,
    pub(crate) updated: Instant,
}

use super::observation::progress::{ActivityReporter, Sample, ScanProgress};
use crate::synchronization::{SynchronizationScan, SynchronizationScanState, SynchronizationWork};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tokio::sync::Notify;

/// A source owns a lifecycle even when its work includes shared Context activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Source {
    Local,
    Remote,
}

impl Source {
    fn index(self) -> usize {
        match self {
            Self::Local => 0,
            Self::Remote => 1,
        }
    }

    pub(crate) fn origin(self) -> Origin {
        match self {
            Self::Local => Origin::Local,
            Self::Remote => Origin::Remote,
        }
    }
}

pub(crate) enum ScanUpdate {
    Progress(ScanProgress),
    State(SynchronizationScanState, Option<u64>),
}

const MAX_ACTIVE: usize = 128;

#[derive(Default)]
struct Slot {
    generation: Option<ActivityId>,
    scan: Option<SynchronizationScan>,
    dirty: bool,
}

struct Active {
    source: Source,
    generation: ActivityId,
    activity: Activity,
}

#[derive(Default)]
struct Latest {
    closed: bool,
    last_id: u64,
    sources: [Slot; 2],
    active: BTreeMap<ActivityId, Active>,
}

impl Latest {
    fn next_id(&mut self) -> ActivityId {
        self.last_id += 1;
        ActivityId(self.last_id)
    }
}

/// Per-Orchestrator latest-state mailbox. Producers never take the public watch lock.
/// The supervisor only holds this lock while taking changed slots; no callbacks run under it.
pub(crate) struct ActivityChannel {
    pub(crate) scope: Scope,
    latest: Mutex<Latest>,
    changed: Notify,
}

impl ActivityChannel {
    pub(crate) fn new(scope: Scope) -> Arc<Self> {
        Arc::new(Self {
            scope,
            latest: Mutex::default(),
            changed: Notify::new(),
        })
    }

    pub(crate) fn open(self: &Arc<Self>, source: Source) -> ActivitySession {
        let mut latest = self.latest.lock().unwrap();
        let generation = latest.next_id();
        if !latest.closed {
            latest.active.retain(|_, active| active.source != source);
            latest.sources[source.index()].generation = Some(generation);
        }
        ActivitySession(ActivityPublisher {
            channel: self.clone(),
            source,
            generation,
        })
    }

    pub(crate) fn close(&self) {
        let mut latest = self.latest.lock().unwrap();
        latest.closed = true;
        latest.active.clear();
        latest.sources = Default::default();
    }

    pub(crate) async fn changed(&self) {
        self.changed.notified().await;
    }

    /// Take slot changes before touching the public status, preserving unrelated work fields.
    pub(crate) fn take_scan_changes(&self) -> ScanChanges {
        let mut latest = self.latest.lock().unwrap();
        ScanChanges(
            latest
                .sources
                .each_mut()
                .map(|slot| std::mem::take(&mut slot.dirty).then(|| slot.scan.clone())),
        )
    }

    fn scan(&self, source: Source, generation: ActivityId, update: ScanUpdate) {
        let changed = {
            let mut latest = self.latest.lock().unwrap();
            if latest.closed {
                return;
            }
            let slot = &mut latest.sources[source.index()];
            if slot.generation != Some(generation) {
                return;
            }
            apply_scan(source, slot, update)
        };
        if changed {
            self.changed.notify_one();
        }
    }
}

pub(crate) struct ScanChanges([Option<Option<SynchronizationScan>>; 2]);

impl ScanChanges {
    pub(crate) fn project(self, work: &mut SynchronizationWork) -> bool {
        let mut changed = false;
        let [local, remote] = self.0;
        for (update, target) in [(local, &mut work.local), (remote, &mut work.remote)] {
            if let Some(update) = update {
                changed |= *target != update;
                *target = update;
            }
        }
        changed
    }
}

fn apply_scan(source: Source, slot: &mut Slot, update: ScanUpdate) -> bool {
    use SynchronizationScanState::{Reading, RetryScheduled};
    let next = match update {
        ScanUpdate::Progress(ScanProgress { path, items, pages }) => SynchronizationScan {
            state: Reading,
            path,
            items,
            pages,
            retry_at: None,
        },
        ScanUpdate::State(state, items) => {
            let mut next = slot.scan.clone().unwrap_or(SynchronizationScan {
                state,
                path: None,
                items: 0,
                pages: 0,
                retry_at: None,
            });
            if state == Reading && next.state != state {
                next.path = None;
                next.items = 0;
                next.pages = 0;
            }
            next.state = state;
            if let Some(items) = items {
                next.items = items;
            }
            next.retry_at = (state == RetryScheduled).then(|| crate::models::DateTime {
                unix_millis: crate::orchestrator::timestamp().unix_millis
                    + match source {
                        Source::Local => 2_000,
                        Source::Remote => 15_000,
                    },
            });
            next
        }
    };
    if slot.scan.as_ref() == Some(&next) {
        return false;
    }
    slot.scan = Some(next);
    slot.dirty = true;
    true
}

/// Owned by the source supervisor. Dropping it invalidates all reporters of this generation.
pub(crate) struct ActivitySession(ActivityPublisher);

impl ActivitySession {
    pub(crate) fn publisher(&self) -> ActivityPublisher {
        self.0.clone()
    }
}

impl Drop for ActivitySession {
    fn drop(&mut self) {
        let mut latest = self.0.channel.latest.lock().unwrap();
        let slot = &mut latest.sources[self.0.source.index()];
        if slot.generation == Some(self.0.generation) {
            slot.generation = None;
            latest
                .active
                .retain(|_, active| active.generation != self.0.generation);
        }
    }
}

#[derive(Clone)]
pub(crate) struct ActivityPublisher {
    channel: Arc<ActivityChannel>,
    source: Source,
    generation: ActivityId,
}

impl ActivityPublisher {
    pub(crate) fn scan(&self, update: ScanUpdate) {
        self.channel.scan(self.source, self.generation, update);
    }

    pub(crate) fn begin(
        &self,
        origin: Origin,
        parent: Option<ActivityId>,
        phase: Phase,
        state: State,
        details: Details,
    ) -> ActivityGuard {
        let id = {
            let mut latest = self.channel.latest.lock().unwrap();
            if latest.sources[self.source.index()].generation != Some(self.generation)
                || latest.active.len() >= MAX_ACTIVE
            {
                return ActivityGuard {
                    publisher: self.clone(),
                    origin,
                    id: None,
                    reporter: None,
                };
            }
            let id = latest.next_id();
            let now = Instant::now();
            latest.active.insert(
                id,
                Active {
                    source: self.source,
                    generation: self.generation,
                    activity: Activity {
                        id,
                        parent,
                        origin,
                        phase,
                        state,
                        details,
                        sequence: 0,
                        started: now,
                        updated: now,
                    },
                },
            );
            id
        };
        let publisher = self.clone();
        ActivityGuard {
            publisher: self.clone(),
            origin,
            id: Some(id),
            reporter: Some(ActivityReporter::new(move |sample| {
                publisher.update(id, sample)
            })),
        }
    }

    fn update(&self, id: ActivityId, sample: Sample<Reported>) {
        let scan_changed = {
            let mut latest = self.channel.latest.lock().unwrap();
            let Some(active) = latest.active.get_mut(&id) else {
                return;
            };
            let activity = &mut active.activity;
            if sample.sequence <= activity.sequence {
                return;
            }
            activity.sequence = sample.sequence;
            activity.updated = sample.at;
            activity.state = sample.value.state;
            activity.details = sample.value.details;
            sample.value.scan.is_some_and(|scan| {
                apply_scan(
                    self.source,
                    &mut latest.sources[self.source.index()],
                    ScanUpdate::Progress(scan),
                )
            })
        };
        if scan_changed {
            self.channel.changed.notify_one();
        }
    }

    fn finish(&self, id: ActivityId, _state: State) {
        self.channel.latest.lock().unwrap().active.remove(&id);
    }
}

struct Reported {
    state: State,
    details: Details,
    scan: Option<ScanProgress>,
}

/// An activity cannot stay active when its future or blocking scope is dropped.
pub(crate) struct ActivityGuard {
    publisher: ActivityPublisher,
    origin: Origin,
    id: Option<ActivityId>,
    reporter: Option<ActivityReporter<Reported>>,
}

impl ActivityGuard {
    pub(crate) fn child(&self, phase: Phase, state: State, details: Details) -> Self {
        self.child_from(self.origin, phase, state, details)
    }

    pub(crate) fn child_from(
        &self,
        origin: Origin,
        phase: Phase,
        state: State,
        details: Details,
    ) -> Self {
        self.publisher.begin(origin, self.id, phase, state, details)
    }

    pub(crate) fn report(&self, details: impl FnOnce() -> Details) {
        if let Some(reporter) = &self.reporter {
            reporter.report(|| Reported {
                state: State::Running,
                details: details(),
                scan: None,
            });
        }
    }

    pub(crate) fn report_scan(
        &self,
        immediate: bool,
        details: impl FnOnce() -> (Details, ScanProgress),
    ) {
        if let Some(reporter) = &self.reporter {
            reporter.publish(immediate, || {
                let (details, scan) = details();
                Reported {
                    state: State::Running,
                    details,
                    scan: Some(scan),
                }
            });
        }
    }

    pub(crate) fn transition(&self, state: State, details: impl FnOnce() -> Details) {
        if let Some(reporter) = &self.reporter {
            reporter.publish(true, || Reported {
                state,
                details: details(),
                scan: None,
            });
        }
    }

    pub(crate) fn finish(mut self, state: State) {
        if let Some(id) = self.id.take() {
            self.publisher.finish(id, state);
        }
    }
}

impl Drop for ActivityGuard {
    fn drop(&mut self) {
        if let Some(id) = self.id {
            self.publisher.finish(
                id,
                if std::thread::panicking() {
                    State::Failed
                } else {
                    State::Cancelled
                },
            );
        }
    }
}
