//! Read-only recent activity preview and detailed in-window history.
mod models;
use adw::{gtk, prelude::*};
use gettextrs::gettext;
pub(crate) use models::{ActivityItem, ActivityKind};

/// Renders the latest three successful activities and a caller-supplied history.
/// Install once per dedicated NavigationView; updates perform no IO or polling.
pub(crate) struct RecentActivity {
    root: gtk::Box,
    preview: gtk::ListBox,
    history: gtk::ListBox,
    empty: gtk::Label,
    history_empty: adw::StatusPage,
    more: gtk::Button,
}
impl RecentActivity {
    pub fn new(navigation: &adw::NavigationView) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
        root.set_widget_name("recent-activity");
        let heading = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let title = gtk::Label::builder()
            .label(gettext("Recent activity"))
            .xalign(0.0)
            .hexpand(true)
            .css_classes(["heading"])
            .build();
        heading.append(&title);
        let more = gtk::Button::with_label(&gettext("View all"));
        more.add_css_class("flat");
        more.set_widget_name("recent-activity-more");
        heading.append(&more);
        root.append(&heading);
        let preview = list();
        root.append(&preview);
        let empty = gtk::Label::builder()
            .label(gettext("No recent activity"))
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        root.append(&empty);
        let history = list();
        let history_empty = adw::StatusPage::builder()
            .title(gettext("No recent activity"))
            .description(gettext("Completed activities will appear here."))
            .icon_name("document-open-recent-symbolic")
            .build();
        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        body.set_margin_top(24);
        body.set_margin_bottom(24);
        body.set_margin_start(24);
        body.set_margin_end(24);
        body.append(&history);
        body.append(&history_empty);
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        header.set_show_start_title_buttons(false);
        header.set_show_end_title_buttons(false);
        header.set_title_widget(Some(&adw::WindowTitle::new(
            &gettext("Activity history"),
            "",
        )));
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(
            &gtk::ScrolledWindow::builder()
                .hscrollbar_policy(gtk::PolicyType::Never)
                .child(&adw::Clamp::builder().maximum_size(900).child(&body).build())
                .build(),
        ));
        let page = adw::NavigationPage::builder()
            .title(gettext("Activity history"))
            .tag("activity-history")
            .child(&toolbar)
            .build();
        navigation.add(&page);
        let weak = navigation.downgrade();
        more.connect_clicked(move |_| {
            if let Some(nav) = weak.upgrade() {
                nav.push_by_tag("activity-history");
            }
        });
        let result = Self {
            root,
            preview,
            history,
            empty,
            history_empty,
            more,
        };
        result.update(&[], 0);
        result
    }
    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// Replace preview and history from one snapshot, including an open history
    /// page. `now` is UTC Unix seconds for relative labels. Producers refresh the
    /// snapshot when appropriate; the component never creates a timer or history DB.
    pub fn update(&self, items: &[ActivityItem], now: i64) {
        for list in [&self.preview, &self.history] {
            while let Some(child) = list.first_child() {
                list.remove(&child);
            }
        }
        let ordered = models::ordered(items);
        for (index, item) in ordered.into_iter().enumerate() {
            if index < 3 {
                self.preview.append(&row(item, now, false));
            }
            self.history.append(&row(item, now, true));
        }
        self.preview.set_visible(!items.is_empty());
        self.history.set_visible(!items.is_empty());
        self.empty.set_visible(items.is_empty());
        self.history_empty.set_visible(items.is_empty());
        self.more.set_sensitive(!items.is_empty());
    }
}
fn list() -> gtk::ListBox {
    gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build()
}
fn row(item: &ActivityItem, now: i64, detailed: bool) -> adw::ActionRow {
    let subtitle = if detailed {
        let date = gtk::glib::DateTime::from_unix_local(item.completed_at)
            .and_then(|date| date.format("%x %X"))
            .map(|date| date.to_string())
            .unwrap_or_else(|_| gettext("Date unavailable"));
        format!(
            "{} · {}\n{}\n{}",
            item.kind.label(),
            date,
            item.account,
            item.path
        )
    } else {
        format!(
            "{} · {}",
            item.kind.label(),
            models::elapsed(item.completed_at, now)
        )
    };
    let row = adw::ActionRow::new();
    row.set_use_markup(false);
    row.set_title(&item.name);
    row.set_subtitle(&subtitle);
    row.set_title_lines(1);
    let icon = gtk::Image::from_icon_name(item.kind.icon());
    icon.set_pixel_size(24);
    icon.add_css_class(if item.kind == ActivityKind::Downloaded {
        "success"
    } else {
        "accent"
    });
    row.add_prefix(&icon);
    let size = gtk::Label::builder()
        .label(
            item.bytes
                .map(|bytes| gtk::glib::format_size(bytes).to_string())
                .unwrap_or_else(|| "—".into()),
        )
        .css_classes(["dim-label"])
        .build();
    row.add_suffix(&size);
    let done = gtk::Image::from_icon_name("emblem-ok-symbolic");
    done.add_css_class("success");
    done.set_tooltip_text(Some(&gettext("Completed")));
    row.add_suffix(&done);
    row
}

#[cfg(test)]
mod tests;
