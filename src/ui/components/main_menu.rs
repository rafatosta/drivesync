//! Shared application menu, independent of the selected account.
use adw::{gtk, prelude::*};
use gettextrs::gettext;

/// Creates the native application menu; account actions live in Preferences.
pub(crate) fn button() -> gtk::MenuButton {
    let model = gtk::gio::Menu::new();
    let application = gtk::gio::Menu::new();
    application.append(Some(&gettext("Add Account")), Some("connection.open"));
    application.append(Some(&gettext("Preferences")), Some("app.preferences"));
    application.append(Some(&gettext("About DriveSync")), Some("app.about"));
    model.append_section(None, &application);
    let exit = gtk::gio::Menu::new();
    exit.append(Some(&gettext("Quit")), Some("app.quit"));
    model.append_section(None, &exit);
    gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text(gettext("Main Menu"))
        .menu_model(&model)
        .build()
}

/// Creates the Preferences-only shortcut for the `connection.open` action.
pub(crate) fn add_account_button() -> gtk::Button {
    gtk::Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text(gettext("Add Account"))
        .action_name("connection.open")
        .build()
}

/// Installs the native About window action.
pub(crate) fn install(app: &adw::Application, window: &adw::ApplicationWindow) {
    let action = gtk::gio::SimpleAction::new("about", None);
    let weak_window = window.downgrade();
    action.connect_activate(move |_, _| {
        if let Some(window) = weak_window.upgrade() {
            let about = adw::AboutWindow::builder()
                .transient_for(&window)
                .modal(true)
                .application_name("DriveSync")
                .application_icon(crate::application::APP_ID)
                .version(crate::core::build_info::VERSION)
                .debug_info(format!(
                    "Source revision: {}",
                    crate::core::build_info::REVISION
                ))
                .comments(gettext("Connect and manage your Google accounts securely."))
                .build();
            about.present();
        }
    });
    app.add_action(&action);
}
