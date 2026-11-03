//! Encapsulated account identity and authentication status.
use adw::{gtk, prelude::*};
use drivesync_engine::{DriveSyncEngine, RestoredAccount};
use std::sync::Arc;

#[derive(Clone)]
pub(in crate::features::dashboard) struct AccountHeader {
    root: gtk::Box,
    title: gtk::Label,
}
impl AccountHeader {
    pub fn new(name: &str, initial_status: &str) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let avatar = adw::Avatar::new(48, Some(name), true);
        root.append(&avatar);
        let identity = gtk::Box::new(gtk::Orientation::Vertical, 6);
        identity.set_hexpand(true);
        let title = gtk::Label::builder()
            .label(name)
            .xalign(0.0)
            .hexpand(true)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .css_classes(["title-1"])
            .build();
        let weak_avatar = avatar.downgrade();
        title.connect_label_notify(move |title| {
            if let Some(avatar) = weak_avatar.upgrade() {
                avatar.set_text(Some(&title.label()));
            }
        });
        let status = gtk::Label::builder()
            .label(initial_status)
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        identity.append(&title);
        identity.append(&status);
        root.append(&identity);
        Self { root, title }
    }
    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }
    pub fn set_menu(&self, menu: &gtk::MenuButton) {
        self.root.append(menu);
    }
    /// Delegates saving to the existing description adapter, keeping identity
    /// widgets private and sidebar identity updates in the same transaction.
    pub fn bind_description(
        &self,
        engine: Arc<DriveSyncEngine>,
        saved: &RestoredAccount,
        entry: &adw::EntryRow,
        group: &adw::PreferencesGroup,
        user: &adw::ActionRow,
        avatar: &adw::Avatar,
    ) {
        super::description::bind(engine, saved, entry, group, &self.title, user, avatar);
    }
}
