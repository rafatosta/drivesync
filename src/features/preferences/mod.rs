//! Application-wide preferences: persistence and the embedded preferences page.
//!
//! Global preferences use a key file.
//! Unknown global values fall back to defaults without deleting the file.
//!
//! - [`model`]: typed values and their key-file representation.
//! - [`window`]: the adaptive preferences page and the `app.preferences` action.
mod model;
mod window;

pub(crate) use model::{ColorScheme, Preferences};
pub(crate) use window::install;

/// Returns whether closing the window should post the "keeps running" notification.
pub(crate) fn notify_on_close() -> bool {
    Preferences::load().notify_on_close
}

/// Returns whether closing the window should keep DriveSync running in the background.
pub(crate) fn keep_running() -> bool {
    Preferences::load().keep_running
}

#[cfg(test)]
pub(crate) use window::build as test_build;
