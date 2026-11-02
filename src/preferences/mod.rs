//! Application-wide preferences: persistence and the native preferences window.
//!
//! Preferences are global to the application and never hold credentials,
//! account data or synchronization state. Account-scoped options (folders,
//! uploads, conflict handling) remain in the account dashboard. Unknown or
//! invalid stored values fall back to defaults without deleting the file.
//!
//! - [`model`]: typed values and their key-file representation.
//! - [`view`]: the adaptive preferences window and the `app.preferences` action.
mod model;
mod view;

pub(crate) use model::{ColorScheme, Preferences, SYNC_INTERVALS};
pub(crate) use view::install;

/// Returns whether closing the window should post the "keeps running" notification.
pub(crate) fn notify_on_close() -> bool {
    Preferences::load().notify_on_close
}

/// Returns whether closing the window should keep DriveSync running in the background.
pub(crate) fn keep_running() -> bool {
    Preferences::load().keep_running
}

/// Returns the delay in seconds between automatic synchronization cycles.
pub(crate) fn sync_interval() -> u64 {
    Preferences::load().sync_interval
}
