//! Pages compose setup components and expose semantic interaction events.
mod connection;
mod provider;
mod review;
mod synchronization;
use adw::{gtk, prelude::*};
pub(super) use connection::ConnectionPage;
pub(super) use provider::ProviderPage;
pub(super) use review::ReviewPage;
pub(super) use synchronization::SynchronizationPage;
/// Shared page composition, independent of the dialog navigation.
fn content(title: &str, description: &str, icon_svg: &str) -> gtk::Box {
    let root = crate::ui::content();
    root.set_spacing(12);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    let image = crate::ui::components::svg_icon(icon_svg, 80);
    root.append(&image);
    root.append(
        &gtk::Label::builder()
            .label(title)
            .wrap(true)
            .justify(gtk::Justification::Center)
            .css_classes(["title-1"])
            .build(),
    );
    root.append(
        &gtk::Label::builder()
            .label(description)
            .wrap(true)
            .justify(gtk::Justification::Center)
            .build(),
    );
    root
}
