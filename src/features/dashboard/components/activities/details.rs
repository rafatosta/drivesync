//! Read-only details for an observed activity.
use super::rows::{activity_title, formatted, origin, path_text};
use adw::{gtk, prelude::*};
use drivesync_engine::{ActivityRecord, LocalActivity};
use gettextrs::gettext;

pub(super) struct Details {
    fields: gtk::glib::WeakRef<gtk::Box>,
    empty: gtk::glib::WeakRef<gtk::Label>,
}

impl Details {
    pub fn new(split: &adw::NavigationSplitView) -> (gtk::Box, Self) {
        let root = crate::ui::content();
        root.set_widget_name("activity-details");
        let back = gtk::Button::from_icon_name("go-previous-symbolic");
        back.set_widget_name("activity-details-back");
        back.set_tooltip_text(Some(&gettext("Back to activities")));
        back.set_halign(gtk::Align::Start);
        back.add_css_class("flat");
        split
            .bind_property("collapsed", &back, "visible")
            .sync_create()
            .build();
        let weak_split = split.downgrade();
        back.connect_clicked(move |_| {
            if let Some(split) = weak_split.upgrade() {
                split.set_show_content(false);
            }
        });
        root.append(&back);

        let empty = gtk::Label::new(Some(&gettext("Select an activity to see its details.")));
        empty.set_wrap(true);
        empty.add_css_class("dim-label");
        root.append(&empty);
        let fields = gtk::Box::new(gtk::Orientation::Vertical, 16);
        root.append(&fields);
        (
            root,
            Self {
                fields: fields.downgrade(),
                empty: empty.downgrade(),
            },
        )
    }

    pub fn show(&self, record: &ActivityRecord, account: &str) {
        let (Some(fields), Some(empty)) = (self.fields.upgrade(), self.empty.upgrade()) else {
            return;
        };
        super::clear(&fields);
        empty.set_visible(false);
        let title = gtk::Label::new(Some(&activity_title(record)));
        title.set_wrap(true);
        title.set_xalign(0.0);
        title.add_css_class("title-3");
        fields.append(&title);
        Self::field(&fields, &gettext("Account"), account);
        Self::field(&fields, &gettext("Origin"), &origin(record));
        if let Ok(date) = gtk::glib::DateTime::from_unix_local(record.observed_at) {
            Self::field(
                &fields,
                &gettext("Date and time"),
                &formatted(&date, "%x %X"),
            );
        }
        if let LocalActivity::Moved { from } = &record.notice.activity {
            Self::field(&fields, &gettext("Previous path"), &path_text(from));
            Self::field(
                &fields,
                &gettext("New path"),
                &path_text(&record.notice.path),
            );
        } else {
            Self::field(&fields, &gettext("Path"), &path_text(&record.notice.path));
        }
        if let Some(integrity) = &record.integrity {
            Self::field(
                &fields,
                &gettext("SHA-256 of downloaded bytes"),
                &integrity.local,
            );
            Self::field(
                &fields,
                &gettext("Google Drive SHA-256"),
                integrity
                    .remote
                    .as_deref()
                    .unwrap_or(&gettext("Unavailable")),
            );
            let status = match integrity.remote.as_deref() {
                Some(remote) if remote == integrity.local => gettext("Checksums match"),
                Some(_) => gettext("Checksums differ"),
                None => gettext("Remote checksum unavailable for comparison"),
            };
            Self::field(&fields, &gettext("Download integrity"), &status);
        }
    }

    pub fn clear(&self) {
        if let (Some(fields), Some(empty)) = (self.fields.upgrade(), self.empty.upgrade()) {
            super::clear(&fields);
            empty.set_visible(true);
        }
    }

    fn field(fields: &gtk::Box, title: &str, value: &str) {
        let field = gtk::Box::new(gtk::Orientation::Vertical, 3);
        let label = gtk::Label::new(Some(title));
        label.set_xalign(0.0);
        label.add_css_class("heading");
        field.append(&label);
        let value = gtk::Label::new(Some(value));
        value.set_wrap(true);
        value.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        value.set_selectable(true);
        value.set_xalign(0.0);
        field.append(&value);
        fields.append(&field);
    }
}
