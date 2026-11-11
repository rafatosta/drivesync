//! Provider page populated exclusively from catalog metadata.
use super::super::components::ProviderChoice;
use crate::services::setup::RegisteredProvider;
use adw::{gtk, prelude::*};
use drivesync_engine::ProviderId;
use std::rc::Rc;
pub(in crate::ui::setup) struct ProviderPage {
    root: gtk::Box,
    choices: Vec<ProviderChoice>,
}
impl ProviderPage {
    pub fn new(providers: &[RegisteredProvider]) -> Self {
        let root = super::content(
            &gettextrs::gettext("Welcome to DriveSync"),
            &gettextrs::gettext("Choose a provider to connect your account."),
            "folder-remote-symbolic",
        );
        let mut choices = Vec::new();
        for provider in providers {
            let choice = ProviderChoice::new(provider, choices.first());
            root.append(choice.widget());
            choices.push(choice);
        }
        if choices.is_empty() {
            root.append(&gtk::Label::new(Some(&gettextrs::gettext(
                "No providers are registered.",
            ))));
        }
        Self { root, choices }
    }
    pub fn widget(&self) -> &impl IsA<gtk::Widget> {
        &self.root
    }
    pub fn set_selected(&self, id: Option<&ProviderId>, enabled: bool) {
        self.root.set_sensitive(enabled);
        for choice in &self.choices {
            choice.set_selected(id);
        }
    }
    pub fn on_selected(&self, callback: Rc<dyn Fn(ProviderId)>) {
        for choice in &self.choices {
            let callback = callback.clone();
            choice.on_selected(move |id| callback(id));
        }
    }
}
