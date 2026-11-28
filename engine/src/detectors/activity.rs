//! Internal account/root-scoped activity contract exposed by the Detectors facade.

mod diagnostics;
mod models;
#[cfg(test)]
mod tests;

use crate::models::Path;
use diagnostics::{Diagnostics, Event};
pub(crate) use models::*;
use std::time::Instant;

use crate::activity::{
    ActivityModule, ActivityReference, ActivityReporter, DiagnosticWriter, Sample,
};
use crate::synchronization::{SynchronizationScan, SynchronizationScanState, SynchronizationWork};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
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

pub(super) struct ScanProgress {
    pub(super) path: Option<Path>,
    pub(super) items: u64,
    pub(super) pages: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ScanState {
    Reading,
    RetryScheduled,
    Complete,
    Failed,
}

pub(super) enum ScanUpdate {
    Progress(ScanProgress),
    State(ScanState, Option<u64>),
}

const MAX_ACTIVE: usize = 128;

#[derive(Default)]
struct Slot {
    generation: Option<ActivityId>,
    scan: Option<SynchronizationScan>,
    dirty: bool,
}

struct Active {
    source: Option<Source>,
    generation: ActivityId,
    activity: Activity,
}

#[derive(Default)]
struct Latest {
    closed: bool,
    last_id: u64,
    sources: [Slot; 2],
    active: BTreeMap<ActivityId, Active>,
    diagnostics: Option<Box<Diagnostics>>,
}

impl Latest {
    fn next_id(&mut self) -> ActivityId {
        self.last_id += 1;
        ActivityId(self.last_id)
    }

    fn retire(&mut self, state: State, belongs_to: impl Fn(&Active) -> bool) {
        let diagnostics = &mut self.diagnostics;
        self.active.retain(|_, active| {
            if !belongs_to(active) {
                return true;
            }
            finish_record(diagnostics, &mut active.activity, state, 0);
            false
        });
    }
}

fn finish_record(
    diagnostics: &mut Option<Box<Diagnostics>>,
    activity: &mut Activity,
    state: State,
    coalesced: u64,
) -> bool {
    if let Some(diagnostics) = diagnostics {
        activity.state = state;
        activity.updated = Instant::now();
        activity.sequence += 1;
        diagnostics.counts.coalesced += coalesced;
        diagnostics.push(Event::Finished, activity);
        true
    } else {
        false
    }
}

/// Per-Orchestrator latest-state mailbox. Producers never take the public watch lock.
/// The supervisor only holds this lock while taking changed slots; no callbacks run under it.
pub(super) struct ActivityChannel {
    scope: Scope,
    session: u64,
    latest: Mutex<Latest>,
    changed: Notify,
}

impl ActivityChannel {
    pub(super) fn new(scope: Scope) -> Arc<Self> {
        Self::with_diagnostics(
            scope,
            std::env::var_os("DRIVESYNC_TRACE_DETECTORS").is_some_and(|value| value == "1"),
        )
    }

    pub(super) fn with_diagnostics(scope: Scope, enabled: bool) -> Arc<Self> {
        static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);
        let diagnostics = enabled.then(|| Box::new(Diagnostics::new(&scope)));
        Arc::new(Self {
            scope,
            session: NEXT_SESSION.fetch_add(1, Ordering::Relaxed),
            latest: Mutex::new(Latest {
                diagnostics,
                ..Latest::default()
            }),
            changed: Notify::new(),
        })
    }

    pub(super) fn open(self: &Arc<Self>, source: Source) -> ActivitySession {
        let (generation, cleared) = {
            let mut latest = self.latest.lock().unwrap();
            let generation = latest.next_id();
            let mut cleared = false;
            if !latest.closed {
                latest.retire(State::Cancelled, |active| active.source == Some(source));
                let slot = &mut latest.sources[source.index()];
                slot.generation = Some(generation);
                cleared = slot.scan.take().is_some();
                slot.dirty |= cleared;
            }
            (
                generation,
                cleared
                    || latest
                        .diagnostics
                        .as_ref()
                        .is_some_and(|diagnostics| diagnostics.pending()),
            )
        };
        if cleared {
            self.changed.notify_one();
        }
        ActivitySession(ActivityPublisher {
            channel: self.clone(),
            source: Some(source),
            generation,
        })
    }

