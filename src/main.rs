//! DriveSync binary entry point; application composition lives in `app`.
mod app;
mod core;
mod features;
mod ui;

fn main() -> adw::gtk::glib::ExitCode {
    app::run()
}
