//! Native account-management application entry point.
mod application;
mod core;
mod services;
mod ui;
fn main() -> adw::gtk::glib::ExitCode {
    application::run()
}
