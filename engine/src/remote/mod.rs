//! Read-only Google Drive folder metadata browsing.
//!
//! This module validates provider responses, folder IDs and pagination while
//! keeping authenticated HTTP transport and error parsing separate. It neither
//! modifies remote content or installs local user files.
//!
//! - [`client`] handles bounded HTTP requests.
//! - [`errors`] maps provider failures to sanitized [`crate::EngineError`].
//! - [`folders`] implements folder lookup and pagination.
//! - [`models`] groups decoded JSON and validated response models.
mod client;
mod errors;
mod folders;
mod models;

pub(crate) use folders::{get, list};
pub use models::FolderPage;

#[cfg(test)]
mod tests;
