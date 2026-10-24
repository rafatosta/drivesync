//! Durable observations are distinct from confirmed download-operation evidence.
use crate::RemoteInventory;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Notice {
    pub id: String,
    pub path: Option<PathBuf>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Checkpoint {
    pub cursor: String,
    pub inventory: RemoteInventory,
    /// Unavailable previous paths remain visible until acknowledged or the same ID/path returns.
    pub notices: Vec<Notice>,
}
