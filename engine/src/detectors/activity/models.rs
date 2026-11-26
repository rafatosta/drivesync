//! Activity types shared at the Detectors boundary.

pub(crate) use crate::activity::{ActivityId, Scope};
use crate::models::Path;
use std::time::Instant;

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
    Collect,
    ValidateAccess,
    Checkpoint,
    ReadDirectory,
    ReadChanges,
    ValidateItems,
    VerifyKnown,
    ReadMetadata,
    VerifyContent,
    ResolveAncestry,
    Contextualize,
    ReadInventory,
    ResolveIdentities,
    ReuseContext,
    Audit,
    Deliver,
    Idle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Wait {
    Worker,
    Ownership,
    Authorization,
    ObserverState,
    Provider,
    Continuation,
    Context,
    Inventory,
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
    PagesRequested,
    ItemsCollected,
    DirectoriesPending,
    Changes,
    ItemsChecked,
    Ancestors,
    Records,
    Queries,
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
    InvalidContinuation,
    CaptureRequired,
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
