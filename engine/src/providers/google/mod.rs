//! Google Drive metadata mapping, independent of application synchronization.
//!
//! `files_get` uses bounded async HTTP and typed, sanitized responses. Other
//! Google operations remain documented contracts for future groups.

mod client;
mod errors;
mod models;
mod resources;
pub use resources::*;

pub use client::{GoogleClientLimits, GoogleDriveClient};
pub use errors::{GoogleApiError, GoogleApiFailure, GoogleResult};
pub use models::*;

#[cfg(test)]
mod tests;

mod reads;

mod mutations;

mod collaboration;

mod oauth_models;
pub use oauth_models::*;
mod oauth;

mod downloads;
pub use downloads::{GoogleByteRange, GoogleByteStream, GoogleDownloadStream};

mod uploads;
pub use uploads::{GoogleUploadChunk, GoogleUploadProgress, GoogleUploadSession};

mod api;
pub use api::*;
