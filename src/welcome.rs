//! First-run presentation only. Account operations go through the engine facade.
use adw::{gtk, prelude::*};
use drivesync_engine::{DriveSyncEngine, EngineError};
use std::rc::Rc;

fn text(value: &str, class: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(value)
        .wrap(true)
        .justify(gtk::Justification::Center)
        .halign(gtk::Align::Center)
        .css_classes([class])
        .build()
}

pub fn build(engine: Rc<DriveSyncEngine>) -> adw::ToolbarView {
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    let menu = gtk::gio::Menu::new();
    menu.append(Some("Quit"), Some("app.quit"));
    header.pack_end(
        &gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .tooltip_text("Main Menu")
            .menu_model(&menu)
            .build(),
    );
    toolbar.add_top_bar(&header);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 24);
    content.set_valign(gtk::Align::Center);
    content.set_vexpand(true);
    content.set_margin_top(32);
    content.set_margin_bottom(32);
    content.set_margin_start(24);
    content.set_margin_end(24);

    // Embed the existing project artwork so it also works from an uninstalled checkout.
    let bytes = gtk::glib::Bytes::from_static(include_bytes!(
        "../data/icons/hicolor/scalable/apps/com.rtosta.DriveSync.Devel.svg"
    ));
    let stream = gtk::gio::MemoryInputStream::from_bytes(&bytes);
    let image = match gtk::gdk_pixbuf::Pixbuf::from_stream(&stream, None::<&gtk::gio::Cancellable>)
    {
        Ok(pixbuf) => gtk::Image::from_paintable(Some(&gtk::gdk::Texture::for_pixbuf(&pixbuf))),
        // Restricted environments may not provide an SVG loader/portal.
        Err(_) => gtk::Image::from_icon_name("folder-download-symbolic"),
    };
    image.set_pixel_size(144);
    image.set_halign(gtk::Align::Center);
    image.update_property(&[gtk::accessible::Property::Label(
        "DriveSync folder and cloud",
    )]);
    content.append(&image);
    content.append(&text("Welcome to DriveSync", "title-1"));
    let description = text(
        "Keep selected Google Drive folders available on your Linux computer. The first milestone brings safe, download-only sync, with support for multiple accounts.",
        "body",
    );
    description.set_max_width_chars(62);
    description.set_halign(gtk::Align::Center);
    content.append(&description);
    content.append(&text("Download only · Technical preview", "caption"));

    let actions = gtk::Box::new(gtk::Orientation::Vertical, 12);
    actions.set_halign(gtk::Align::Center);
    let connect = gtk::Button::with_label("Connect Google Account");
    connect.add_css_class("suggested-action");
    connect.add_css_class("pill");
    connect.set_tooltip_text(Some("Check account connection availability"));
    actions.append(&connect);

    let feedback = text("", "body");
    feedback.set_max_width_chars(60);
    feedback.set_selectable(true);
    feedback.set_visible(false);
    let feedback_clone = feedback.clone();
    connect.connect_clicked(move |button| {
        // No credentials, browser URL or synthetic account is created in the UI.
        match engine.add_account() {
            Err(EngineError::NotImplemented(_)) => {
                feedback_clone.set_label("Account connection is not available in this technical preview. OAuth browser sign-in still needs to be implemented. No account has been connected.");
                button.set_label("Account Connection Unavailable");
                button.set_sensitive(false);
            }
            Ok(_) => {
                feedback_clone.set_label("Account added. Folder setup is not available in this preview.");
                button.set_sensitive(false);
            }
        }
        feedback_clone.set_visible(true);
    });
    let learn = gtk::ToggleButton::with_label("Learn More");
    learn.add_css_class("flat");
    actions.append(&learn);
    content.append(&actions);
    content.append(&feedback);

    let details = gtk::Revealer::new();
    let explanation = text(
        "DriveSync is in early development: sign-in and file transfers are not functional yet. The MVP will download files from Google Drive to local folders. Local edits will not be uploaded, and remote deletions will not automatically delete local files. Google Workspace documents are outside this milestone.",
        "body",
    );
    explanation.set_max_width_chars(70);
    explanation.set_selectable(true);
    details.set_child(Some(&explanation));
    let details_clone = details.clone();
    learn.connect_toggled(move |button| details_clone.set_reveal_child(button.is_active()));
    content.append(&details);

    let note = text(
        "Browser OAuth · When available, sign-in will open securely in your web browser. DriveSync will never ask for your Google password here.",
        "dim-label",
    );
    note.set_max_width_chars(80);
    note.set_halign(gtk::Align::Center);
    content.append(&note);

    let clamp = adw::Clamp::builder()
        .maximum_size(1000)
        .vexpand(true)
        .child(&content)
        .build();
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&clamp)
        .build();
    toolbar.set_content(Some(&scroll));
    toolbar
}
