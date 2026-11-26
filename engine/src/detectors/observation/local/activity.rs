//! Translation from Scanner diagnostics to the local observation lifecycle.

use super::super::{
    scanner::{
        activity::{Event, Rebuild, Reporter, Traversal},
        ScanError,
    },
    ScanProgress,
};
use crate::{
    detectors::activity::{ActivityGuard, Details, Phase, Progress, Reason, State, Unit},
    models::Path,
};
use std::cell::RefCell;

pub(super) fn scanner(parent: &ActivityGuard) -> Reporter {
    let traversal = RefCell::new(Some(parent.child(
        Phase::Traverse,
        State::Running,
        counts(Traversal::default()),
    )));
    let checksum: RefCell<Option<ChecksumActivity>> = RefCell::new(None);
    Box::new(move |event| match event {
        Event::Position(path, metrics) => {
            if let Some(scan) = traversal.borrow().as_ref() {
                scan.report_scan(false, || {
                    let mut details = counts(metrics);
                    details.item = Some(path.to_path_buf());
                    let display = ScanProgress {
                        path: details.item.clone(),
                        items: metrics.selected,
                        pages: 0,
                    };
                    (details, display)
                });
            }
        }
        Event::Rebuild(reason) => {
            if let Some(scan) = traversal.borrow().as_ref() {
                scan.child(
                    Phase::Recover,
                    State::Running,
                    Details {
                        reason: Some(match reason {
                            Rebuild::Coverage => Reason::CoverageLost,
                            Rebuild::Namespace => Reason::NamespaceChanged,
                            Rebuild::NoCurrentWatch => Reason::NoCurrentWatch,
                        }),
                        ..Details::default()
                    },
                )
                .finish(State::Complete);
            }
        }
        Event::Final(metrics, result) => {
            if let Some(scan) = traversal.borrow_mut().take() {
                scan.transition(State::Running, || counts(metrics));
                scan.finish(match result {
                    Ok(super::ObservationCoverage::Complete) => State::Complete,
                    Err(ScanError::Cancelled) => State::Cancelled,
                    _ => State::Failed,
                });
            }
        }
        Event::ChecksumStart { path, total } => {
            if let Some(scan) = traversal.borrow().as_ref() {
                let path = path.to_path_buf();
                let guard = scan.child(Phase::Checksum, State::Running, bytes(&path, 0, total));
                checksum.replace(Some(ChecksumActivity {
                    guard,
                    path,
                    total,
                    processed: 0,
                }));
            }
        }
        Event::ChecksumBytes(processed) => {
            if let Some(checksum) = checksum.borrow_mut().as_mut() {
                checksum.processed = processed;
                checksum
                    .guard
                    .report(|| bytes(&checksum.path, processed, checksum.total));
            }
        }
        Event::ChecksumEnd(result) => {
            if let Some(checksum) = checksum.borrow_mut().take() {
                checksum.guard.transition(State::Running, || {
                    bytes(&checksum.path, checksum.processed, checksum.total)
                });
                checksum.guard.finish(match result {
                    Ok(()) => State::Complete,
                    Err(ScanError::Cancelled) => State::Cancelled,
                    Err(_) => State::Failed,
                });
            }
        }
    })
}

struct ChecksumActivity {
    guard: ActivityGuard,
    path: Path,
    total: u64,
    processed: u64,
}

fn bytes(path: &std::path::Path, completed: u64, total: u64) -> Details {
    Details {
        item: Some(path.to_path_buf()),
        ..Details::count(Progress {
            unit: Unit::Bytes,
            completed,
            total: Some(total),
        })
    }
}

fn counts(metrics: Traversal) -> Details {
    let count = |unit, completed| {
        Some(Progress {
            unit,
            completed,
            total: None,
        })
    };
    Details {
        progress: [
            count(Unit::EntriesVisited, metrics.visited),
            count(Unit::EntriesCompleted, metrics.completed),
            count(Unit::Directories, metrics.directories),
            count(Unit::SnapshotItems, metrics.snapshot),
            count(Unit::ChecksumsReused, metrics.hashes_reused),
        ],
        ..Details::default()
    }
}
