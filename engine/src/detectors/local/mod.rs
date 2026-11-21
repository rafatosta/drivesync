//! Local observation data contract.
//!
//! This module defines the public shape of local observations. It performs no
//! filesystem access and contains no detection implementation.

mod models;

pub use models::{
    Checksum, FileKind, FileState, Id, LocalCapabilities, LocalIdentity, LocalObservation,
    ObservationEvent, Path,
};
