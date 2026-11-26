//! Redacted, bounded diagnostic records. Formatting runs outside producer locks.

use super::{Activity, ActivityId, Origin, Phase, Progress, Reason, Scope, State};
use std::{
    collections::{hash_map::RandomState, VecDeque},
    hash::BuildHasher,
    io::{self, Write},
    sync::OnceLock,
    time::Duration,
};

pub(super) const CAPACITY: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Event {
    Started,
    Progress,
    Transition,
    Finished,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Item {
    Root,
    Relative(u64),
}

#[derive(Clone, Copy)]
struct Record {
    event: Event,
    id: ActivityId,
    parent: Option<ActivityId>,
    origin: Origin,
    phase: Phase,
    state: State,
    item: Option<Item>,
    progress: [Option<Progress>; 5],
    reason: Option<Reason>,
    sequence: u64,
    elapsed: Duration,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Counts {
    pub(super) coalesced: u64,
    pub(super) stale: u64,
    pub(super) overwritten: u64,
    pub(super) capacity: u64,
}

impl Counts {
    fn is_empty(self) -> bool {
        self == Self::default()
    }
}

pub(super) struct Diagnostics {
    labels: (u64, u64),
    records: VecDeque<Record>,
    pub(super) counts: Counts,
}

impl Diagnostics {
    pub(super) fn new(scope: &Scope) -> Self {
        let hash = hasher();
        Self {
            labels: (
                hash.hash_one(&scope.account.0),
                hash.hash_one(&scope.root.0),
            ),
            records: VecDeque::with_capacity(CAPACITY),
            counts: Counts::default(),
        }
    }

    pub(super) fn push(&mut self, event: Event, activity: &Activity) {
        let record = Record {
            event,
            id: activity.id,
            parent: activity.parent,
            origin: activity.origin,
            phase: activity.phase,
            state: activity.state,
            item: activity.details.item.as_ref().map(|path| {
                if path.as_os_str().is_empty() {
                    Item::Root
                } else {
                    Item::Relative(hasher().hash_one(path))
                }
            }),
            progress: activity.details.progress,
            reason: activity.details.reason,
            sequence: activity.sequence,
            elapsed: activity.updated.saturating_duration_since(activity.started),
        };
        if let Some(last) = self.records.back_mut() {
            if event == Event::Progress && last.event == Event::Progress && last.id == record.id {
                *last = record;
                self.counts.coalesced += 1;
                return;
            }
        }
        if self.records.len() == CAPACITY {
            self.records.pop_front();
            self.counts.overwritten += 1;
        }
        self.records.push_back(record);
    }

    pub(super) fn pending(&self) -> bool {
        !self.records.is_empty() || !self.counts.is_empty()
    }

    pub(super) fn take(&mut self) -> Option<Batch> {
        self.pending().then(|| Batch {
            labels: self.labels,
            records: self.records.drain(..).collect(),
            counts: std::mem::take(&mut self.counts),
        })
    }
}

pub(super) struct Batch {
    labels: (u64, u64),
    records: Vec<Record>,
    counts: Counts,
}

impl Batch {
    pub(super) fn write(&self, mut output: impl Write) -> io::Result<()> {
        let (account, root) = self.labels;
        for record in &self.records {
            write!(
                output,
                "drivesync-detectors account={account:016x} root={root:016x} origin={:?} activity={} parent={:?} phase={:?} event={:?} state={:?} sequence={} elapsed_ms={} item={:?}",
                record.origin, record.id.0, record.parent, record.phase, record.event, record.state,
                record.sequence, record.elapsed.as_millis(), record.item,
            )?;
            if let Some(reason) = record.reason {
                write!(output, " reason={reason:?}")?;
            }
            for metric in record.progress.iter().flatten() {
                write!(output, " {:?}={}", metric.unit, metric.completed)?;
                if let Some(total) = metric.total {
                    write!(output, "/{total}")?;
                }
            }
            writeln!(output)?;
        }
        if !self.counts.is_empty() {
            writeln!(
                output,
                "drivesync-detectors account={account:016x} root={root:016x} coalesced={} stale={} overwritten={} capacity={}",
                self.counts.coalesced, self.counts.stale, self.counts.overwritten, self.counts.capacity
            )?;
        }
        Ok(())
    }
}

fn hasher() -> &'static RandomState {
    static HASHER: OnceLock<RandomState> = OnceLock::new();
    HASHER.get_or_init(RandomState::new)
}
