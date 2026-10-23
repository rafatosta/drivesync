//! Native folder list and navigation controls without Drive I/O.
use adw::{gtk, prelude::*};

pub(super) struct PickerView {
    pub(super) window: adw::Window,
    pub(super) list: gtk::ListBox,
    pub(super) breadcrumbs: gtk::Box,
    pub(super) spinner: gtk::Spinner,
    pub(super) status: gtk::Label,
    pub(super) choose: gtk::Button,
    pub(super) up: gtk::Button,
    pub(super) more: gtk::Button,
    pub(super) retry: gtk::Button,
}

/// Creates an activatable folder row using symbolic GNOME icons.
pub(super) fn folder_row(name: &str) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(name)
        .use_markup(false)
        .title_lines(1)
        .activatable(true)
        .build();
    row.set_tooltip_text(Some(name));
    row.add_prefix(&gtk::Image::from_icon_name("folder-symbolic"));
    row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
    row
}

/// Keeps the current location readable without making it an inactive button.
pub(super) fn current_location(name: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(name)
        .css_classes(["heading"])
        .margin_start(10)
        .margin_end(10)
        .build()
}

/// Builds a resizable modal with scrollable rows and always-visible actions.
pub(super) fn build(parent: Option<&gtk::Window>) -> PickerView {
    let window = adw::Window::builder()
        .title("Choose Google Drive Folder")
        .modal(true)
        .default_width(600)
        .default_height(480)
        .width_request(320)
        .height_request(300)
        .build();
    window.set_widget_name("drive-folder-picker");
    window.set_transient_for(parent);
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new(
        "Google Drive",
        "Choose a folder",
    )));
    let up = gtk::Button::from_icon_name("go-up-symbolic");
    up.set_tooltip_text(Some("Parent folder"));
    let retry = gtk::Button::from_icon_name("view-refresh-symbolic");
    retry.set_tooltip_text(Some("Reload folders"));
    header.pack_start(&up);
    header.pack_end(&retry);
    toolbar.add_top_bar(&header);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_start(18);
    content.set_margin_end(18);
    content.set_margin_top(12);
    content.set_margin_bottom(18);
    let breadcrumbs = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    breadcrumbs.set_widget_name("drive-folder-path");
    let path_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .child(&breadcrumbs)
        .build();
    content.append(&path_scroll);
    let progress = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let spinner = gtk::Spinner::new();
    spinner.set_visible(false);
    progress.append(&spinner);
    let status = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .hexpand(true)
        .build();
    progress.append(&status);
    content.append(&progress);
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    list.set_widget_name("drive-folder-list");
    content.append(&list);
    let more = gtk::Button::with_label("Load More Folders");
    more.set_visible(false);
    content.append(&more);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&content)
        .build();
    toolbar.set_content(Some(&scroll));
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    footer.set_margin_start(18);
    footer.set_margin_end(18);
    footer.set_margin_top(12);
    footer.set_margin_bottom(12);
    let cancel = gtk::Button::with_label("Cancel");
    let weak_window = window.downgrade();
    cancel.connect_clicked(move |_| {
        if let Some(window) = weak_window.upgrade() {
            window.close();
        }
    });
    footer.append(&cancel);
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    footer.append(&spacer);
    let choose = gtk::Button::with_label("Select Folder");
    choose.add_css_class("suggested-action");
    choose.set_tooltip_text(Some("Select the folder shown in the path above"));
    choose.set_sensitive(false);
    footer.append(&choose);
    toolbar.add_bottom_bar(&footer);
    window.set_content(Some(&toolbar));
    PickerView {
        window,
        list,
        breadcrumbs,
        spinner,
        status,
        choose,
        up,
        more,
        retry,
    }
}
