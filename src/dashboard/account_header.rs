//! Encapsulated account identity and operational state. The dashboard supplies
//! snapshots and a command callback; callers never mutate the header's widgets.
use crate::automatic::{Phase, Snapshot};
use adw::{gtk, prelude::*};
use drivesync_engine::{DriveSyncEngine, RestoredAccount};
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct AccountHeader {
    root: gtk::Box,
    title: gtk::Label,
    status: gtk::Label,
    toggle: gtk::Button,
    fallback: String,
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
        let toggle = gtk::Button::from_icon_name("media-playback-start-symbolic");
        toggle.set_widget_name("account-sync-toggle");
        toggle.set_tooltip_text(Some("Start or resume synchronization"));
        toggle.set_valign(gtk::Align::Center);
        toggle.set_sensitive(false);
        root.append(&toggle);
        Self {
            root,
            title,
            status,
            toggle,
            fallback: initial_status.into(),
        }
    }
    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }
    pub fn set_menu(&self, menu: &gtk::MenuButton) {
        self.root.append(menu);
    }
    pub fn connect_toggle(&self, callback: impl Fn() + 'static) {
        self.toggle.connect_clicked(move |_| callback());
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
    pub fn present(&self, snapshot: &Snapshot) {
        let status = if snapshot.phase == Phase::Unconfigured {
            self.fallback.as_str()
        } else {
            super::presentation::status(snapshot)
        };
        self.status.set_label(status);
        let (icon, tip, enabled) = match snapshot.phase {
            Phase::Running if snapshot.pausing => {
                ("process-working-symbolic", "Pausing safely…", false)
            }
            Phase::Running => (
                "media-playback-pause-symbolic",
                "Pause synchronization",
                true,
            ),
            Phase::Paused => (
                "media-playback-start-symbolic",
                "Resume synchronization",
                true,
            ),
            Phase::Unconfigured => (
                "media-playback-start-symbolic",
                "Choose a folder to enable synchronization",
                false,
            ),
            _ => (
                "media-playback-start-symbolic",
                "Start or resume synchronization",
                true,
            ),
        };
        self.toggle.set_icon_name(icon);
        self.toggle.set_tooltip_text(Some(&gettextrs::gettext(tip)));
        self.toggle.set_sensitive(enabled);
    }
    #[cfg(test)]
    pub fn status_for_test(&self) -> gtk::Label {
        self.status.clone()
    }
}
