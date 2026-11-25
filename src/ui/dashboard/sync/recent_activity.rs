//! Read-only presentation of the selected account's confirmed operations.
use adw::{gtk, prelude::*};
use drivesync_engine::{RecentActivity, RecentActivityKind};
use gettextrs::{gettext, ngettext};

#[derive(Clone)]
pub(super) struct RecentActivityList {
    pub widget: adw::PreferencesGroup,
    list: gtk::ListBox,
    state: gtk::Label,
}

impl RecentActivityList {
    pub fn new() -> Self {
        let widget = adw::PreferencesGroup::builder()
            .title(gettext("Recent activity"))
            .visible(false)
            .build();
        widget.set_widget_name("dashboard-recent-activity");
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        let state = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        widget.add(&list);
        widget.add(&state);
        Self {
            widget,
            list,
            state,
        }
    }

    pub fn clear(&self) {
        self.widget.set_visible(false);
        self.clear_rows();
    }

    fn clear_rows(&self) {
        while let Some(row) = self.list.first_child() {
            self.list.remove(&row);
        }
    }

    pub fn show(&self, activities: Option<&[RecentActivity]>, now_ms: i64) {
        self.clear_rows();
        for activity in activities.into_iter().flatten() {
            self.list.append(&activity_row(activity, now_ms));
        }
        let empty = self.list.first_child().is_none();
        self.list.set_visible(!empty);
        self.state.set_visible(empty);
        if empty {
            self.state.set_label(&gettext(if activities.is_some() {
                "No recent activity"
            } else {
                "Recent activity is unavailable."
            }));
        }
        self.widget.set_visible(true);
    }
}

fn activity_row(activity: &RecentActivity, now_ms: i64) -> adw::ActionRow {
    use RecentActivityKind::*;
    let (icon, color, action) = match activity.kind {
        Upload => ("go-up-symbolic", "accent", gettext("Uploaded")),
        Download => ("go-down-symbolic", "success", gettext("Downloaded")),
        Update => ("view-refresh-symbolic", "accent", gettext("Updated")),
        Rename => ("document-edit-symbolic", "accent", gettext("Renamed")),
        Move => ("folder-symbolic", "accent", gettext("Moved")),
        Trash => (
            "user-trash-symbolic",
            "dim-label",
            gettext("Moved to trash"),
        ),
        CreateFolder => ("folder-new-symbolic", "accent", gettext("Folder created")),
        ProjectShortcut => (
            "insert-link-symbolic",
            "accent",
            gettext("Shortcut synchronized"),
        ),
    };
    let when = relative_time(now_ms.saturating_sub(activity.confirmed_at.unix_millis));
    let row = adw::ActionRow::builder()
        .title_lines(1)
        .subtitle_lines(2)
        .build();
    row.set_use_markup(false);
    row.set_title(&activity.name);
    row.set_subtitle(&format!("{action} • {when}"));
    let operation = gtk::Image::from_icon_name(icon);
    operation.add_css_class(color);
    row.add_prefix(&operation);
    if let Some(size) = activity.size {
        let size = gtk::Label::new(Some(&gtk::glib::format_size(size)));
        size.add_css_class("dim-label");
        row.add_suffix(&size);
    }
    let check = gtk::Image::from_icon_name("object-select-symbolic");
    check.add_css_class("success");
    let completed = gettext("Completed");
    check.set_tooltip_text(Some(&completed));
    check.update_property(&[gtk::accessible::Property::Label(&completed)]);
    row.add_suffix(&check);
    row
}

fn relative_time(elapsed_ms: i64) -> String {
    let minutes = elapsed_ms / 60_000;
    let (singular, plural, count) = match minutes {
        ..=0 => return gettext("Just now"),
        1..=59 => ("{count} minute ago", "{count} minutes ago", minutes),
        60..=1439 => ("{count} hour ago", "{count} hours ago", minutes / 60),
        _ => ("{count} day ago", "{count} days ago", minutes / 1440),
    };
    ngettext(singular, plural, u32::try_from(count).unwrap_or(u32::MAX))
        .replace("{count}", &count.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a GTK display"]
    fn rows_preserve_literal_names_and_clear_when_the_account_changes() {
        adw::init().unwrap();
        let history = RecentActivityList::new();
        let entry = RecentActivity {
            kind: RecentActivityKind::Upload,
            name: "<draft>&.docx".into(),
            size: Some(0),
            confirmed_at: drivesync_engine::Timestamp { unix_millis: 0 },
        };
        history.show(Some(&[entry]), 120_000);
        let row = history
            .list
            .first_child()
            .and_downcast::<adw::ActionRow>()
            .unwrap();
        assert_eq!(row.title(), "<draft>&.docx");
        assert_eq!(row.subtitle().unwrap(), "Uploaded • 2 minutes ago");
        history.clear();
        assert!(!history.widget.is_visible());
        assert!(history.list.first_child().is_none());
        history.show(Some(&[]), 120_000);
        assert!(history.widget.is_visible());
        assert!(history.state.is_visible());
        assert!(!history.list.is_visible());
    }
}
