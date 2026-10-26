//! Initial association of Google Drive folders with local directories.
//!
//! Separates configuration records from local filesystem and Flatpak portal
//! path handling. The module validates paths and folder metadata but does not
//! transfer files, start synchronization or perform Google authentication.
//!
//! - [`models`]: persisted remote/local configuration data and remote IDs.
//! - [`identity`]: the portable marker retained in each selected local root.
//! - [`paths`]: local access checks, host-origin resolution and overlap tests.
pub(crate) mod identity;
mod models;
mod paths;

pub(crate) use models::valid_id;
pub use models::{DriveFolder, FolderConfiguration};
pub(crate) use paths::{access_path, local_folder, local_origin, overlap};

#[cfg(test)]
mod tests;
