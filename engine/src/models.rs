//! Shared identity types. This module performs no I/O.
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
