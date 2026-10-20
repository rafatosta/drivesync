use adw::prelude::*;
use drivesync_engine::DriveSyncEngine;

const APP_ID: &str = "com.rtosta.DriveSync.Devel";

fn main() -> adw::gtk::glib::ExitCode {
    let engine = DriveSyncEngine::new();
    let _initial_status = engine.status();
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(|app| {
        let label = adw::gtk::Label::builder()
            .label("DriveSync")
            .css_classes(["title-1"])
            .halign(adw::gtk::Align::Center)
            .valign(adw::gtk::Align::Center)
            .build();
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("DriveSync")
            .default_width(560)
            .default_height(400)
            .build();
        window.set_content(Some(&label));
        window.present();
    });
    app.run()
}
