//! Gettext configuration shared by the GTK presentation.

use gettextrs::{bind_textdomain_codeset, bindtextdomain, setlocale, textdomain, LocaleCategory};

/// Initializes translations before creating any UI or worker threads.
pub(crate) fn init() {
    // SAFETY: this is called at the start of main, before GTK or any threads.
    if unsafe { setlocale(LocaleCategory::LcAll, "") }.is_none() {
        eprintln!("Could not activate the locale requested by the environment");
    }
    bindtextdomain("drivesync", "/app/share/locale")
        .expect("Could not bind the DriveSync translation catalog");
    bind_textdomain_codeset("drivesync", "UTF-8")
        .expect("Could not set the translation catalog encoding");
    textdomain("drivesync").expect("Could not select the DriveSync translation domain");
}
