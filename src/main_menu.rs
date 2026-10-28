//! Shared application menu, independent of the selected account.
use adw::{gtk, prelude::*};

/// Creates the native primary menu. Account connection resolves through the
/// containing navigation view; application actions remain available on all pages.
pub(crate) fn button() -> gtk::MenuButton {
    let model = gtk::gio::Menu::new();
    let accounts = gtk::gio::Menu::new();
    accounts.append(Some("Add Account…"), Some("connection.open"));
    model.append_section(None, &accounts);
    let application = gtk::gio::Menu::new();
    application.append(Some("About DriveSync"), Some("app.about"));
    model.append_section(None, &application);
    let exit = gtk::gio::Menu::new();
    exit.append(Some("Quit"), Some("app.quit"));
    model.append_section(None, &exit);
    gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text("Main Menu")
        .menu_model(&model)
        .build()
}

/// Installs the native About window action without adding preferences or settings.
pub(crate) fn install(app: &adw::Application, window: &adw::ApplicationWindow) {
    let action = gtk::gio::SimpleAction::new("about", None);
    let weak_window = window.downgrade();
    action.connect_activate(move |_, _| {
        if let Some(window) = weak_window.upgrade() {
            let about = adw::AboutWindow::builder()
                .transient_for(&window)
                .modal(true)
                .application_name("DriveSync")
                .application_icon(crate::APP_ID)
                .version(crate::build_info::VERSION)
                .debug_info(format!("Source revision: {}", crate::build_info::REVISION))
                .comments("Download files from Google Drive. Local edits are preserved.")
                .build();
            about.present();
        }
    });
    app.add_action(&action);
}
