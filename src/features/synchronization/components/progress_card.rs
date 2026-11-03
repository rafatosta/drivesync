//! Snapshot-driven synchronization presentation, with no worker or global subscriptions.
use crate::features::synchronization::automatic::{Phase, Snapshot};
use adw::{gtk, prelude::*};

/// A reusable account-scoped view. The caller supplies snapshots from one account
/// and replaces the view when switching accounts; presenting never controls jobs.
#[derive(Clone)]
pub(crate) struct SyncProgressCard {
    root: gtk::Box,
    icon: gtk::Image,
    title: gtk::Label,
    description: gtk::Label,
    percentage: gtk::Label,
    progress: gtk::ProgressBar,
}

impl SyncProgressCard {
    pub(crate) fn new() -> Self {
        let progress = gtk::ProgressBar::new();
        progress.set_widget_name("sync-card-progress");
        let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
        root.add_css_class("card");
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(18);
        content.set_margin_bottom(18);
        content.set_margin_start(18);
        content.set_margin_end(18);
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        let icon = gtk::Image::new();
        icon.set_pixel_size(32);
        let text = gtk::Box::new(gtk::Orientation::Vertical, 4);
        text.set_hexpand(true);
        let title = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .css_classes(["heading"])
            .build();
        let description = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .css_classes(["dim-label"])
            .build();
        let percentage = gtk::Label::new(None);
        text.append(&title);
        text.append(&description);
        header.append(&icon);
        header.append(&text);
        header.append(&percentage);
        content.append(&header);
        content.append(&progress);
        root.append(&content);
        Self {
            root,
            icon,
            title,
            description,
            percentage,
            progress: progress.clone(),
        }
    }

    pub(crate) fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// Updates all visual properties from a complete snapshot. Progress is cycle
    /// item progress, never current-file bytes, and stays below 100 until success.
    pub(crate) fn present(&self, snapshot: &Snapshot) {
        let (title, detail, icon, class) = match snapshot.phase {
            Phase::Running if snapshot.pausing => (
                "Pausing synchronization",
                "Waiting for a safe stopping point.",
                "media-playback-pause-symbolic",
                "accent",
            ),
            Phase::Running => (
                "Synchronizing files",
                "Checking for changes…",
                "view-refresh-symbolic",
                "accent",
            ),
            Phase::Paused => (
                "Synchronization paused",
                "Completed work is preserved. Resume to continue.",
                "media-playback-pause-symbolic",
                "dim-label",
            ),
            Phase::Offline => (
                "No connection",
                "Waiting for a connection. Synchronization will retry automatically.",
                "network-offline-symbolic",
                "warning",
            ),
            Phase::Blocked | Phase::Attention => (
                "Synchronization needs attention",
                "Review the pending synchronization problems.",
                "dialog-warning-symbolic",
                "warning",
            ),
            Phase::Retrying => (
                "Retrying synchronization",
                "A temporary failure prevented completion. Retrying automatically.",
                "dialog-warning-symbolic",
                "warning",
            ),
            Phase::UpToDate if complete(snapshot) => (
                "Everything synchronized",
                "Your files are up to date on this computer and Google Drive.",
                "object-select-symbolic",
                "success",
            ),
            Phase::UpToDate => (
                "Synchronization needs attention",
                "Some items are still pending or completion could not be confirmed.",
                "dialog-warning-symbolic",
                "warning",
            ),
            Phase::Waiting => (
                "Waiting for synchronization",
                "Configured accounts are checked automatically.",
                "appointment-soon-symbolic",
                "dim-label",
            ),
            Phase::Unconfigured => (
                "Waiting for configuration",
                "Choose a folder to enable synchronization.",
                "folder-symbolic",
                "dim-label",
            ),
        };
        self.title.set_label(&gettextrs::gettext(title));
        self.description.set_label(&gettextrs::gettext(detail));
        if snapshot.phase == Phase::Running && !snapshot.pausing {
            if let Some(stage) = snapshot
                .progress
                .as_ref()
                .and_then(|p| p.preparation.as_ref())
            {
                let (title, detail) =
                    crate::features::dashboard::presentation::preparation_text(stage);
                self.title.set_label(&gettextrs::gettext(title));
                self.description.set_label(&detail);
            }
        }
        if matches!(
            snapshot.phase,
            Phase::Attention | Phase::Blocked | Phase::UpToDate
        ) {
            if let Some(Ok(report)) = &snapshot.result {
                if !report.attention().is_empty() {
                    self.description.set_label(
                        &crate::features::dashboard::presentation::attention_text(report),
                    );
                }
            }
        }
        let theme = gtk::IconTheme::for_display(&self.root.display());
        let icon = if theme.has_icon(icon) {
            icon
        } else if class == "warning" {
            "dialog-warning-symbolic"
        } else {
            "view-refresh-symbolic"
        };
        self.icon.set_icon_name(Some(icon));
        for name in ["accent", "warning", "success", "dim-label"] {
            self.icon.remove_css_class(name);
        }
        self.icon.add_css_class(class);
        let fraction = fraction(snapshot);
        self.progress.set_show_text(false);
        self.progress.set_visible(fraction.is_some());
        self.percentage.set_visible(fraction.is_some());
        self.progress.set_fraction(fraction.unwrap_or(0.0));
        self.percentage.set_label(&format!(
            "{} %",
            (fraction.unwrap_or(0.0) * 100.0).floor() as u32
        ));
        if matches!(snapshot.phase, Phase::Running | Phase::Paused) {
            if let Some(event) = &snapshot.progress {
                if fraction.is_some() {
                    let detail = progress_detail(
                        event,
                        snapshot.phase == Phase::Running && !snapshot.pausing,
                    );
                    self.description.set_label(&detail);
                }
            }
        }
    }
}

