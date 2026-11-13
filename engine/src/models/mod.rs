//! Shared identities and pure observation models. This module performs no I/O.
//!
//! Local metadata records evidence, never authorization to synchronize or delete.
//! File envelopes and remote observation models remain a separate implementation.
use serde::{Deserialize, Serialize};
/// Stable application account identity, independent of presentation fields.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AccountId(pub String);
/// External service identifier; not an account identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProviderId(pub String);
/// UTC instant with millisecond precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timestamp {
    pub unix_millis: i64,
}

/// Identity of a configured synchronization root, not a filesystem path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SyncId(pub String);

/// Identity of one observation; retries preserve it and new observations do not.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FileId(pub String);

/// Optional logical local identity, never inferred from an inode or path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LocalFileId(pub String);

/// Evidence state for one attribute, keeping absence distinct from known zero.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetadataValue<T> {
    /// A value actually observed, including zero or an empty collection.
    Known(T),
    /// No collection was attempted.
    NotCollected,
    /// The attribute does not apply to this entry.
    NotApplicable,
    /// The collection backend or filesystem does not support the attribute.
    Unsupported,
    /// The attempted query was denied.
    PermissionDenied,
    /// The attempted observation did not yield a reliable value.
    Unavailable,
}

/// A pure validation failure, containing no paths, metadata or OS diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelError {
    /// A supplied logical identity is empty or whitespace-only.
    EmptyIdentity,
    /// Unix path bytes violate the relative-entry path contract.
    InvalidRelativePath,
    /// A timestamp fraction is outside the nanosecond range.
    InvalidTimestamp,
    /// Related metadata fields contradict each other.
    InconsistentMetadata,
    /// Reserved for the separately implemented observation envelope validator.
    InconsistentEvent,
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::EmptyIdentity => "empty identity",
            Self::InvalidRelativePath => "invalid relative path",
            Self::InvalidTimestamp => "invalid timestamp",
            Self::InconsistentMetadata => "inconsistent metadata",
            Self::InconsistentEvent => "inconsistent event",
        })
    }
}

impl std::error::Error for ModelError {}

/// Result of a pure model construction or validation operation.
pub type ModelResult<T> = Result<T, ModelError>;

mod local;
pub use local::*;
mod events;
pub use events::*;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod event_tests;
