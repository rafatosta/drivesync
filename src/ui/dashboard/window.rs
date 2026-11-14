//! Main application window composition, independent of startup operations.
use adw::{gtk, prelude::*};
/// Builds the main window and mounts its presentation root.
pub(crate) fn build(
    app: &adw::Application,
    content: &impl IsA<gtk::Widget>,
) -> adw::ApplicationWindow {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("DriveSync")
        .default_width(960)
        .default_height(720)
        .width_request(360)
        .height_request(360)
        .build();
    window.set_content(Some(content));
    window
}