    pub(super) fn shared(self: &Arc<Self>) -> ActivityPublisher {
        ActivityPublisher {
            channel: self.clone(),
            source: None,
            generation: ActivityId(0),
        }
    }

    pub(super) fn inbox(self: &Arc<Self>) -> ActivityInbox {
        ActivityInbox(self.clone())
    }

    fn close(&self) {
        let pending = {
            let mut latest = self.latest.lock().unwrap();
            latest.closed = true;
            latest.retire(State::Cancelled, |_| true);
            latest.sources = Default::default();
            latest
                .diagnostics
                .as_ref()
                .is_some_and(|diagnostics| diagnostics.pending())
        };
        if pending {
            self.changed.notify_one();
        }
    }

    fn has_scan_changes(&self) -> bool {
        self.latest
            .lock()
            .unwrap()
            .sources
            .iter()
            .any(|slot| slot.dirty)
    }

    fn flush_diagnostics(self: &Arc<Self>) {
        // A blocked stderr cannot delay publication or shutdown. One off-thread writer keeps
        // outstanding diagnostic output bounded even when many sessions start and stop.
        static WRITER: DiagnosticWriter = DiagnosticWriter::new();
        if !self
            .latest
            .lock()
            .unwrap()
            .diagnostics
            .as_ref()
            .is_some_and(|diagnostics| diagnostics.pending())
        {
            return;
        }
        let channel = self.clone();
        let wake = self.clone();
        WRITER.spawn(
            move || {
                if let Some(batch) = channel.take_diagnostics() {
                    let _ = batch.write(std::io::stderr().lock());
                }
            },
            move || wake.changed.notify_one(),
        );
    }

    fn take_diagnostics(&self) -> Option<diagnostics::Batch> {
        self.latest.lock().unwrap().diagnostics.as_mut()?.take()
    }

    async fn changed(&self) {
        self.changed.notified().await;
    }

