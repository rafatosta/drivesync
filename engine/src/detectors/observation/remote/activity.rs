//! Provider-neutral activity contract for a scoped remote observation round.

use super::super::ScanProgress;
use crate::{
    detectors::activity::{ActivityGuard, Details, Progress, Unit},
    models::Path,
};

/// Diagnostics are optional and separate from the functional request and result.
#[derive(Clone, Copy, Default)]
pub(crate) struct RemoteActivity<'a> {
    parent: Option<&'a ActivityGuard>,
}

/// A display position resolved within the root, with the existing snapshot and page counts.
pub(super) struct Position {
    pub(super) path: Option<Path>,
    pub(super) items: u64,
    pub(super) pages: u64,
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
                let Position { path, items, pages } = position();
                let count = |unit, completed| {
                    Some(Progress {
                        unit,
                        completed,
                        total: None,
                    })
                };
                let details = Details {
                    item: path.clone(),
                    progress: [
                        count(Unit::SnapshotItems, items),
                        count(Unit::PagesReceived, pages),
                        None,
                        None,
                        None,
                    ],
                    ..Details::default()
                };
                (details, ScanProgress { path, items, pages })
            });
        }
    }
}
