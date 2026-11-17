//! Read-only GTK status card. Call `update` on the GTK main thread when a
//! producer emits a new snapshot; the component performs no polling or engine IO.
mod models;
use adw::{gtk, prelude::*};
use gettextrs::gettext;
pub(crate) use models::{SyncActivity, SyncStatusData};

/// Reusable card with one public presentation update boundary.
#[derive(Clone)]
pub(crate) struct SyncStatus {
    root: gtk::Box,
    title: gtk::Label,
    detail: gtk::Label,
    icon: gtk::Image,
    spinner: gtk::Spinner,
    percent: gtk::Label,
    progress: gtk::ProgressBar,
}
impl SyncStatus {
    pub fn new() -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 18);
        root.add_css_class("card");
        root.set_widget_name("sync-status");
        let body = gtk::Box::new(gtk::Orientation::Vertical, 18);
        for set in [
            gtk::Widget::set_margin_top,
            gtk::Widget::set_margin_bottom,
            gtk::Widget::set_margin_start,
            gtk::Widget::set_margin_end,
        ] {
            set(body.upcast_ref(), 24);
        }
        root.append(&body);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 18);
        let icon = gtk::Image::from_icon_name("emblem-ok-symbolic");
        icon.set_pixel_size(48);
        icon.add_css_class("success");
        let spinner = gtk::Spinner::new();
        spinner.set_size_request(48, 48);
        spinner.add_css_class("accent");
        row.append(&icon);
        row.append(&spinner);
        let text = gtk::Box::new(gtk::Orientation::Vertical, 6);
        text.set_hexpand(true);
        let title = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .css_classes(["title-2"])
            .build();
        let detail = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        text.append(&title);
        text.append(&detail);
        row.append(&text);
        let percent = gtk::Label::builder().css_classes(["title-2"]).build();
        row.append(&percent);
        body.append(&row);
        let progress = gtk::ProgressBar::new();
        progress.add_css_class("osd");
        body.append(&progress);
        let result = Self {
            root,
            title,
            detail,
            icon,
            spinner,
            percent,
            progress,
        };
        result.update(&SyncStatusData::default());
        result
    }
    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// Replace all visual state atomically on the GTK main thread. Unconfirmed
    /// inactivity hides the card instead of making an unsupported success claim.
    pub fn update(&self, data: &SyncStatusData) {
        use SyncActivity::*;
        let active = data.activity != Idle;
        self.root.set_visible(active || data.synchronized);
        self.icon.set_visible(!active);
        self.spinner.set_visible(active);
        self.spinner.set_spinning(active);
        self.title.set_label(&gettext(if active {
            "Syncing files"
        } else {
            "Everything is synchronized"
        }));
        let message = match data.activity {
            Idle => "Your files are up to date between your computer and Google Drive.",
            Upload => "Uploading files…",
            Download => "Downloading files…",
            Scanning => "Checking local files…",
            CheckingRemote => "Checking cloud changes…",
            Renaming => "Renaming files…",
            Moving => "Moving files…",
            Deleting => "Deleting files…",
            Reconciling => "Syncing changes…",
        };
        let mut detail = gettext(message);
        if matches!(data.activity, Upload | Download) {
            if let (Some(done), Some(total)) = (data.completed, data.total.filter(|n| *n > 0)) {
                detail = gettext(if data.activity == Upload {
                    "Uploading {completed} of {total} files"
                } else {
                    "Downloading {completed} of {total} files"
                })
                .replace("{completed}", &done.min(total).to_string())
                .replace("{total}", &total.to_string());
            }
            if let Some(bytes) = data.transferred_bytes {
                detail.push_str(&format!(" ({})", gtk::glib::format_size(bytes)));
            }
        }
        self.detail.set_label(&detail);
        let fraction = data.fraction();
        self.progress.set_visible(fraction.is_some());
        self.percent.set_visible(fraction.is_some());
        self.progress.set_fraction(fraction.unwrap_or(0.0));
        self.percent.set_label(
            &fraction
                .map(|n| format!("{:.0}%", n * 100.0))
                .unwrap_or_default(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires an isolated display"]
    fn sync_status_widget_transitions() {
        adw::init().unwrap();
        let card = SyncStatus::new();
        assert!(!card.widget().is_visible());
        card.update(&SyncStatusData {
            synchronized: true,
            ..Default::default()
        });
        assert!(card.widget().is_visible());
        assert!(card.icon.is_visible());
        assert!(!card.progress.is_visible());
        card.update(&SyncStatusData {
            activity: SyncActivity::Upload,
            completed: Some(3),
            total: Some(12),
            ..Default::default()
        });
        assert_eq!(card.progress.fraction(), 0.25);
        assert_eq!(card.percent.label(), "25%");
        assert!(card.progress.is_visible());
        card.update(&SyncStatusData {
            activity: SyncActivity::Scanning,
            ..Default::default()
        });
        assert!(!card.progress.is_visible());
        assert!(!card.percent.is_visible());
        assert!(card.spinner.is_spinning());
        card.update(&SyncStatusData::default());
        assert!(!card.widget().is_visible());
        assert!(!card.spinner.is_spinning());
    }
}
