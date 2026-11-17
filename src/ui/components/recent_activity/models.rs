//! Completed activity snapshots, independent of engine operations and inventory.
use gettextrs::{gettext, ngettext};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActivityKind {
    Uploaded,
    Downloaded,
    Updated,
}
impl ActivityKind {
    pub fn label(self) -> String {
        gettext(match self {
            Self::Uploaded => "Uploaded",
            Self::Downloaded => "Downloaded",
            Self::Updated => "Updated",
        })
    }
    pub fn icon(self) -> &'static str {
        match self {
            Self::Uploaded => "go-up-symbolic",
            Self::Downloaded => "go-down-symbolic",
            Self::Updated => "view-refresh-symbolic",
        }
    }
}

/// A confirmed successful activity, not a queued or currently running operation.
/// IDs must be unique and account-qualified; times are UTC Unix seconds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ActivityItem {
    pub id: String,
    pub name: String,
    pub path: String,
    pub account: String,
    pub kind: ActivityKind,
    pub bytes: Option<u64>,
    pub completed_at: i64,
}

/// Sort a borrowed snapshot newest-first with stable tie breaking.
pub(super) fn ordered(items: &[ActivityItem]) -> Vec<&ActivityItem> {
    let mut result: Vec<_> = items.iter().collect();
    result.sort_by(|a, b| {
        b.completed_at
            .cmp(&a.completed_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    result
}

/// Relative labels use the caller's snapshot time, clamping future timestamps.
pub(super) fn elapsed(completed_at: i64, now: i64) -> String {
    let seconds = now.saturating_sub(completed_at).max(0) as u64;
    let (count, singular, plural) = if seconds < 60 {
        return gettext("Just now");
    } else if seconds < 3600 {
        (seconds / 60, "{count} minute ago", "{count} minutes ago")
    } else if seconds < 86400 {
        (seconds / 3600, "{count} hour ago", "{count} hours ago")
    } else {
        (seconds / 86400, "{count} day ago", "{count} days ago")
    };
    ngettext(singular, plural, count.min(u32::MAX as u64) as u32)
        .replace("{count}", &count.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn elapsed_handles_boundaries_and_clock_skew() {
        assert_eq!(elapsed(101, 100), "Just now");
        assert_eq!(elapsed(0, 59), "Just now");
        assert_eq!(elapsed(0, 60), "1 minute ago");
        assert_eq!(elapsed(0, 120), "2 minutes ago");
        assert_eq!(elapsed(0, 3600), "1 hour ago");
        assert_eq!(elapsed(0, 86400), "1 day ago");
    }
}
