//! Persisted account and folder index for this module.
use crate::{AccountSummary, FolderConfiguration};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Index {
    pub(super) version: u32,
    pub(super) clients: BTreeMap<String, Vec<AccountSummary>>,
    #[serde(default)]
    pub(super) folders: BTreeMap<String, Vec<FolderConfiguration>>,
}
impl Default for Index {
    fn default() -> Self {
        Self {
            version: 2,
            clients: BTreeMap::new(),
            folders: BTreeMap::new(),
        }
    }
}
