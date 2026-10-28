//! GTK4/Libadwaita first-run presentation.
//!
//! Constructs the welcome page and its reusable presentation widgets, while
//! Browser authorization is presented by the shared connection module.
use adw::{gtk, prelude::*};

/// First-run content and the entry point to the shared connection window.
pub(super) struct WelcomeView {
    pub(super) toolbar: adw::ToolbarView,
    pub(super) connect: gtk::Button,
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
pub(super) fn build() -> WelcomeView {
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.pack_end(&crate::main_menu::button());
    toolbar.add_top_bar(&header);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 24);
    content.set_valign(gtk::Align::Center);
    content.set_vexpand(true);
    content.set_margin_top(32);
    content.set_margin_bottom(32);
    content.set_margin_start(24);
    content.set_margin_end(24);

    // Embed the existing artwork; uninstalled Builder runs use the Devel profile.
    let bytes = gtk::glib::Bytes::from_static(include_bytes!(
        "../../data/icons/hicolor/scalable/apps/com.rtosta.DriveSync.svg"
    ));
    let stream = gtk::gio::MemoryInputStream::from_bytes(&bytes);
    let image = match gtk::gdk_pixbuf::Pixbuf::from_stream(&stream, None::<&gtk::gio::Cancellable>)
    {
        Ok(pixbuf) => gtk::Image::from_paintable(Some(&gtk::gdk::Texture::for_pixbuf(&pixbuf))),
        Err(error) => {
            gtk::glib::g_warning!(
                "drivesync",
                "Could not load welcome SVG: {error}. Install the production Flatpak before \
                 running it; use the Devel manifest for uninstalled Builder runs."
            );
            gtk::Image::from_icon_name("folder-download-symbolic")
        }
    };
    image.set_pixel_size(112);
    image.set_halign(gtk::Align::Center);
    image.update_property(&[gtk::accessible::Property::Label(
        "DriveSync application icon",
    )]);
    content.append(&image);
    content.append(&text("Welcome to DriveSync", "title-1"));
    let description = text("Your Google Drive, a little closer.", "body");
    description.set_max_width_chars(48);
    description.set_halign(gtk::Align::Center);
    content.append(&description);
    content.append(&text("Download only · Technical preview", "caption"));

    let actions = gtk::Box::new(gtk::Orientation::Vertical, 12);
    actions.set_halign(gtk::Align::Center);
    let connect = gtk::Button::with_label("Connect Google Account");
    connect.add_css_class("suggested-action");
    connect.add_css_class("pill");
    connect.set_tooltip_text(Some("Sign in securely in your web browser"));
    actions.append(&connect);
    content.append(&actions);

    let footnote = text(
        "A calmer place for the files that matter to you.",
        "dim-label",
    );
    footnote.set_max_width_chars(80);
    footnote.set_halign(gtk::Align::Center);
    content.append(&footnote);
    let version = text(crate::build_info::VERSION, "caption");
    version.add_css_class("dim-label");
    version.set_selectable(true);
    version.set_tooltip_text(Some(crate::build_info::REVISION));
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
