//! Application-wide preferences: persistence and the embedded preferences page.
//!
//! Global preferences use a key file.
//! Unknown global values fall back to defaults without deleting the file.
//!
//! - [`model`]: typed values and their key-file representation.
//! - [`window`]: the adaptive preferences page and the `app.preferences` action.
pub(crate) use crate::services::preferences::{ColorScheme, Preferences};
mod dialog;

pub(crate) use dialog::install;

#[cfg(test)]
pub(crate) use dialog::build as test_build;
