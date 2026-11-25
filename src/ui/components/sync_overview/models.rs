//! Caller-owned presentation data; counts represent items, never engine operations.
use std::time::SystemTime;

/// An explicit visual state. Missing values remain distinct from zero.
#[derive(Clone, Debug, PartialEq)]
pub enum SyncOverviewState {
    Checking {
        checked_items: Option<u64>,
    },
    /// An item needs evidence; no active transfer is implied.
    Waiting {
        message: String,
    },
    Syncing {
        /// Unavailable when the source reports operations rather than confirmed items.
        completed_items: Option<u64>,
        total_items: Option<u64>,
        remaining_items: Option<u64>,
        remaining_bytes: Option<u64>,
        /// Normalized progress. Finite values are clamped; non-finite values are unavailable.
        progress: Option<f64>,
    },
    /// A selected account whose synchronization cannot currently run.
    Inactive {
        message: String,
    },
    Completed {
        last_sync: Option<SystemTime>,
    },
    Attention {
        pending_items: Option<u64>,
    },
}
