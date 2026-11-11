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
fn content(title: &str, description: &str, icon: &str) -> gtk::Box {
    let root = crate::ui::content();
    let image = gtk::Image::from_icon_name(icon);
    image.set_pixel_size(72);
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
