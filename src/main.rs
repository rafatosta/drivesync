use adw::prelude::*;
use drivesync_engine::DriveSyncEngine;
use std::{cell::RefCell, rc::Rc, sync::Arc};

mod dashboard;
mod flow;
mod folder_picker;
mod restoration;
mod session;
mod setup;
mod welcome;

const APP_ID: &str = "com.rtosta.DriveSync.Devel";

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
            .hide_on_close(true)
            .build();
        let navigation = adw::NavigationView::new();
        let toasts = adw::ToastOverlay::new();
        toasts.set_child(Some(&navigation));
        let cancellation = Rc::new(RefCell::new(None::<drivesync_engine::Cancellation>));
        let welcome = welcome::build(engine, &navigation, &toasts, cancellation.clone());
        let welcome_page = adw::NavigationPage::new(&welcome, "Welcome");
        welcome_page.set_tag(Some("welcome"));
        navigation.add(&welcome_page);
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
        app.set_accels_for_action("app.quit", &["<Primary>q"]);
    });
    app.connect_activate(|app| {
        if let Some(window) = app.active_window() {
            window.present();
        }
    });
    app.run()
}
