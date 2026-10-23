//! GTK application lifecycle and initial DriveSync window.
//!
//! Initializes the engine, account restoration, shutdown cancellation and
//! application actions. The main thread never performs blocking Drive work.
use adw::prelude::*;
use drivesync_engine::DriveSyncEngine;
use std::{cell::RefCell, rc::Rc, sync::Arc};

mod background;
mod connection;
mod dashboard;
mod flow;
mod folder_picker;
mod restoration;
mod session;
mod welcome;

/// The build profile selects the installed Flatpak application identity.
/// Development remains the fallback for plain local Cargo builds.
const APP_ID: &str = match option_env!("DRIVESYNC_APP_ID") {
    Some(id) => id,
    None => "com.rtosta.DriveSync.Devel",
};

/// Launches the single-instance GTK application.
fn main() -> adw::gtk::glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    // Startup runs once for the primary application, even on repeated activation.
    app.connect_startup(|app| {
        let engine = Arc::new(DriveSyncEngine::new());
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("DriveSync")
            .default_width(960)
            .default_height(800)
            .width_request(360)
            .height_request(360)
            .build();
        let navigation = adw::NavigationView::new();
        let toasts = adw::ToastOverlay::new();
        toasts.set_child(Some(&navigation));
        let cancellation = Rc::new(RefCell::new(None::<drivesync_engine::Cancellation>));
        let startup = restoration::build(engine, &navigation, &toasts, cancellation.clone());
        navigation.add(&startup);
        window.set_content(Some(&toasts));
        app.connect_shutdown(move |_| {
            if let Some(cancel) = cancellation.borrow().as_ref() {
                cancel.cancel();
            }
        });
        let quit = adw::gtk::gio::SimpleAction::new("quit", None);
        let weak_app = app.downgrade();
        quit.connect_activate(move |_, _| {
            if let Some(app) = weak_app.upgrade() {
                app.quit();
            }
        });
        app.add_action(&quit);
        background::install(app, &window);
        app.set_accels_for_action("app.quit", &["<Primary>q"]);
    });
    app.run()
}
