//! Typed preferences and their `GKeyFile` persistence.
use adw::gtk::glib;
use std::path::PathBuf;

const GROUP: &str = "Preferences";

/// Preferred application color scheme. `System` follows the desktop setting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ColorScheme {
    #[default]
    System,
    Light,
    Dark,
}

impl ColorScheme {
    pub(crate) const ALL: [ColorScheme; 3] = [Self::System, Self::Light, Self::Dark];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::System => "Follow System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    fn from_key(key: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|scheme| scheme.key() == key)
            .unwrap_or_default()
    }

    pub(crate) fn adw(self) -> adw::ColorScheme {
        match self {
            Self::System => adw::ColorScheme::Default,
            Self::Light => adw::ColorScheme::ForceLight,
            Self::Dark => adw::ColorScheme::ForceDark,
        }
    }
}

/// Allowed automatic synchronization intervals in seconds, with display labels.
pub(crate) const SYNC_INTERVALS: [(u64, &str); 5] = [
    (60, "Every Minute"),
    (300, "Every 5 Minutes"),
    (900, "Every 15 Minutes"),
    (1800, "Every 30 Minutes"),
    (3600, "Every Hour"),
];

/// Persisted application preferences.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Preferences {
    pub color_scheme: ColorScheme,
    /// Delay between automatic synchronization cycles; always one of [`SYNC_INTERVALS`].
    pub sync_interval: u64,
    /// Keeps DriveSync running when the window is closed; otherwise closing quits.
    pub keep_running: bool,
    /// Posts a notification when the window is hidden and DriveSync keeps running.
    pub notify_on_close: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            color_scheme: ColorScheme::default(),
            sync_interval: SYNC_INTERVALS[0].0,
            keep_running: true,
            notify_on_close: true,
        }
    }
}

impl Preferences {
    fn path() -> PathBuf {
        glib::user_config_dir()
            .join("drivesync")
            .join("preferences.ini")
    }

    /// Reads the stored preferences; any missing or invalid entry uses its default.
    pub(crate) fn load() -> Self {
        let file = glib::KeyFile::new();
        match file.load_from_file(Self::path(), glib::KeyFileFlags::NONE) {
            Ok(_) => Self::from_key_file(&file),
            Err(_) => Self::default(),
        }
    }

    /// Writes the preferences atomically (temporary file and rename).
    pub(crate) fn save(&self) -> Result<(), glib::Error> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| glib::Error::new(glib::FileError::Failed, &e.to_string()))?;
        }
        glib::file_set_contents(path, self.to_key_file().to_data().as_bytes())
    }

    pub(crate) fn from_key_file(file: &glib::KeyFile) -> Self {
        let defaults = Self::default();
        Self {
            color_scheme: file
                .string(GROUP, "color-scheme")
                .map(|value| ColorScheme::from_key(&value))
                .unwrap_or(defaults.color_scheme),
            sync_interval: file
                .integer(GROUP, "sync-interval")
                .ok()
                .and_then(|value| u64::try_from(value).ok())
                .filter(|value| SYNC_INTERVALS.iter().any(|(secs, _)| secs == value))
                .unwrap_or(defaults.sync_interval),
            keep_running: file
                .boolean(GROUP, "keep-running")
                .unwrap_or(defaults.keep_running),
            notify_on_close: file
                .boolean(GROUP, "notify-on-close")
                .unwrap_or(defaults.notify_on_close),
        }
    }

    pub(crate) fn to_key_file(self) -> glib::KeyFile {
        let file = glib::KeyFile::new();
        file.set_string(GROUP, "color-scheme", self.color_scheme.key());
        file.set_integer(GROUP, "sync-interval", self.sync_interval as i32);
        file.set_boolean(GROUP, "keep-running", self.keep_running);
        file.set_boolean(GROUP, "notify-on-close", self.notify_on_close);
        file
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_falls_back_to_defaults() {
        let value = Preferences {
            color_scheme: ColorScheme::Dark,
            sync_interval: 900,
            keep_running: false,
            notify_on_close: false,
        };
        assert_eq!(Preferences::from_key_file(&value.to_key_file()), value);
        let invalid = glib::KeyFile::new();
        invalid.set_string(GROUP, "color-scheme", "neon");
        invalid.set_string(GROUP, "notify-on-close", "maybe");
        invalid.set_string(GROUP, "keep-running", "maybe");
        invalid.set_integer(GROUP, "sync-interval", 7);
        assert_eq!(Preferences::from_key_file(&invalid), Preferences::default());
    }
}
