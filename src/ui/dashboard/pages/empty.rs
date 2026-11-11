//! GTK4/Libadwaita first-run presentation.
//!
//! Constructs the welcome page and its reusable presentation widgets, while
//! Browser authorization is presented by the shared connection module.
use adw::{gtk, prelude::*};
use gettextrs::gettext;

/// First-run content and the entry point to the shared connection window.
pub(crate) struct WelcomeView {
    pub(crate) toolbar: adw::ToolbarView,
    pub(crate) connect: gtk::Button,
}

/// Creates a centered, wrapping text label with the requested CSS class.
fn text(value: &str, class: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(value)
        .wrap(true)
        .justify(gtk::Justification::Center)
        .halign(gtk::Align::Center)
        .css_classes([class])
        .build()
}

/// Builds the welcome view without starting or completing Google OAuth.
///
/// Account restoration and browser authorization have separate screens.
pub(crate) fn build_view() -> WelcomeView {
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.pack_end(&crate::ui::components::main_menu::button());
    toolbar.add_top_bar(&header);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 24);
    content.set_valign(gtk::Align::Center);
    content.set_vexpand(true);
    content.set_margin_top(32);
    content.set_margin_bottom(32);
    content.set_margin_start(24);
    content.set_margin_end(24);

    // Embed a raster export of the existing SVG to avoid an external SVG loader
    // when the app runs in an isolated SDK session or without installed icons.
    let bytes = gtk::glib::Bytes::from_static(include_bytes!("../../../../data/icons/welcome.png"));
    let image = match gtk::gdk::Texture::from_bytes(&bytes) {
        Ok(texture) => gtk::Image::from_paintable(Some(&texture)),
        Err(_) => gtk::Image::from_icon_name("folder-download-symbolic"),
    };
    image.set_pixel_size(112);
    image.set_halign(gtk::Align::Center);
    image.update_property(&[gtk::accessible::Property::Label(&gettext(
        "DriveSync application icon",
    ))]);
    content.append(&image);
    content.append(&text(&gettext("Welcome to DriveSync"), "title-1"));
    let description = text(&gettext("Your Google Drive, a little closer."), "body");
    description.set_max_width_chars(48);
    description.set_halign(gtk::Align::Center);
    content.append(&description);
    content.append(&text(
        &gettext("Account management · Technical preview"),
        "caption",
    ));

    let actions = gtk::Box::new(gtk::Orientation::Vertical, 12);
    actions.set_halign(gtk::Align::Center);
    let connect = gtk::Button::with_label(&gettext("Connect Google Account"));
    connect.add_css_class("suggested-action");
    connect.add_css_class("pill");
    connect.set_tooltip_text(Some(&gettext("Sign in securely in your web browser")));
    actions.append(&connect);
    content.append(&actions);

    let footnote = text(
        &gettext("A calmer place for the files that matter to you."),
        "dim-label",
    );
    footnote.set_max_width_chars(80);
    footnote.set_halign(gtk::Align::Center);
    content.append(&footnote);
    let version = text(crate::core::build_info::VERSION, "caption");
    version.add_css_class("dim-label");
    version.set_selectable(true);
    version.set_tooltip_text(Some(crate::core::build_info::REVISION));
    content.append(&version);

    let clamp = adw::Clamp::builder()
        .maximum_size(760)
        .vexpand(true)
        .child(&content)
        .build();
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&clamp)
        .build();
    toolbar.set_content(Some(&scroll));

    WelcomeView { toolbar, connect }
}

/// Creates the empty dashboard with the add-account action.
pub(crate) fn build() -> adw::ToolbarView {
    let view = build_view();
    view.connect.set_action_name(Some("connection.open"));
    view.toolbar
}
