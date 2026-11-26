//! Scanner positions and counters consumed by its LocalObserver.

use super::{ObservationCoverage, ScanError};
use std::path::Path;

#[derive(Clone, Copy, Default)]
pub(in crate::detectors::observation) struct Traversal {
    pub visited: u64,
    pub selected: u64,
    pub completed: u64,
    pub directories: u64,
    pub snapshot: u64,
    pub hashes_reused: u64,
}

#[derive(Clone, Copy)]
pub(in crate::detectors::observation) enum Rebuild {
    Coverage,
    Namespace,
    NoCurrentWatch,
}

pub(in crate::detectors::observation) enum Event<'a> {
    Position(&'a Path, Traversal),
    Rebuild(Rebuild),
    Final(Traversal, Result<ObservationCoverage, ScanError>),
    ChecksumStart { path: &'a Path, total: u64 },
    ChecksumBytes(u64),
    ChecksumEnd(Result<(), ScanError>),
}

pub(in crate::detectors::observation) type Reporter = Box<dyn for<'a> Fn(Event<'a>) + Send>;
