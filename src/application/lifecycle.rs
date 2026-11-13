//! Native application startup and shutdown, independent of sync mechanisms.
use super::{background, main_menu, restoration, Context};
use crate::{core::i18n, ui::preferences};
use adw::prelude::*;
use drivesync_engine::{CancelGoogleSignInRequest, EngineConfig};
use std::path::PathBuf;
/// Each installed profile has its own native application identity.
pub(crate) const APP_ID: &str = match option_env!("DRIVESYNC_APP_ID") {
    Some(id) => id,
    None => "com.rtosta.DriveSync.Devel",
};
fn config() -> EngineConfig {
    let bundled = PathBuf::from("/app/share/drivesync/oauth.json");
    let development = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/oauth/development.json");
    let google_oauth_file = std::env::var_os("DRIVESYNC_GOOGLE_OAUTH_FILE")
        .map(PathBuf::from)
        .or_else(|| {
            if bundled.is_file() {
                Some(bundled)
            } else if APP_ID.ends_with(".Devel") && development.is_file() {
                Some(development)
            } else {
                None
            }
        });
    EngineConfig {
        data_directory: adw::gtk::glib::user_data_dir()
            .join("drivesync")
            .join("accounts"),
        application_id: APP_ID.into(),
        google_oauth_file,
    }
}
/// Launches the account interface. All disk/network engine operations are async.
pub(crate) fn run() -> adw::gtk::glib::ExitCode {
    i18n::init();
    let app = adw::Application::builder().application_id(APP_ID).build();
    background::configure_application(&app);
    app.connect_startup(|app| {
        let navigation = adw::NavigationView::new();
        let toasts = adw::ToastOverlay::new();
        toasts.set_child(Some(&navigation));
        let context = Context::default();
        navigation.add(&restoration::build(
            config(),
            &context,
            &navigation,
            &toasts,
        ));
        let window = crate::ui::dashboard::window::build(app, &toasts);
        crate::ui::developer::install(&window, &context);
        let shutdown_context = context.clone();
        app.connect_shutdown(move |_| {
            super::detectors::shutdown(&shutdown_context);
            if let (Some(engine), Some(id)) = (
                shutdown_context.engine.borrow().as_ref(),
                shutdown_context.pending.borrow_mut().take(),
            ) {
                engine.cancel_google_sign_in(CancelGoogleSignInRequest { connection_id: id });
            }
        });
        let quit = adw::gtk::gio::SimpleAction::new("quit", None);
        let weak = app.downgrade();
        let weak_window = window.downgrade();
        quit.connect_activate(move |_, _| {
            if let Some(window) = weak_window.upgrade() {
                if window.widget_name() == "drivesync-account-setup" {
                    window.close();
                    return;
                }
            }
            if let Some(app) = weak.upgrade() {
                if let Some(setup) = app
                    .windows()
                    .into_iter()
                    .find(|window| window.widget_name() == "drivesync-account-setup")
                {
                    let weak = app.downgrade();
                    setup.connect_destroy(move |_| {
                        if let Some(app) = weak.upgrade() {
                            app.quit();
                        }
                    });
                    setup.close();
                } else {
                    app.quit();
                }
            }
        });
        app.add_action(&quit);
        background::install(app, &window);
        background::install_startup(app, &window, &toasts);
        main_menu::install(app, &window);
        preferences::install(app, &navigation, &toasts, &context);
        app.set_accels_for_action("app.quit", &["<Primary>q"]);
    });
    app.run()
}
