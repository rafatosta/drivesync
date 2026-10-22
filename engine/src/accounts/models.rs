//! Non-secret data structures serialized to the account index.
//!
//! These types represent saved settings, not live credentials or sessions.
use crate::{AccountSummary, FolderConfiguration};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Root of the `accounts.json` index, scoped by OAuth client.
///
/// Unknown top-level fields are rejected. Additional integrity checks are
/// performed by [`super::validation::validate`].
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Index {
    /// Storage format: only version 2 is currently accepted.
    pub(super) version: u32,
    /// Saved identities keyed by OAuth client identifier.
    pub(super) clients: BTreeMap<String, Vec<AccountSummary>>,
    /// Folder pairs keyed by client; at most one pair per account.
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
