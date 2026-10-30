//! Durable observations are distinct from confirmed download-operation evidence.
use crate::RemoteInventory;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Notice {
    pub id: String,
    /// Explicit trash evidence for this identity, never inferred from a missing listing.
    pub deleted: bool,
    pub path: Option<PathBuf>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Checkpoint {
    pub cursor: String,
    pub inventory: RemoteInventory,
    /// Previous paths are informational activity until the same ID/path returns.
    pub notices: Vec<Notice>,
}
