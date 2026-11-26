//! Direct Google Drive v3, desktop OAuth and OpenID Connect mapping.
//!
//! The documented public calls are preserved. Permanent deletion endpoints are
//! disabled locally by explicit policy; callers must use trash instead. Network operations use bounded async HTTP on Tokio; pure
//! parameter transformations and authorization URL construction stay synchronous.
//! No credential persistence, retries, task scheduler or synchronization policy.

mod api;
mod client;
mod collaboration;
mod conditional;
pub use conditional::GoogleFileSnapshot;
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
pub(crate) use downloads::ContentVerification;
pub use downloads::{GoogleByteRange, GoogleByteStream, GoogleDownloadStream};
pub use errors::{GoogleApiError, GoogleApiFailure, GoogleResult};
pub use models::*;
pub use oauth_models::*;
pub use resources::*;
pub use uploads::{GoogleUploadChunk, GoogleUploadProgress, GoogleUploadSession};

#[cfg(test)]
mod tests;
