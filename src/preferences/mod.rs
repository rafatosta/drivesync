//! Application-wide preferences: persistence and the native preferences window.
//!
//! Global preferences use a key file. Account-scoped upload and conflict controls
//! use the engine's existing per-account storage and never become global defaults.
//! Unknown global values fall back to defaults without deleting the file.
//!
//! - [`model`]: typed values and their key-file representation.
//! - [`view`]: the adaptive preferences window and the `app.preferences` action.
mod accounts;
mod conflict_preferences;
mod model;
mod upload_mode;
mod view;

pub(crate) use accounts::{AccountSettings, Accounts};
pub(crate) use model::{ColorScheme, Preferences, SYNC_INTERVALS};
pub(crate) use view::{install, present_with_accounts};

#[cfg(test)]
pub(crate) use upload_mode::verify_presentations as verify_upload_presentations;

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
