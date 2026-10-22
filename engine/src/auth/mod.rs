//! Desktop OAuth authorization, credential persistence and account verification.
//!
//! The engine exposes only sanitized account identities and errors to the UI.
//! OAuth tokens stay inside the private provider and desktop credential vault.
//! A browser callback alone does not constitute successful authorization;
//! token exchange, identity checks and secure persistence must also succeed.
//!
//! Components:
//! - [`provider`]: OAuth lifecycle, renewal and per-account synchronization.
//! - [`models`]: sensitive credential and identity data structures.
//! - [`ports`]: testable Google API and vault contracts.
//! - [`google`] and [`storage`]: production adapters.
//! - [`callback`]: cancelable and validated loopback redirect listener.
//! - [`error`]: sanitized, typed authentication failures.
mod callback;
mod error;
mod google;
mod models;
mod ports;
mod provider;
mod storage;

pub use callback::Cancellation;
pub use error::AuthError;
pub use google::OAuthConfig;
pub(crate) use provider::AuthProvider;

#[cfg(test)]
mod tests;
