//! Presentation and application action for global preferences.
pub(crate) use crate::services::preferences::{ColorScheme, Preferences};
mod dialog;
#[cfg(test)]
pub(crate) use dialog::build as test_build;
pub(crate) use dialog::install;
/// Maps persisted appearance to the native presentation API.
fn appearance(scheme: ColorScheme) -> adw::ColorScheme {
    match scheme {
        ColorScheme::System => adw::ColorScheme::Default,
        ColorScheme::Light => adw::ColorScheme::ForceLight,
        ColorScheme::Dark => adw::ColorScheme::ForceDark,
    }
}

mod configuration;
