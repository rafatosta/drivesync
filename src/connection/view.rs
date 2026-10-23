//! Native modal presentation independent of authentication and stored accounts.
use adw::{gtk, prelude::*};

/// Controls owned by a single browser authorization attempt window.
pub(super) struct ConnectionView {
    pub(super) window: adw::Window,
    pub(super) connect: gtk::Button,
    pub(super) copy: gtk::Button,
    pub(super) cancel_button: gtk::Button,
    pub(super) spinner: gtk::Spinner,
    pub(super) feedback: gtk::Label,
}

/// Builds the Google account connection surface without starting OAuth.
pub(super) fn build(parent: Option<&gtk::Window>) -> ConnectionView {
    let window = adw::Window::builder()
        .title("Google Account")
        .modal(true)
        .default_width(460)
        .default_height(570)
        .width_request(300)
        .height_request(300)
        .build();
    window.set_widget_name("google-account-connection");
    window.set_transient_for(parent);
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&gtk::Label::new(None)));
    toolbar.add_top_bar(&header);
    let content = crate::flow::content();
    content.set_valign(gtk::Align::Center);
    content.set_vexpand(true);
    let status = adw::StatusPage::builder()
        .icon_name("goa-panel-symbolic")
        .title("Google Account")
        .description("Sign in to Google using your web browser.")
        .build();
    let controls = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let connect = gtk::Button::with_label("Sign In…");
    connect.add_css_class("suggested-action");
    connect.add_css_class("pill");
    let copy = gtk::Button::builder()
        .label("Copy Link")
        .sensitive(false)
        .tooltip_text("Available after starting sign-in")
        .build();
    copy.add_css_class("pill");
    let help = gtk::Label::builder().label("After starting sign-in, copy the authorization link to continue in a different browser.")
        .wrap(true).justify(gtk::Justification::Center).css_classes(["dim-label"]).build();
    let spinner = gtk::Spinner::new();
    spinner.set_visible(false);
    let feedback = gtk::Label::builder()
        .wrap(true)
        .selectable(true)
        .justify(gtk::Justification::Center)
        .visible(false)
        .build();
    let cancel_button = gtk::Button::with_label("Cancel");
    cancel_button.set_visible(false);
    controls.append(&connect);
    controls.append(&copy);
    controls.append(&help);
    controls.append(&spinner);
    controls.append(&feedback);
    controls.append(&cancel_button);
    status.set_child(Some(&controls));
    content.append(&status);
    toolbar.set_content(Some(&crate::flow::scroll(&content, 460)));
    window.set_content(Some(&toolbar));
    ConnectionView {
        window,
        connect,
        copy,
        cancel_button,
        spinner,
        feedback,
    }
}
