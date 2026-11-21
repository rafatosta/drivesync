//! Remote observation data contract.
//!
//! This module defines the public shape of observations obtained from a
//! provider. It performs no network access and contains no detection
//! implementation.

mod models;

pub use crate::models::{ObservationEvent, ObservationKind, ObservationOrigin};
pub use models::{FileState, RemoteCapabilities, RemoteId, RemoteObservation};
