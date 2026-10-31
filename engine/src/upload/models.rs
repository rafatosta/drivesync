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
    pub retry: Retry,
}

/// Durable pass budget, reserved before remote requests so crashes consume it.
/// A pass may send many verified chunks; success never requires one retry per chunk.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Retry {
    pub attempts: u8,
    pub not_before: u64,
}
impl Retry {
    pub const LIMIT: u8 = 5;

    /// Reserves a pass or a read-only reconciliation probe. No sleeping or network.
    /// Deadlines use Unix seconds; clock rollback postpones rather than bypasses them.
    pub fn reserve(&mut self, now: u64, jitter: u64) -> Result<bool, crate::EngineError> {
        if now < self.not_before {
            return Err(crate::EngineError::UploadDeferred);
        }
        let writable = self.attempts < Self::LIMIT;
        if writable {
            self.attempts += 1;
        }
        let delay = 60u64 << self.attempts.saturating_sub(1);
        self.not_before = now.saturating_add(delay).saturating_add(jitter % 31);
        Ok(writable)
    }
}
