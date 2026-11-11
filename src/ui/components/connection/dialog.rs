//! Compact native connection presentation independent of authentication.
use adw::{gtk, prelude::*};
use gettextrs::gettext;

/// Controls owned by one browser authorization window.
pub(super) struct ConnectionView {
    window: adw::Window,
    connect: gtk::Button,
    copy: gtk::Button,
    subtitle: gtk::Label,
    spinner: gtk::Spinner,
    feedback: gtk::Label,
}

impl ConnectionView {
    pub(super) fn window(&self) -> &adw::Window {
        &self.window
    }
    pub(super) fn on_connect(&self, callback: impl Fn() + 'static) {
        self.connect.connect_clicked(move |_| callback());
    }
    pub(super) fn on_copy(&self, callback: impl Fn() + 'static) {
        self.copy.connect_clicked(move |_| callback());
    }
    pub(super) fn on_close(&self, callback: impl Fn() + 'static) {
        self.window.connect_close_request(move |_| {
            callback();
            gtk::glib::Propagation::Proceed
        });
    }
    pub(super) fn handle(&self) -> ConnectionHandle {
        ConnectionHandle {
            window: self.window.downgrade(),
            connect: self.connect.downgrade(),
            copy: self.copy.downgrade(),
            subtitle: self.subtitle.downgrade(),
            spinner: self.spinner.downgrade(),
            feedback: self.feedback.downgrade(),
        }
    }
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

/// Semantic presentation handle. Redesigns keep this contract and change only
/// this module; controllers neither know nor retain the component's widgets.
#[derive(Clone)]
pub(super) struct ConnectionHandle {
    window: gtk::glib::WeakRef<adw::Window>,
    connect: gtk::glib::WeakRef<gtk::Button>,
    copy: gtk::glib::WeakRef<gtk::Button>,
    subtitle: gtk::glib::WeakRef<gtk::Label>,
    spinner: gtk::glib::WeakRef<gtk::Spinner>,
    feedback: gtk::glib::WeakRef<gtk::Label>,
}
impl ConnectionHandle {
    pub fn set_waiting(&self, waiting: bool) {
        if let (Some(connect), Some(subtitle), Some(spinner), Some(feedback)) = (
            self.connect.upgrade(),
            self.subtitle.upgrade(),
            self.spinner.upgrade(),
            self.feedback.upgrade(),
        ) {
            set_waiting(&connect, &subtitle, &spinner, &feedback, waiting);
        }
    }
    pub fn set_connect_enabled(&self, enabled: bool) {
        if let Some(button) = self.connect.upgrade() {
            button.set_sensitive(enabled);
        }
    }
    pub fn set_copy_enabled(&self, enabled: bool) {
        if let Some(button) = self.copy.upgrade() {
            button.set_sensitive(enabled);
        }
    }
    pub fn show_feedback(&self, text: &str) {
        if let Some(label) = self.feedback.upgrade() {
            label.set_label(text);
            label.set_visible(true);
        }
    }
    pub fn copy_url(&self, url: &str) {
        if let Some(window) = self.window.upgrade() {
            window.clipboard().set_text(url);
        }
    }
    pub fn close(&self) {
        if let Some(window) = self.window.upgrade() {
            window.close();
        }
    }
}

/// Updates shared labels and progress without retaining the dialog window.
fn set_waiting(
    connect: &gtk::Button,
    subtitle: &gtk::Label,
    spinner: &gtk::Spinner,
    feedback: &gtk::Label,
    waiting: bool,
) {
    connect.set_label(&if waiting {
        gettext("Cancel")
    } else {
        gettext("Sign In…")
    });
    connect.set_sensitive(true);
    subtitle.set_label(&if waiting {
        gettext("Complete authorization in your web browser, then return here.")
    } else {
        gettext("Sign in to Google using your web browser.")
    });
    spinner.set_opacity(if waiting { 1.0 } else { 0.0 });
    spinner.set_spinning(waiting);
    feedback.set_visible(false);
}

/// Fits the normal content naturally; scrolling remains for small displays.
pub(super) fn build(parent: Option<&gtk::Window>) -> ConnectionView {
    let window = adw::Window::builder()
        .title(gettext("Google Account"))
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
            .label(gettext("Google Account"))
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
    let connect = gtk::Button::with_label(&gettext("Sign In…"));
    connect.add_css_class("suggested-action");
    connect.add_css_class("pill");
    content.append(&connect);
    let copy = gtk::Button::builder()
        .label(gettext("Copy Link"))
        .sensitive(false)
        .tooltip_text(gettext("Available after starting sign-in"))
        .build();
    copy.add_css_class("pill");
    content.append(&copy);
    let help = gtk::Label::builder().label(gettext("Copy the authorization link to continue in a different browser after starting sign-in."))
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
