//! Normalized observations shared by local and remote producers.

pub(crate) mod models;
pub(crate) mod remote;

pub(crate) use models::{
    LocalObservation, LocalObservationSource, Observation, ObservationCoverage, RemoteObservation,
};

pub(crate) mod local;
pub(crate) mod scanner;

pub(super) mod progress;
pub(crate) use progress::{ProgressReporter, ScanProgress};
