//! Native account-management application entry point.
mod app;
mod core;
mod features;
mod ui;
fn main() -> adw::gtk::glib::ExitCode {
    app::run()
}
