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
/// Shared content margins and rhythm for setup steps after provider selection.
fn content() -> gtk::Box {
    let root = crate::ui::content();
    root.set_spacing(12);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    root
}

/// Keeps artwork, title and description anchored at the same positions in each step.
struct PageHeader {
    root: gtk::Box,
    title: gtk::Label,
}
impl PageHeader {
    fn new(name: &str, artwork: &impl IsA<gtk::Widget>, title: &str, description: &str) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let slot = gtk::Overlay::new();
        slot.set_widget_name(&format!("setup-{name}-artwork"));
        let space = gtk::Box::new(gtk::Orientation::Vertical, 0);
        space.set_height_request(184);
        slot.set_child(Some(&space));
        artwork.set_halign(gtk::Align::Center);
        artwork.set_valign(gtk::Align::Center);
        slot.add_overlay(artwork);
        slot.set_clip_overlay(artwork, true);
        root.append(&slot);
        let title = gtk::Label::builder()
            .label(title)
            .wrap(true)
            .justify(gtk::Justification::Center)
            .css_classes(["title-1"])
            .build();
        title.set_widget_name(&format!("setup-{name}-title"));
        root.append(&title);
        let description_slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
        description_slot.set_height_request(40);
        description_slot.append(
            &gtk::Label::builder()
                .label(description)
                .wrap(true)
                .justify(gtk::Justification::Center)
                .halign(gtk::Align::Center)
                .max_width_chars(60)
                .build(),
        );
        root.append(&description_slot);
        Self { root, title }
    }
    fn widget(&self) -> &gtk::Box {
        &self.root
    }
    fn title(&self) -> &gtk::Label {
        &self.title
    }
}
