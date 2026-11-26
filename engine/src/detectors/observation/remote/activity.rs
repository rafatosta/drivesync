//! Provider-neutral activity contract for scoped remote observation.

use super::super::ScanProgress;
use crate::{
    detectors::activity::{ActivityGuard, Details, Phase, Progress, Reason, State, Unit, Wait},
    models::Path,
};
use std::future::Future;

/// Diagnostics are optional and separate from the functional request and result.
#[derive(Clone, Copy, Default)]
pub(crate) struct RemoteActivity<'a> {
    parent: Option<&'a ActivityGuard>,
}

/// Collection progress and the independently maintained compatibility page count.
pub(super) struct Position {
    pub(super) path: Option<Path>,
    pub(super) displayed_pages: u64,
    pub(super) counts: Collection,
}

pub(super) enum Collection {
    Capture {
        requested: u64,
        received: u64,
        collected: u64,
        directories_completed: u64,
        directories_pending: u64,
    },
    Changes {
        requested: u64,
        received: u64,
        processed: u64,
        known: u64,
    },
}

#[derive(Clone, Copy)]
pub(super) enum Step {
    ObserverState,
    ValidateAccess,
    Checkpoint,
    List,
    Changes,
    ValidateItems,
    Normalize,
    VerifyKnown,
    Metadata,
    VerifyContent,
    Ancestry,
    Recover(Recovery),
}

#[derive(Clone, Copy)]
pub(super) enum Recovery {
    InvalidContinuation,
    CaptureRequired,
    Budget,
    Coverage,
}

impl<'a> RemoteActivity<'a> {
    pub(crate) fn new(parent: &'a ActivityGuard) -> Self {
        Self {
            parent: Some(parent),
        }
    }

    pub(super) fn position(self, immediate: bool, position: impl FnOnce() -> Position) {
        if let Some(parent) = self.parent {
            parent.report_scan(immediate, || {
                let Position {
                    path,
                    displayed_pages,
                    counts,
                } = position();
                let count = |unit, completed| {
                    Some(Progress {
                        unit,
                        completed,
                        total: None,
                    })
                };
                let (items, progress) = match counts {
                    Collection::Capture {
                        requested,
                        received,
                        collected,
                        directories_completed,
                        directories_pending,
                    } => (
                        collected,
                        [
                            count(Unit::PagesRequested, requested),
                            count(Unit::PagesReceived, received),
                            count(Unit::ItemsCollected, collected),
                            count(Unit::Directories, directories_completed),
                            count(Unit::DirectoriesPending, directories_pending),
                        ],
                    ),
                    Collection::Changes {
                        requested,
                        received,
                        processed,
                        known,
                    } => (
                        known,
                        [
                            count(Unit::PagesRequested, requested),
                            count(Unit::PagesReceived, received),
                            count(Unit::Changes, processed),
                            count(Unit::SnapshotItems, known),
                            None,
                        ],
                    ),
                };
                let details = Details {
                    item: path.clone(),
                    progress,
                    ..Details::default()
                };
                (
                    details,
                    ScanProgress {
                        path,
                        items,
                        pages: displayed_pages,
                    },
                )
            });
        }
    }

    pub(super) fn verifying(self, step: Step, total: Option<usize>) -> RemoteWork {
        self.begin(step, State::Running, total)
    }

    pub(super) fn state_lock(self) -> RemoteWork {
        self.begin(
            Step::ObserverState,
            State::Waiting(Wait::ObserverState),
            None,
        )
    }

    pub(super) async fn response<T, E>(
        self,
        step: Step,
        response: impl Future<Output = Result<T, E>>,
    ) -> Result<T, E> {
        let waiting = self.begin(step, State::Waiting(Wait::Provider), None);
        let result = response.await;
        waiting.finish(result.is_ok());
        result
    }

    fn begin(self, step: Step, state: State, total: Option<usize>) -> RemoteWork {
        let (phase, unit, reason) = match step {
            Step::ObserverState => (Phase::Observe, None, None),
            Step::ValidateAccess => (Phase::ValidateAccess, None, None),
            Step::Checkpoint => (Phase::Checkpoint, None, None),
            Step::List => (Phase::ReadDirectory, None, None),
            Step::Changes => (Phase::ReadChanges, Some(Unit::Changes), None),
            Step::ValidateItems => (Phase::ValidateItems, Some(Unit::EntriesCompleted), None),
            Step::Normalize => (Phase::BuildObservations, Some(Unit::Observations), None),
            Step::VerifyKnown => (Phase::VerifyKnown, Some(Unit::ItemsChecked), None),
            Step::Metadata => (Phase::ReadMetadata, None, None),
            Step::VerifyContent => (Phase::VerifyContent, None, None),
            Step::Ancestry => (Phase::ResolveAncestry, Some(Unit::Ancestors), None),
            Step::Recover(recovery) => (
                Phase::Recover,
                None,
                Some(match recovery {
                    Recovery::InvalidContinuation => Reason::InvalidContinuation,
                    Recovery::CaptureRequired => Reason::CaptureRequired,
                    Recovery::Budget => Reason::BudgetReached,
                    Recovery::Coverage => Reason::IncompleteObservation,
                }),
            ),
        };
        let guard = self.parent.map(|parent| {
            parent.child(
                phase,
                state,
                Details {
                    reason,
                    progress: [
                        unit.map(|unit| Progress::new(unit, 0, total)),
                        None,
                        None,
                        None,
                        None,
                    ],
                    ..Details::default()
                },
            )
        });
        RemoteWork { guard, unit, total }
    }
}

pub(super) struct RemoteWork {
    guard: Option<ActivityGuard>,
    unit: Option<Unit>,
    total: Option<usize>,
}

impl RemoteWork {
    pub(super) fn advance(&self, completed: usize) {
        if let (Some(guard), Some(unit)) = (&self.guard, self.unit) {
            guard.report(|| Details::count(Progress::new(unit, completed, self.total)));
        }
    }

    pub(super) fn complete(self, completed: usize) {
        if let (Some(guard), Some(unit)) = (&self.guard, self.unit) {
            guard.transition(State::Running, || {
                Details::count(Progress::new(unit, completed, self.total))
            });
        }
        self.finish(true);
    }

    pub(super) fn finish(self, succeeded: bool) {
        if let Some(guard) = self.guard {
            guard.finish(if succeeded {
                State::Complete
            } else {
                State::Failed
            });
        }
    }
}
