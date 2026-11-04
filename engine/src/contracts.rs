//! Inert public data contracts retained for later synchronization design.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A previously defined bulk-impact policy; it does not enable any operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkPolicy {
    pub absolute: usize,
    pub minimum: usize,
    pub percent: u8,
}

impl Default for BulkPolicy {
    fn default() -> Self {
        Self {
            absolute: 100,
            minimum: 10,
            percent: 20,
        }
    }
}

/// A previously defined impact summary, with no execution authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkImpact {
    pub folder: PathBuf,
    pub affected: usize,
    pub population: usize,
}

/// A non-executable review proposal retained as an error data contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkReview {
    fingerprint: String,
    impacts: Vec<BulkImpact>,
}

impl BulkReview {
    /// Returns the recorded impact summaries.
    pub fn impacts(&self) -> &[BulkImpact] {
        &self.impacts
    }
}

impl std::fmt::Display for BulkReview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Synchronization paused for bulk changes. Review the affected folders before continuing. Existing data was preserved.")
    }
}
