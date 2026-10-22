//! Compact startup view for restoring previously saved Google accounts.
//!
//! Presentation widgets are independent of credential verification and
//! dashboard routing, which run in the controller.
use adw::{gtk, prelude::*};

/// Handles for the status and controls of the startup restoration panel.
pub(super) struct RestorationView {
    pub(super) panel: gtk::Box,
    pub(super) status: gtk::Label,
    pub(super) spinner: gtk::Spinner,
    pub(super) retry: gtk::Button,
    pub(super) open: gtk::Button,
}

/// Creates a restoration-status panel without starting any I/O.
pub(super) fn build() -> RestorationView {
    let panel = gtk::Box::new(gtk::Orientation::Vertical, 8);
    panel.set_halign(gtk::Align::Center);
    let status = gtk::Label::builder()
        .label("Checking your connection…")
        .wrap(true)
        .max_width_chars(48)
        .build();
    let spinner = gtk::Spinner::new();
    let retry = gtk::Button::with_label("Try Again");
    retry.set_visible(false);
    let open = gtk::Button::with_label("Open Dashboard");
    open.set_visible(false);
    open.add_css_class("flat");
    panel.append(&spinner);
    panel.append(&status);
    panel.append(&retry);
    panel.append(&open);

    RestorationView {
        panel,
        status,
        spinner,
        retry,
        open,
    }
}
