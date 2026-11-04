//! Bounded Linux filesystem activity observation, without content reads or sync.
//! Sessions are independent and volatile. Coverage gaps require later investigation.
mod models;
mod watcher;
pub use models::{
    LocalActivity, LocalBatch, LocalCoverageGap, LocalNotice, LocalScope, LocalWatchError,
    LocalWatchOptions, LocalWatchState,
};
pub use watcher::LocalWatcher;
