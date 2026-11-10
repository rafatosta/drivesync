//! Direct Google Drive v3, desktop OAuth and OpenID Connect mapping.
//!
//! All 78 documented operations are implemented independently of application
//! synchronization. Network operations use bounded async HTTP on Tokio; pure
//! parameter transformations and authorization URL construction stay synchronous.
//! No credential persistence, retries, task scheduler or synchronization policy.

mod api;
mod client;
mod collaboration;
mod downloads;
mod errors;
mod models;
mod mutations;
mod oauth;
mod oauth_models;
mod reads;
mod resources;
mod uploads;

pub use api::*;
pub use client::{GoogleClientLimits, GoogleDriveClient};
pub use downloads::{GoogleByteRange, GoogleByteStream, GoogleDownloadStream};
pub use errors::{GoogleApiError, GoogleApiFailure, GoogleResult};
pub use models::*;
pub use oauth_models::*;
pub use resources::*;
pub use uploads::{GoogleUploadChunk, GoogleUploadProgress, GoogleUploadSession};

#[cfg(test)]
mod tests;