/// Reports cycle item counts and current-file bytes without revealing paths.
/// Counts include folders and skipped items; bytes belong only to the current file.
fn progress_detail(event: &drivesync_engine::DownloadProgress, active: bool) -> String {
    use drivesync_engine::DownloadStage;
    let action = match event
        .current
        .as_ref()
        .filter(|_| active)
        .map(|item| item.stage)
    {
        Some(DownloadStage::Uploading) => "Uploading",
        Some(DownloadStage::Downloading) => "Downloading",
        Some(DownloadStage::Checking) => "Checking",
        Some(DownloadStage::Verifying) => "Verifying",
        Some(DownloadStage::Installing) => "Saving",
        Some(DownloadStage::Linking) => "Creating a shortcut",
        None => "Synchronization",
    };
    let mut detail = gettextrs::gettext("{action} · {completed} of {total} items processed")
        .replace("{action}", &gettextrs::gettext(action))
        .replace("{completed}", &event.completed_items.to_string())
        .replace("{total}", &event.total_items.to_string());
    if let Some(item) = event.current.as_ref().filter(|item| {
        matches!(
            item.stage,
            DownloadStage::Uploading | DownloadStage::Downloading
        )
    }) {
        detail.push_str(&format!(" ({})", gtk::glib::format_size(item.received)));
    }
    detail
}

fn complete(snapshot: &Snapshot) -> bool {
    crate::features::dashboard::presentation::confirmed(snapshot)
}

fn fraction(snapshot: &Snapshot) -> Option<f64> {
    if !matches!(snapshot.phase, Phase::Running | Phase::Paused) {
        return None;
    }
    let progress = snapshot.progress.as_ref()?;
    if progress.preparation.is_some() || progress.total_items == 0 {
        return None;
    }
    Some((progress.completed_items as f64 / progress.total_items as f64).clamp(0.0, 0.99))
}

#[cfg(test)]
mod tests {
    use super::*;
    use drivesync_engine::{DownloadProgress, DownloadReport};

