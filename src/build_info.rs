//! Shared identity embedded in the executable, independent of the application ID.

/// Application version and abbreviated source revision for compact presentation.
pub(crate) const VERSION: &str = env!("DRIVESYNC_VERSION");

/// Full source revision for support, including tracked local changes when present.
pub(crate) const REVISION: &str = env!("DRIVESYNC_SOURCE_REVISION");
