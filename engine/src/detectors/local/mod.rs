//! Local observation data contract.
//!
//! This module defines the public shape of local observations. It performs no
//! filesystem access and contains no detection implementation.

mod models;

pub use crate::models::{ObservationEvent, ObservationKind, ObservationOrigin};
pub use models::{
    FileKind, FileState, Id, LocalCapabilities, LocalIdentity, LocalObservation, Path,
};
