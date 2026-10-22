//! Read-only access to Google Drive folder metadata for initial setup.
//!
//! This module validates provider responses, folder IDs and pagination while
//! keeping authenticated HTTP transport and error parsing separate. It neither
//! downloads/uploads files nor modifies remote or local user content.
//!
//! - [`client`] handles bounded HTTP requests.
//! - [`errors`] maps provider failures to sanitized [`crate::EngineError`].
//! - [`folders`] implements folder lookup and pagination.
//! - [`models`] groups decoded JSON and validated response models.
mod client;
mod errors;
mod folders;
mod models;

pub use models::FolderPage;
pub(crate) use folders::{get, list};

#[cfg(test)]
mod tests;
