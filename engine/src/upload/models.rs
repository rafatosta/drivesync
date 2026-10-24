//! Scoped create-only intents. Sensitive session URIs deliberately have no Debug.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Intent {
    pub path: PathBuf,
    pub root: crate::download::Identity,
    pub source: Option<crate::download::Identity>,
    pub id: String,
    pub parent: String,
    pub directory: bool,
    pub size: u64,
    pub md5: String,
    pub sha: String,
    pub snapshot: String,
    pub session: Option<String>,
    pub confirmed: bool,
}
