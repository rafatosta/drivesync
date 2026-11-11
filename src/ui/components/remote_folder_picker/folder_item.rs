//! Recycled grid tiles with theme-aware selection and confirmation indicator.
use super::model::RemoteFolder;
use adw::{gtk, prelude::*};
pub(super) fn factory() -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, object| {
        let item = object.downcast_ref::<gtk::ListItem>().unwrap();
        let overlay = gtk::Overlay::new();
        let tile = gtk::Box::new(gtk::Orientation::Vertical, 8);
        tile.add_css_class("remote-folder-tile");
        let icon = folder_icon(56);
        tile.append(&icon);
        let label = gtk::Label::builder()
            .wrap(true)
            .justify(gtk::Justification::Center)
            .build();
        tile.append(&label);
        overlay.set_child(Some(&tile));
        let check = gtk::Image::from_icon_name("object-select-symbolic");
        check.set_halign(gtk::Align::End);
        check.set_valign(gtk::Align::Start);
        check.set_margin_top(14);
        check.set_margin_end(14);
        check.add_css_class("remote-folder-check");
        overlay.add_overlay(&check);
        item.set_child(Some(&overlay));
        item.connect_selected_notify(move |item| {
            check.set_opacity(if item.is_selected() { 1.0 } else { 0.0 });
        });
    });
    factory.connect_bind(|_, object| {
        let item = object.downcast_ref::<gtk::ListItem>().unwrap();
        let value = item
            .item()
            .unwrap()
            .downcast::<gtk::glib::BoxedAnyObject>()
            .unwrap();
        let folder = value.borrow::<RemoteFolder>();
        let overlay = item.child().unwrap().downcast::<gtk::Overlay>().unwrap();
        let tile = overlay.child().unwrap().downcast::<gtk::Box>().unwrap();
        tile.last_child()
            .unwrap()
            .downcast::<gtk::Label>()
            .unwrap()
            .set_label(&folder.name);
        overlay
            .last_child()
            .unwrap()
            .set_opacity(if item.is_selected() { 1.0 } else { 0.0 });
    });
    factory
}

/// Bundled artwork keeps folder tiles consistent across system icon themes.
pub(super) fn folder_icon(size: i32) -> gtk::Image {
    let bytes = gtk::glib::Bytes::from_static(include_bytes!("folder.png"));
    let texture =
        gtk::gdk::Texture::from_bytes(&bytes).expect("bundled folder artwork is valid PNG");
    let image = gtk::Image::from_paintable(Some(&texture));
    image.set_pixel_size(size);
    image
}
