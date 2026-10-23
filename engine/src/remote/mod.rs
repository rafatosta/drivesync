//! Read-only Google Drive metadata browsing and binary media transport.
//!
//! This module validates provider responses, folder IDs and pagination while
//! keeping authenticated HTTP transport and error parsing separate. It neither
//! modifies remote content or installs local user files.
//!
//! - [`client`] handles bounded HTTP requests.
//! - [`errors`] maps provider failures to sanitized [`crate::EngineError`].
//! - [`folders`] implements folder lookup and pagination.
//! - [`models`] groups decoded JSON and validated response models.
//! - [`inventory`] traverses configured roots before any download planning.
mod changes;
mod client;
pub(crate) use changes::valid_token as valid_change_token;
pub(crate) use changes::{change_page, change_start, Change, ChangePage};
mod errors;
mod folders;
mod inventory;
mod media;
mod models;
pub(crate) use media::{media as file_media, metadata as file_metadata};

pub(crate) use folders::{get, list};
pub(crate) use inventory::{
    page as inventory_page, root as inventory_root, scan as inventory_scan,
};
pub use inventory::{InventoryEntry, InventoryKind, RemoteInventory, SkipReason};
pub use models::FolderPage;

#[cfg(test)]
mod tests;
