//! Reusable Adwaita/GTK presentation primitives for account screens.
//!
//! Layout helpers have no direct access to the DriveSync engine or storage.
use adw::{gtk, prelude::*};

/// Creates a non-markup action row with a leading icon and selectable subtitle.
pub fn row(title: &str, subtitle: &str, icon: &str) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .use_markup(false)
        .subtitle_selectable(true)
        .build();
    row.add_prefix(&gtk::Image::from_icon_name(icon));
    row
}
/// Wraps a widget in a horizontally constrained scrollable container.
///
/// Horizontal scrolling is disabled; `maximum` controls the Adwaita clamp.
pub fn scroll(content: &impl IsA<gtk::Widget>, maximum: i32) -> gtk::ScrolledWindow {
    let clamp = adw::Clamp::builder()
        .maximum_size(maximum)
        .child(content)
        .build();
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&clamp)
        .build()
}
/// Creates a vertical content container with consistent 24px margins.
pub fn content() -> gtk::Box {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 24);
    content.set_margin_start(24);
    content.set_margin_end(24);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content
}
