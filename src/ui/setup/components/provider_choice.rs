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
        root.add_css_class("setup-provider-choice");
        root.set_margin_top(4);
        root.set_margin_bottom(4);
        let choice = gtk::CheckButton::builder()
            .valign(gtk::Align::Center)
            .margin_end(12)
            .build();
        choice.update_property(&[gtk::accessible::Property::Label(&provider.name)]);
        if let Some(group) = group {
            choice.set_group(Some(&group.choice));
        }
        let image = gtk::Image::from_icon_name(&provider.icon_name);
        image.set_pixel_size(32);
        image.set_margin_start(16);
        root.append(&image);
        let label = gtk::Label::builder()
            .label(&provider.name)
            .xalign(0.0)
            .hexpand(true)
            .margin_top(16)
            .margin_bottom(16)
            .css_classes(["heading"])
            .build();
        root.append(&label);
        root.append(&choice);
        let click = gtk::GestureClick::new();
        let button = choice.clone();
        click.connect_released(move |_, _, _, _| button.set_active(true));
        root.add_controller(click);
        let card = root.downgrade();
        choice.connect_toggled(move |button| {
            if let Some(card) = card.upgrade() {
                if button.is_active() {
                    card.add_css_class("selected");
                } else {
                    card.remove_css_class("selected");
                }
            }
        });
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
