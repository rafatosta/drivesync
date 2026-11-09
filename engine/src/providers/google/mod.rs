//! Google Drive REST v3 mapping (technical API surface).
//!
//! **TDD status:** only the types, error model and the `files_get` entry point
//! exist, and the behaviour is intentionally *not implemented* (`unimplemented!`).
//! The test suites under `tests/` define the expected behaviour (RED phase).
//! See `docs/engine/providers/google/` for the contract.

mod client;
mod errors;
mod models;

pub use client::GoogleDriveClient;
pub use errors::{GoogleApiError, GoogleApiFailure, GoogleResult};
pub use models::*;

#[cfg(test)]
mod tests;
