//! Read-only Google Drive folder browsing and initial remote inventory.
//!
//! This module validates provider responses, folder IDs and pagination while
//! keeping authenticated HTTP transport and error parsing separate. It neither
//! downloads/uploads files nor modifies remote or local user content.
//!
//! - [`client`] handles bounded HTTP requests.
//! - [`errors`] maps provider failures to sanitized [`crate::EngineError`].
//! - [`folders`] implements folder lookup and pagination.
//! - [`models`] groups decoded JSON and validated response models.
//! - [`inventory`] traverses configured roots before any download planning.
mod client;
mod errors;
mod folders;
mod inventory;
mod models;

pub(crate) use folders::{get, list};
pub(crate) use inventory::{
    page as inventory_page, root as inventory_root, scan as inventory_scan,
};
pub use inventory::{InventoryEntry, InventoryKind, RemoteInventory, SkipReason};
pub use models::FolderPage;

#[cfg(test)]
mod tests;
