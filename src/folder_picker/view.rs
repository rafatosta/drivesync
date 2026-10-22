//! GTK/Libadwaita widgets used by the Drive folder picker.
//!
//! This module constructs the dialog and controls without fetching Drive data.
use adw::{gtk, prelude::*};

/// Widget handles used by the browsing controller.
pub(super) struct PickerView {
    pub(super) window: adw::Window,
    pub(super) list: gtk::Box,
    pub(super) status: gtk::Label,
    pub(super) choose: gtk::Button,
    pub(super) up: gtk::Button,
    pub(super) more: gtk::Button,
    pub(super) retry: gtk::Button,
}

/// Constructs an unpresented modal folder-picker dialog.
pub(super) fn build(parent: Option<&gtk::Window>) -> PickerView {
    let window = adw::Window::builder()
        .title("Choose Google Drive Folder")
        .modal(true)
        .default_width(640)
        .default_height(600)
        .build();
    window.set_transient_for(parent);
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    let content = crate::flow::content();
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let up = gtk::Button::with_label("Up");
    let retry = gtk::Button::with_label("Reload");
    controls.append(&up);
    controls.append(&retry);
    content.append(&controls);
    let status = gtk::Label::builder().wrap(true).xalign(0.0).build();
    content.append(&status);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 8);
    content.append(&list);
    let more = gtk::Button::with_label("Load More Folders");
    more.set_visible(false);
    content.append(&more);
    let choose = gtk::Button::with_label("Select This Folder");
    choose.add_css_class("suggested-action");
    content.append(&choose);
    toolbar.set_content(Some(&crate::flow::scroll(&content, 620)));
    window.set_content(Some(&toolbar));

    PickerView {
        window,
        list,
        status,
        choose,
        up,
        more,
        retry,
    }
}
