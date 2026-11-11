//! Independently styled provider selector with a semantic selection event.
use crate::services::setup::RegisteredProvider;
use adw::{gtk, prelude::*};
use drivesync_engine::ProviderId;

pub(in crate::ui::setup) struct ProviderChoice {
    root: gtk::Box,
    choice: gtk::CheckButton,
    id: ProviderId,
}
impl ProviderChoice {
    pub fn new(provider: &RegisteredProvider, group: Option<&Self>) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        root.add_css_class("card");
        root.set_margin_top(4);
        root.set_margin_bottom(4);
        let choice = gtk::CheckButton::builder()
            .label(&provider.name)
            .hexpand(true)
            .margin_top(16)
            .margin_bottom(16)
            .margin_start(16)
            .margin_end(16)
            .build();
        if let Some(group) = group {
            choice.set_group(Some(&group.choice));
        }
        let image = gtk::Image::from_icon_name(&provider.icon_name);
        image.set_pixel_size(32);
        image.set_margin_start(16);
        root.append(&image);
        root.append(&choice);
        Self {
            root,
            choice,
            id: provider.id.clone(),
        }
    }
    pub fn widget(&self) -> &impl IsA<gtk::Widget> {
        &self.root
    }
    pub fn on_selected(&self, callback: impl Fn(ProviderId, bool) + 'static) {
        let id = self.id.clone();
        self.choice.connect_toggled(move |button| {
            callback(id.clone(), button.is_active());
        });
    }
    pub fn set_selected(&self, id: Option<&ProviderId>) {
        let active = id == Some(&self.id);
        if self.choice.is_active() != active {
            self.choice.set_active(active);
        }
    }
}
