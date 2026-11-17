//! In-window detail pages; cancellation emits no intent and stale pages cannot submit.
use super::*;
use std::cell::Cell;

fn label(text: &str, class: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .css_classes([class])
        .build()
}
fn margins(widget: &impl IsA<gtk::Widget>, value: i32) {
    widget.set_margin_top(value);
    widget.set_margin_bottom(value);
    widget.set_margin_start(value);
    widget.set_margin_end(value);
}
fn version(title: &str, icon: &str, value: &FileVersion) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    card.add_css_class("card");
    card.set_hexpand(true);
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    margins(&content, 16);
    let image = gtk::Image::from_icon_name(icon);
    image.set_pixel_size(28);
    content.append(&image);
    let text = gtk::Box::new(gtk::Orientation::Vertical, 4);
    text.append(&label(title, "heading"));
    text.append(&label(
        value
            .modified
            .as_deref()
            .unwrap_or(&gettext("Modification time unavailable")),
        "dim-label",
    ));
    text.append(&label(
        &value
            .bytes
            .map(|n| gtk::glib::format_size(n).to_string())
            .unwrap_or_else(|| gettext("Size unavailable")),
        "heading",
    ));
    content.append(&text);
    card.append(&content);
    card
}

pub(super) fn open(
    nav: &adw::NavigationView,
    item: &AlertItem,
    callback: ActionHandler,
    revision: Rc<Cell<u64>>,
) {
    let expected = revision.get();
    let conflict = matches!(item.details, AlertDetails::Conflict { .. });
    let title = gettext(if conflict {
        "Resolve conflict"
    } else {
        "Fix problem"
    });
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_show_start_title_buttons(false);
    header.set_show_end_title_buttons(false);
    header.set_title_widget(Some(&adw::WindowTitle::new(&title, &item.account)));
    let close = gtk::Button::from_icon_name("window-close-symbolic");
    close.set_tooltip_text(Some(&gettext("Close review")));
    close.set_widget_name("review-close");
    close.add_css_class("flat");
    let weak = nav.downgrade();
    close.connect_clicked(move |_| {
        if let Some(nav) = weak.upgrade() {
            nav.pop();
        }
    });
    header.pack_end(&close);
    toolbar.add_top_bar(&header);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 24);
    margins(&body, 24);
    let file = adw::ActionRow::builder()
        .title(&item.title)
        .subtitle(&item.detail)
        .build();
    file.set_use_markup(false);
    file.add_prefix(&gtk::Image::from_icon_name(if conflict {
        "text-x-generic-symbolic"
    } else {
        "dialog-warning-symbolic"
    }));
    body.append(&file);
    let selected = Rc::new(Cell::new("both"));
    let action = match &item.details {
        AlertDetails::Conflict { local, remote } => {
            let versions = gtk::FlowBox::builder()
                .selection_mode(gtk::SelectionMode::None)
                .homogeneous(true)
                .min_children_per_line(1)
                .max_children_per_line(2)
                .row_spacing(12)
                .column_spacing(12)
                .build();
            versions.insert(
                &version(&gettext("Local version"), "computer-symbolic", local),
                -1,
            );
            versions.insert(
                &version(
                    &gettext("Google Drive version"),
                    "folder-remote-symbolic",
                    remote,
                ),
                -1,
            );
            body.append(&versions);
            body.append(&label(
                &gettext("How would you like to resolve this?"),
                "heading",
            ));
            let choices = gtk::ListBox::new();
            choices.set_selection_mode(gtk::SelectionMode::None);
            let mut group = None;
            for (id, title, detail) in [
                (
                    "both",
                    "Keep both versions",
                    "Both versions will be preserved with different names.",
                ),
                (
                    "local",
                    "Use local version",
                    "The computer version will replace the Google Drive version.",
                ),
                (
                    "remote",
                    "Use Google Drive version",
                    "The cloud version will replace the computer version.",
                ),
            ] {
                let check = gtk::CheckButton::new();
                check.set_widget_name(&format!("resolve-{id}"));
                if let Some(first) = &group {
                    check.set_group(Some(first));
                } else {
                    group = Some(check.clone());
                }
                check.set_active(id == "both");
                let choice = selected.clone();
                check.connect_toggled(move |check| {
                    if check.is_active() {
                        choice.set(id);
                    }
                });
                let row = adw::ActionRow::builder()
                    .title(gettext(title))
                    .subtitle(gettext(detail))
                    .build();
                row.add_prefix(&check);
                row.set_activatable_widget(Some(&check));
                choices.append(&row);
            }
            body.append(&choices);
            None
        }
        AlertDetails::Problem(category) => {
            body.append(&label(&category.title(), "title-2"));
            body.append(&label(
                &gettext(match category {
                    ProblemCategory::Authentication => {
                        "Reconnect this account to allow synchronization to continue."
                    }
                    ProblemCategory::Permission => {
                        "Check your access to this item and grant the required permission."
                    }
                    ProblemCategory::Storage => {
                        "Free up space on the affected storage before continuing."
                    }
                    ProblemCategory::LocalFolder => {
                        "Check that the local folder is accessible or choose it again."
                    }
                }),
                "body",
            ));
            Some(category.action())
        }
    };
    toolbar.set_content(Some(
        &gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&adw::Clamp::builder().maximum_size(820).child(&body).build())
            .build(),
    ));
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    margins(&footer, 18);
    footer.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label(&gettext("Cancel"));
    cancel.set_widget_name("review-cancel");
    let weak = nav.downgrade();
    cancel.connect_clicked(move |_| {
        if let Some(nav) = weak.upgrade() {
            nav.pop();
        }
    });
    footer.append(&cancel);
    let confirm = gtk::Button::with_label(
        &action
            .as_ref()
            .map(|(_, label)| label.clone())
            .unwrap_or_else(|| gettext("Confirm")),
    );
    confirm.set_widget_name("review-confirm");
    confirm.add_css_class("suggested-action");
    let id = item.id.clone();
    let weak = nav.downgrade();
    let submitted = Cell::new(false);
    confirm.connect_clicked(move |_| {
        if revision.get() != expected || submitted.replace(true) {
            return;
        }
        let Some(nav) = weak.upgrade() else {
            return;
        };
        if nav
            .visible_page()
            .is_none_or(|p| p.tag().as_deref() != Some("alert-review"))
        {
            return;
        }
        nav.pop();
        callback(
            &id,
            action.as_ref().map(|(id, _)| *id).unwrap_or(selected.get()),
        );
    });
    footer.append(&confirm);
    toolbar.add_bottom_bar(&footer);
    let page = adw::NavigationPage::builder()
        .title(&title)
        .tag("alert-review")
        .child(&toolbar)
        .build();
    nav.push(&page);
}
