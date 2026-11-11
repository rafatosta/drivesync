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
