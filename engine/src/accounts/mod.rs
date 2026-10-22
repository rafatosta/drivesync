//! Persistent account profiles and folder configuration.
//!
//! Stores non-secret account identities and selected remote/local folder pairs.
//! OAuth credentials are managed separately by the authentication module.
//!
//! - [`models`] defines the current on-disk index.
//! - [`repository`] handles validated, serialized filesystem operations.
//! - [`validation`] checks index integrity and account-folder references.
//!
//! Other engine components access [`Accounts`] through [`crate::DriveSyncEngine`].
//! Persisted identities are not proof of an authenticated Google session.
//! Only JSON schema version 2 is supported; no migration is performed.
//! Repository operations never remove user files from selected local folders.
mod models;
mod repository;
mod validation;

pub(crate) use repository::Accounts;

#[cfg(test)]
mod tests;
