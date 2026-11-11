//! Provider page populated exclusively from catalog metadata.
use super::super::components::ProviderChoice;
use crate::services::setup::RegisteredProvider;
use adw::{gtk, prelude::*};
use drivesync_engine::ProviderId;
use std::{cell::Cell, rc::Rc};
pub(in crate::ui::setup) struct ProviderPage {
    root: gtk::Box,
    choices: Vec<ProviderChoice>,
    updating: Rc<Cell<bool>>,
}
impl ProviderPage {
    pub fn new(providers: &[RegisteredProvider]) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 32);
        root.set_margin_start(24);
        root.set_margin_end(24);
        root.set_margin_top(16);
        root.set_margin_bottom(8);
        let bytes =
            gtk::glib::Bytes::from_static(include_bytes!("../../../../data/icons/welcome.png"));
        let texture = gtk::gdk::Texture::from_bytes(&bytes).expect("bundled welcome image");
        let image = gtk::Picture::for_paintable(&texture);
        image.set_size_request(220, 220);
        image.set_can_shrink(true);
        image.set_valign(gtk::Align::Center);
        image.set_hexpand(true);
        root.append(&image);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_size_request(340, -1);
        content.set_valign(gtk::Align::Center);
        content.set_hexpand(true);
        content.append(
            &gtk::Label::builder()
                .label(gettextrs::gettext("Welcome to DriveSync"))
                .xalign(0.0)
                .wrap(true)
                .css_classes(["title-1"])
                .build(),
        );
        content.append(&gtk::Label::builder()
            .label(gettextrs::gettext("Connect your cloud storage accounts and keep your files synchronized on your computer."))
            .xalign(0.0).wrap(true).max_width_chars(44).build());
        content.append(
            &gtk::Label::builder()
                .label(gettextrs::gettext("Choose a provider to get started:"))
                .xalign(0.0)
                .wrap(true)
                .css_classes(["heading"])
                .margin_top(4)
                .build(),
        );
        root.append(&content);
        let mut choices = Vec::new();
        for provider in providers {
            let choice = ProviderChoice::new(provider, choices.first());
            content.append(choice.widget());
            choices.push(choice);
        }
        if choices.is_empty() {
            content.append(&gtk::Label::new(Some(&gettextrs::gettext(
                "No providers are registered.",
            ))));
        }
        Self {
            root,
            choices,
            updating: Rc::new(Cell::new(false)),
        }
    }
    pub fn widget(&self) -> &impl IsA<gtk::Widget> {
        &self.root
    }
    pub fn set_selected(&self, id: Option<&ProviderId>, enabled: bool) {
        self.root.set_sensitive(enabled);
        if self.updating.replace(true) {
            return;
        }
        for choice in &self.choices {
            choice.set_selected(id);
        }
        self.updating.set(false);
    }
    pub fn on_selected(&self, callback: Rc<dyn Fn(ProviderId, bool)>) {
        for choice in &self.choices {
            let callback = callback.clone();
            let updating = self.updating.clone();
            choice.on_selected(move |id, selected| {
                if updating.replace(true) {
                    return;
                }
                // Rendering within a selection event must not rewrite a GTK group
                // while it is switching its active button.
                callback(id, selected);
                updating.set(false);
            });
        }
    }
}
