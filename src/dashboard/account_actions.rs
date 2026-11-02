//! Account-local commands shared by sidebar and compact dashboard menus.
//! Widgets only dispatch to the existing controller and configuration workflows.
use crate::automatic::{Controller, Phase};
use adw::{
    gtk::{self, gio},
    prelude::*,
};
use gettextrs::gettext;
use std::time::Duration;

/// One action group is installed on both account surfaces. No command resolves
/// its target from the selected sidebar row; callbacks retain their own account.
pub(super) struct AccountActions {
    group: gio::SimpleActionGroup,
    model: gio::Menu,
}
impl AccountActions {
    pub fn new(
        job: Controller,
        folders: &gio::SimpleActionGroup,
        configuration: &adw::ExpanderRow,
        user: &adw::ActionRow,
        list: &gtk::ListBox,
        sign_out: &gtk::Button,
    ) -> Self {
        let group = gio::SimpleActionGroup::new();
        for name in ["open-local", "open-remote"] {
            if let Some(action) = folders.lookup_action(name) {
                group.add_action(&action);
            }
        }
        let sync = gio::SimpleAction::new("sync", None);
        let clicked = job.clone();
        sync.connect_activate(move |_, _| {
            if !clicked.running() {
                clicked.request();
            }
        });
        group.add_action(&sync);
        let toggle = gio::SimpleAction::new("toggle", None);
        let clicked = job.clone();
        toggle.connect_activate(move |_, _| {
            if clicked.running() {
                clicked.pause();
            } else {
                clicked.request();
            }
        });
        group.add_action(&toggle);
        let configure = gio::SimpleAction::new("configure", None);
        let weak_configuration = configuration.downgrade();
        let weak_list = list.downgrade();
        let weak_user = user.downgrade();
        configure.connect_activate(move |_, _| {
            if let (Some(list), Some(user), Some(configuration)) = (
                weak_list.upgrade(),
                weak_user.upgrade(),
                weak_configuration.upgrade(),
            ) {
                list.select_row(Some(&user));
                list.emit_by_name::<()>("row-activated", &[&user]);
                configuration.set_expanded(true);
                configuration.grab_focus();
            }
        });
        group.add_action(&configure);
        let logout = gio::SimpleAction::new("sign-out", None);
        let weak_sign_out = sign_out.downgrade();
        logout.connect_activate(move |_, _| {
            if let Some(button) = weak_sign_out.upgrade() {
                if button.is_sensitive() {
                    button.emit_clicked();
                }
            }
        });
        group.add_action(&logout);
        let model = gio::Menu::new();
        let operation = gio::Menu::new();
        let paths = gio::Menu::new();
        paths.append(
            Some(&gettext("Open local folder")),
            Some("account-controls.open-local"),
        );
        paths.append(
            Some(&gettext("Open Google Drive folder")),
            Some("account-controls.open-remote"),
        );
        let settings = gio::Menu::new();
        settings.append(
            Some(&gettext("Configure account")),
            Some("account-controls.configure"),
        );
        settings.append(
            Some(&gettext("Account preferences…")),
            Some("account-controls.preferences"),
        );
        let session = gio::Menu::new();
        session.append(
            Some(&gettext("Sign Out")),
            Some("account-controls.sign-out"),
        );
        for section in [&operation, &paths, &settings, &session] {
            model.append_section(None, section);
        }
        let weak_sign_out = sign_out.downgrade();
        let last_revision = std::cell::Cell::new(u64::MAX);
        let refresh = move || {
            let snapshot = job.snapshot();
            if last_revision.replace(snapshot.revision) == snapshot.revision {
                return;
            }
            let running = snapshot.phase == Phase::Running;
            let configured = job.folder().is_some() && snapshot.phase != Phase::Unconfigured;
            sync.set_enabled(configured && !running);
            toggle.set_enabled(configured && !snapshot.pausing);
            logout.set_enabled(!running);
            if let Some(button) = weak_sign_out.upgrade() {
                button.set_sensitive(!running);
            }
            operation.remove_all();
            operation.append(
                Some(&gettext("Synchronize now")),
                Some("account-controls.sync"),
            );
            operation.append(
                Some(&gettext(if running {
                    "Pause synchronization"
                } else if snapshot.phase == Phase::Paused {
                    "Resume synchronization"
                } else {
                    "Start synchronization"
                })),
                Some("account-controls.toggle"),
            );
        };
        refresh();
        let weak_group = group.downgrade();
        gtk::glib::timeout_add_local(Duration::from_millis(100), move || {
            if weak_group.upgrade().is_none() {
                return gtk::glib::ControlFlow::Break;
            }
            refresh();
            gtk::glib::ControlFlow::Continue
        });
        Self { group, model }
    }
    pub fn install(&self, widget: &impl IsA<gtk::Widget>) {
        widget.insert_action_group("account-controls", Some(&self.group));
    }
    pub fn group(&self) -> &gio::SimpleActionGroup {
        &self.group
    }
    pub fn menu(&self) -> gtk::MenuButton {
        gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .tooltip_text(gettext("Account actions"))
            .menu_model(&self.model)
            .valign(gtk::Align::Center)
            .build()
    }
}
