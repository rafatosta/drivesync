//! Optional create-only uploads with immutable snapshots and durable provider IDs.
//! Existing remote data is never updated or deleted. See `create-only.md`.
pub(crate) mod execution;
mod local;
mod models;
pub(crate) mod provider;
mod transport;
pub(crate) use models::{Intent, Retry};
pub(crate) use transport::valid_session;

#[cfg(test)]
mod tests;
