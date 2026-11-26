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
    Deliver,
    Idle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Wait {
    Worker,
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

/// Latest known details. Paths are root-relative display positions, with no provider identifiers.
#[derive(Debug, Clone, Default)]
pub(crate) struct Details {
    pub(crate) item: Option<Path>,
    pub(crate) progress: [Option<Progress>; 4],
}

impl Details {
    pub(crate) fn count(progress: Progress) -> Self {
        Self {
            progress: [Some(progress), None, None, None],
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
