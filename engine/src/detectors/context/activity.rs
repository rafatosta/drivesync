//! Context phases and units for work on observations and persisted evidence.

use crate::detectors::activity::{ActivityGuard, Details, Phase, Progress, State, Unit, Wait};
use std::cell::Cell;

#[derive(Clone, Copy, Default)]
pub(super) struct Trace<'a> {
    parent: Option<&'a ActivityGuard>,
    lookups: Option<&'a Cell<usize>>,
}

#[derive(Clone, Copy)]
pub(super) enum Work {
    Validate,
    InventoryRecords,
    Compare,
    Index,
    Build,
    Reuse,
    Audit,
}

#[derive(Clone, Copy)]
pub(super) enum Resource {
    Context,
    Inventory,
}

impl<'a> Trace<'a> {
    pub(super) fn new(parent: &'a ActivityGuard) -> Self {
        Self {
            parent: Some(parent),
            lookups: None,
        }
    }

    pub(super) fn count_lookups(self, lookups: &'a Cell<usize>) -> Self {
        Self {
            lookups: Some(lookups),
            ..self
        }
    }

    pub(super) fn lookup<T>(&self, query: impl FnOnce() -> T) -> T {
        if let Some(lookups) = self.lookups {
            let value = query();
            lookups.set(lookups.get() + 1);
            return value;
        }
        self.wait(Resource::Inventory, || {
            Ok::<_, std::convert::Infallible>(query())
        })
        .unwrap()
    }

    pub(super) fn cache<T, E>(self, access: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
        if self.lookups.is_some() {
            return access();
        }
        self.wait(Resource::Context, access)
    }

    pub(super) fn wait<T, E>(
        self,
        resource: Resource,
        access: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        let guard = self.parent.map(|parent| {
            let (phase, wait) = match resource {
                Resource::Context => (Phase::Contextualize, Wait::Context),
                Resource::Inventory => (Phase::ReadInventory, Wait::Inventory),
            };
            parent.child(phase, State::Waiting(wait), Details::default())
        });
        let result = access();
        if let Some(guard) = guard {
            guard.finish(if result.is_ok() {
                State::Complete
            } else {
                State::Failed
            });
        }
        result
    }

    pub(super) fn batch(self, work: Work, total: usize) -> Batch<'a> {
        let (phase, unit) = match work {
            Work::Validate => (Phase::ValidateItems, Unit::Observations),
            Work::InventoryRecords => (Phase::ReadInventory, Unit::Records),
            Work::Compare => (Phase::CompareSnapshots, Unit::ItemsCompared),
            Work::Index => (Phase::ResolveIdentities, Unit::ItemsChecked),
            Work::Build => (Phase::Contextualize, Unit::Observations),
            Work::Reuse => (Phase::ReuseContext, Unit::Files),
            Work::Audit => (Phase::Audit, Unit::Records),
        };
        let guard = self.parent.map(|parent| {
            parent.child(
                phase,
                State::Running,
                Details::count(Progress::new(unit, 0, Some(total))),
            )
        });
        Batch {
            guard,
            unit,
            total,
            processed: 0,
            lookups: self.lookups,
        }
    }
}

pub(super) struct Batch<'a> {
    guard: Option<ActivityGuard>,
    unit: Unit,
    total: usize,
    processed: usize,
    lookups: Option<&'a Cell<usize>>,
}

impl Batch<'_> {
    pub(super) fn advance(&mut self) {
        if let Some(guard) = &self.guard {
            self.processed += 1;
            guard.report(|| self.details());
        }
    }

    pub(super) fn finish(mut self) {
        if let Some(guard) = self.guard.take() {
            guard.transition(State::Running, || self.details());
            guard.finish(State::Complete);
        }
    }

    fn details(&self) -> Details {
        let mut details =
            Details::count(Progress::new(self.unit, self.processed, Some(self.total)));
        if let Some(lookups) = self.lookups {
            details.progress[1] = Some(Progress::new(Unit::Queries, lookups.get(), None));
        }
        details
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detectors::activity::{ActivityChannel, Origin, Scope};

    #[test]
    fn a_context_batch_exposes_intermediate_progress_while_it_is_active() {
        let channel = ActivityChannel::with_diagnostics(
            Scope {
                account: crate::models::AccountId("account".into()),
                root: crate::models::SyncRootId("root".into()),
            },
            false,
        );
        let root = channel.shared().begin(
            Origin::Shared,
            None,
            Phase::Contextualize,
            State::Running,
            Details::default(),
        );
        let queries = Cell::new(0);
        let trace = Trace::new(&root).count_lookups(&queries);
        let mut batch = trace.batch(Work::Build, 1024);
        trace.lookup(|| ());
        batch.advance();
        let active = channel.inbox().current();
        let details = &active.last().unwrap().details;
        assert_eq!(
            details.progress[0],
            Some(Progress::new(Unit::Observations, 1, Some(1024)))
        );
        assert_eq!(
            details.progress[1],
            Some(Progress::new(Unit::Queries, 1, None))
        );
        batch.finish();
        assert_eq!(channel.inbox().current().len(), 1);
    }
}