    #[test]
    #[ignore = "requires a desktop display"]
    fn native_progress_states_use_available_symbolic_icons() {
        adw::init().unwrap();
        let theme = gtk::IconTheme::for_display(&gtk::gdk::Display::default().unwrap());
        let card = SyncProgressCard::new();
        let mut missing = Vec::new();
        for phase in [
            Phase::Unconfigured,
            Phase::Waiting,
            Phase::Running,
            Phase::Paused,
            Phase::Offline,
            Phase::Retrying,
            Phase::Blocked,
            Phase::Attention,
            Phase::UpToDate,
        ] {
            let snapshot = Snapshot {
                revision: 1,
                phase,
                pausing: false,
                progress: None,
                result: Some(Ok(DownloadReport::default())),
            };
            card.present(&snapshot);
            let icon = card.icon.icon_name().unwrap();
            if !theme.has_icon(&icon) {
                missing.push(format!("{phase:?}: {icon}"));
            }
        }
        assert!(
            missing.is_empty(),
            "theme {:?} lacks {:?}",
            theme.theme_name(),
            missing
        );
    }
    #[test]
    fn processing_feedback_omits_paths_and_keeps_item_and_byte_scopes() {
        use drivesync_engine::{DownloadItemProgress, DownloadStage};
        let mut event = DownloadProgress {
            preparation: None,
            completed_items: 3,
            total_items: 12,
            current: Some(DownloadItemProgress {
                relative_path: "private-folder/private-file.txt".into(),
                stage: DownloadStage::Uploading,
                received: 24_600_000,
                total: Some(30_000_000),
            }),
        };
        for (stage, action) in [
            (DownloadStage::Uploading, "Uploading"),
            (DownloadStage::Downloading, "Downloading"),
            (DownloadStage::Checking, "Checking"),
            (DownloadStage::Verifying, "Verifying"),
            (DownloadStage::Installing, "Saving"),
            (DownloadStage::Linking, "Creating a shortcut"),
        ] {
            event.current.as_mut().unwrap().stage = stage;
            let text = progress_detail(&event, true);
            assert!(text.starts_with(action));
            assert!(text.contains("3 of 12 items processed"));
            assert!(!text.contains("private"));
            assert_eq!(
                text.contains(&gtk::glib::format_size(24_600_000).to_string()),
                matches!(stage, DownloadStage::Uploading | DownloadStage::Downloading)
            );
            assert!(progress_detail(&event, false).starts_with("Synchronization"));
        }
        let (_, preparing) = crate::features::dashboard::presentation::preparation_text(
            &drivesync_engine::PreparationProgress::PreparingUpload {
                current_path: "private-folder/private-file.txt".into(),
            },
        );
        assert!(!preparing.contains("private"));
        event.current = None;
        assert_eq!(
            progress_detail(&event, true),
            "Synchronization · 3 of 12 items processed"
        );
    }

    #[test]
    fn unknown_totals_and_idle_states_hide_progress() {
        let mut snapshot = Snapshot {
            revision: 1,
            phase: Phase::Running,
            pausing: false,
            result: None,
            progress: Some(DownloadProgress {
                preparation: None,
                completed_items: 0,
                total_items: 0,
                current: None,
            }),
        };
        assert_eq!(fraction(&snapshot), None);
        snapshot.progress.as_mut().unwrap().total_items = 10;
        snapshot.progress.as_mut().unwrap().preparation =
            Some(drivesync_engine::PreparationProgress::Connecting);
        assert_eq!(fraction(&snapshot), None);
        snapshot.progress.as_mut().unwrap().preparation = None;
        for phase in [
            Phase::Unconfigured,
            Phase::Waiting,
            Phase::Offline,
            Phase::Retrying,
            Phase::Attention,
            Phase::Blocked,
            Phase::UpToDate,
        ] {
            snapshot.phase = phase;
            assert_eq!(fraction(&snapshot), None);
        }
    }

    #[test]
    fn processing_never_claims_completion_and_pause_preserves_progress() {
        let mut snapshot = Snapshot {
            revision: 1,
            phase: Phase::Running,
            pausing: false,
            progress: Some(DownloadProgress {
                preparation: None,
                completed_items: 3,
                total_items: 12,
                current: None,
            }),
            result: None,
        };
        assert_eq!(fraction(&snapshot), Some(0.25));
        snapshot.phase = Phase::Paused;
        assert_eq!(fraction(&snapshot), Some(0.25));
        snapshot.progress.as_mut().unwrap().completed_items = 12;
        assert_eq!(fraction(&snapshot), Some(0.99));
        snapshot.phase = Phase::UpToDate;
        assert!(!complete(&snapshot));
        snapshot.result = Some(Ok(DownloadReport::default()));
        assert!(complete(&snapshot));
        snapshot.result = Some(Ok(DownloadReport {
            skipped: 1,
            ..Default::default()
        }));
        assert!(!complete(&snapshot));
        snapshot.result = Some(Ok(DownloadReport {
            issues: vec![drivesync_engine::DownloadIssue {
                relative_path: None,
                error: drivesync_engine::EngineError::DownloadConflict,
            }],
            ..Default::default()
        }));
        assert!(!complete(&snapshot));
        snapshot.result = Some(Ok(DownloadReport::default()));
        snapshot.progress.as_mut().unwrap().completed_items = 11;
        assert!(!complete(&snapshot));
    }
}