    /// Take slot changes before touching the public status, preserving unrelated work fields.
    fn take_scan_changes(&self) -> ScanChanges {
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

/// Read side of the contract. The Orchestrator never opens a source session or publishes activity.
#[derive(Clone)]
pub(crate) struct ActivityInbox(Arc<ActivityChannel>);

impl ActivityInbox {
    pub(crate) fn scope(&self) -> &Scope {
        &self.0.scope
    }

    pub(crate) fn close(&self) {
        self.0.close();
    }

    pub(crate) async fn changed(&self) {
        self.0.changed().await;
    }

    pub(crate) fn has_scan_changes(&self) -> bool {
        self.0.has_scan_changes()
    }

    pub(crate) fn flush_diagnostics(&self) {
        self.0.flush_diagnostics();
    }

    pub(crate) fn project(&self, work: &mut SynchronizationWork) -> bool {
        self.0.take_scan_changes().project(work)
    }

    #[cfg(test)]
    pub(crate) fn test_scope(scope: Scope) -> Self {
        ActivityChannel::new(scope).inbox()
    }

    #[cfg(test)]
    pub(crate) fn test_source(&self, source: Source) -> ActivitySession {
        self.0.open(source)
    }

    /// Returns the active activity snapshot for this account and synchronization root.
    pub(crate) fn current(&self) -> Vec<Activity> {
        self.0
            .latest
            .lock()
            .unwrap()
            .active
            .values()
            .map(|active| active.activity.clone())
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn test_diagnostics(&self) -> String {
        let mut output = Vec::new();
        if let Some(batch) = self.0.take_diagnostics() {
            batch.write(&mut output).unwrap();
        }
        String::from_utf8(output).unwrap()
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
            let state = match state {
                ScanState::Reading => Reading,
                ScanState::RetryScheduled => RetryScheduled,
                ScanState::Complete => SynchronizationScanState::Complete,
                ScanState::Failed => SynchronizationScanState::Failed,
            };
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

impl ActivitySession {
    pub(super) fn fail(&self) {
        self.retire(State::Failed);
    }

    fn retire(&self, state: State) {
        let changed = {
            let mut latest = self.0.channel.latest.lock().unwrap();
            let source = self.0.source.expect("a source session owns a slot");
            let slot = &mut latest.sources[source.index()];
            if slot.generation != Some(self.0.generation) {
                return;
            }
            let scan = state == State::Failed
                && apply_scan(source, slot, ScanUpdate::State(ScanState::Failed, None));
            slot.generation = None;
            latest.retire(state, |active| active.generation == self.0.generation);
            scan || latest
                .diagnostics
                .as_ref()
                .is_some_and(|diagnostics| diagnostics.pending())
        };
        if changed {
            self.0.channel.changed.notify_one();
        }
    }
}

#[cfg(test)]
impl ActivitySession {
    pub(crate) fn test_progress(&self, path: Option<Path>, items: u64, pages: u64) {
        self.0
            .scan(ScanUpdate::Progress(ScanProgress { path, items, pages }));
    }

    pub(crate) fn test_retry(&self) {
        self.0
            .scan(ScanUpdate::State(ScanState::RetryScheduled, None));
    }
}

impl Drop for ActivitySession {
    fn drop(&mut self) {
        self.retire(State::Cancelled);
    }
}

#[derive(Clone)]
pub(crate) struct ActivityPublisher {
    channel: Arc<ActivityChannel>,
    source: Option<Source>,
    generation: ActivityId,
}

impl ActivityPublisher {
    pub(super) fn scan(&self, update: ScanUpdate) {
        if let Some(source) = self.source {
            self.channel.scan(source, self.generation, update);
        }
    }

    pub(crate) fn begin(
        &self,
        origin: Origin,
        parent: Option<ActivityId>,
        phase: Phase,
        state: State,
        details: Details,
    ) -> ActivityGuard {
        let (id, changed) = {
            let mut latest = self.channel.latest.lock().unwrap();
            if latest.closed
                || self.source.is_some_and(|source| {
                    latest.sources[source.index()].generation != Some(self.generation)
                })
                || parent.is_some_and(|parent| {
                    !latest
                        .active
                        .get(&parent)
                        .is_some_and(|active| active.generation == self.generation)
                })
            {
                return self.disabled(origin);
            }
            if latest.active.len() >= MAX_ACTIVE {
                if let Some(diagnostics) = &mut latest.diagnostics {
                    diagnostics.counts.capacity += 1;
                    drop(latest);
                    self.channel.changed.notify_one();
                }
                return self.disabled(origin);
            }
            let id = latest.next_id();
            let now = Instant::now();
            let activity = Activity {
                id,
                parent,
                origin,
                phase,
                state,
                details,
                sequence: 0,
                started: now,
                updated: now,
            };
            let changed = if let Some(diagnostics) = &mut latest.diagnostics {
                diagnostics.push(Event::Started, &activity);
                true
            } else {
                false
            };
            latest.active.insert(
                id,
                Active {
                    source: self.source,
                    generation: self.generation,
                    activity,
                },
            );
            (id, changed)
        };
        if changed {
            self.channel.changed.notify_one();
        }
        ActivityGuard {
            publisher: self.clone(),
            origin,
            id: Some(id),
            reporter: ActivityReporter::default(),
        }
    }

    fn disabled(&self, origin: Origin) -> ActivityGuard {
        ActivityGuard {
            publisher: self.clone(),
            origin,
            id: None,
            reporter: ActivityReporter::default(),
        }
    }

    fn update(&self, id: ActivityId, sample: Sample<Reported>) {
        let changed = {
            let mut latest = self.channel.latest.lock().unwrap();
            let Latest {
                active,
                diagnostics,
                ..
            } = &mut *latest;
            if let Some(diagnostics) = diagnostics {
                diagnostics.counts.coalesced += sample.coalesced;
            }
            if let Some(activity) = active
                .get_mut(&id)
                .map(|active| &mut active.activity)
                .filter(|activity| sample.sequence > activity.sequence)
            {
                let event = if activity.state == sample.value.state {
                    Event::Progress
                } else {
                    Event::Transition
                };
                activity.sequence = sample.sequence;
                activity.updated = sample.at;
                activity.state = sample.value.state;
                if let Some(details) = sample.value.details {
                    activity.details = details;
                }
                let diagnostic_changed = if let Some(diagnostics) = diagnostics {
                    diagnostics.push(event, activity);
                    true
                } else {
                    false
                };
                let scan_changed =
                    self.source
                        .zip(sample.value.scan)
                        .is_some_and(|(source, scan)| {
                            apply_scan(
                                source,
                                &mut latest.sources[source.index()],
                                ScanUpdate::Progress(scan),
                            )
                        });
                diagnostic_changed || scan_changed
            } else if let Some(diagnostics) = diagnostics {
                diagnostics.counts.stale += 1;
                true
            } else {
                false
            }
        };
        if changed {
            self.channel.changed.notify_one();
        }
    }

    fn finish(&self, id: ActivityId, state: State, coalesced: u64) {
        let changed = {
            let mut latest = self.channel.latest.lock().unwrap();
            latest.active.remove(&id).is_some_and(|mut active| {
                finish_record(
                    &mut latest.diagnostics,
                    &mut active.activity,
                    state,
                    coalesced,
                )
            })
        };
        if changed {
            self.channel.changed.notify_one();
        }
    }
}

struct Reported {
    state: State,
    details: Option<Details>,
    scan: Option<ScanProgress>,
}

/// An activity cannot stay active when its future or blocking scope is dropped.
pub(crate) struct ActivityGuard {
    publisher: ActivityPublisher,
    origin: Origin,
    id: Option<ActivityId>,
    reporter: ActivityReporter,
}

impl ActivityGuard {
    pub(crate) fn reference(&self) -> Option<ActivityReference> {
        let id = self.id?;
        Some(ActivityReference {
            module: ActivityModule::Detectors,
            producer: self.publisher.channel.session,
            session: self.publisher.generation.0,
            account: self.publisher.channel.scope.account.clone(),
            root: Some(self.publisher.channel.scope.root.clone()),
            id,
        })
    }

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
        if self.id.is_none() {
            return self.publisher.disabled(origin);
        }
        self.publisher.begin(origin, self.id, phase, state, details)
    }

    pub(crate) fn report(&self, details: impl FnOnce() -> Details) {
        self.publish(false, || Reported {
            state: State::Running,
            details: Some(details()),
            scan: None,
        });
    }

    pub(super) fn report_scan(
        &self,
        immediate: bool,
        details: impl FnOnce() -> (Details, ScanProgress),
    ) {
        self.publish(immediate, || {
            let (details, scan) = details();
            Reported {
                state: State::Running,
                details: Some(details),
                scan: Some(scan),
            }
        });
    }

    pub(crate) fn set_state(&self, state: State) {
        self.publish(true, || Reported {
            state,
            details: None,
            scan: None,
        });
    }

    pub(crate) fn transition(&self, state: State, details: impl FnOnce() -> Details) {
        self.publish(true, || Reported {
            state,
            details: Some(details()),
            scan: None,
        });
    }

    fn publish(&self, immediate: bool, update: impl FnOnce() -> Reported) {
        if let Some(id) = self.id {
            if let Some(sample) = self.reporter.sample(immediate, update) {
                self.publisher.update(id, sample);
            }
        }
    }

    pub(crate) fn finish(mut self, state: State) {
        if let Some(id) = self.id.take() {
            self.publisher
                .finish(id, state, self.reporter.take_coalesced());
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
                self.reporter.take_coalesced(),
            );
        }
    }
}
