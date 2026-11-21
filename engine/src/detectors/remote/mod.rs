//! Remote observation data contract.
//!
//! This module defines the public shape of observations obtained from a
//! provider. It performs no network access and contains no detection
//! implementation.

mod models;

pub use models::{FileState, ObservationEvent, RemoteCapabilities, RemoteId, RemoteObservation};
