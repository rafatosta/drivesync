//! Compact native connection presentation independent of authentication.
use adw::{gtk, prelude::*};

/// Controls owned by one browser authorization window.
pub(super) struct ConnectionView {
    pub(super) window: adw::Window,
    pub(super) connect: gtk::Button,
    pub(super) copy: gtk::Button,
    pub(super) subtitle: gtk::Label,
    pub(super) spinner: gtk::Spinner,
    pub(super) feedback: gtk::Label,
}

impl ConnectionView {
    /// Changes presentation only; the controller owns authorization/cancellation.
    pub(super) fn set_waiting(&self, waiting: bool) {
        set_waiting(
            &self.connect,
            &self.subtitle,
            &self.spinner,
            &self.feedback,
            waiting,
        );
    }
}

/// Updates shared labels and progress without retaining the dialog window.
pub(super) fn set_waiting(
    connect: &gtk::Button,
    subtitle: &gtk::Label,
    spinner: &gtk::Spinner,
    feedback: &gtk::Label,
    waiting: bool,
) {
    connect.set_label(if waiting { "Cancel" } else { "Sign In…" });
    connect.set_sensitive(true);
    subtitle.set_label(if waiting {
        "Complete authorization in your web browser, then return here."
    } else {
        "Sign in to Google using your web browser."
    });
    spinner.set_opacity(if waiting { 1.0 } else { 0.0 });
    spinner.set_spinning(waiting);
    feedback.set_visible(false);
}

/// Fits the normal content naturally; scrolling remains for small displays.
pub(super) fn build(parent: Option<&gtk::Window>) -> ConnectionView {
    let window = adw::Window::builder()
        .title("Google Account")
        .modal(true)
        .default_width(420)
        .width_request(300)
        .height_request(300)
        .resizable(false)
        .build();
    window.set_widget_name("google-account-connection");
    window.set_transient_for(parent);
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&gtk::Label::new(None)));
    toolbar.add_top_bar(&header);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
    for (setter, value) in [
        ("margin-top", 12),
        ("margin-bottom", 24),
        ("margin-start", 24),
        ("margin-end", 24),
    ] {
        content.set_property(setter, value);
    }
    let image = gtk::Image::from_icon_name("network-workgroup-symbolic");
    image.set_pixel_size(80);
    image.add_css_class("dim-label");
    image.set_margin_bottom(8);
    content.append(&image);
    content.append(
        &gtk::Label::builder()
            .label("Google Account")
            .css_classes(["title-1"])
            .wrap(true)
            .justify(gtk::Justification::Center)
            .build(),
    );
    let subtitle = gtk::Label::builder()
        .wrap(true)
        .justify(gtk::Justification::Center)
        .build();
    subtitle.set_widget_name("connection-subtitle");
    content.append(&subtitle);
    let spinner = gtk::Spinner::builder()
        .halign(gtk::Align::Center)
        .opacity(0.0)
        .build();
    spinner.set_widget_name("connection-spinner");
    content.append(&spinner);
    let connect = gtk::Button::with_label("Sign In…");
    connect.add_css_class("suggested-action");
    connect.add_css_class("pill");
    content.append(&connect);
    let copy = gtk::Button::builder()
        .label("Copy Link")
        .sensitive(false)
        .tooltip_text("Available after starting sign-in")
        .build();
    copy.add_css_class("pill");
    content.append(&copy);
    let help = gtk::Label::builder().label("Copy the authorization link to continue in a different browser after starting sign-in.")
        .wrap(true).justify(gtk::Justification::Center).css_classes(["dim-label"]).build();
    content.append(&help);
    let feedback = gtk::Label::builder()
        .wrap(true)
        .selectable(true)
        .justify(gtk::Justification::Center)
        .visible(false)
        .build();
    content.append(&feedback);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(560)
        .child(&content)
        .build();
    toolbar.set_content(Some(&scroll));
    window.set_content(Some(&toolbar));
    let view = ConnectionView {
        window,
        connect,
        copy,
        subtitle,
        spinner,
        feedback,
    };
    view.set_waiting(false);
    view
}
