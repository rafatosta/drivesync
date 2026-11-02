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
            if clicked.snapshot().phase == Phase::Paused {
                clicked.request();
            } else {
                clicked.pause();
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
                Some(&gettext(if snapshot.phase == Phase::Paused {
                    "Resume synchronization"
                } else {
                    "Pause synchronization"
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

#[cfg(test)]
mod tests {
    use super::*;
    use drivesync_engine::{AccountId, AccountSummary, DriveSyncEngine, RestoredAccount};
    use std::{cell::Cell, rc::Rc, sync::Arc};

    #[test]
    #[ignore = "requires a desktop display and an isolated XDG_DATA_HOME"]
    fn native_idle_pause_suspends_checks_instead_of_starting_a_transfer() {
        adw::init().unwrap();
        assert!(std::env::var("XDG_DATA_HOME")
            .unwrap()
            .starts_with("/tmp/drivesync-ui-"));
        let id = AccountId("google:idle-pause-ui".into());
        let saved = RestoredAccount {
            account: AccountSummary {
                id: id.clone(),
                provider_subject: "idle-pause-ui".into(),
                email: None,
            },
            error: None,
            folder: Some(drivesync_engine::FolderConfiguration {
                account_id: id,
                remote: drivesync_engine::DriveFolder {
                    id: "fixture".into(),
                    name: "Fixture".into(),
                },
                local: "/tmp/drivesync-ui-unconfigured-fixture".into(),
                local_display: "/tmp/drivesync-ui-unconfigured-fixture".into(),
            }),
        };
        let job = Controller::attach(Arc::new(DriveSyncEngine::new()), &saved);
        let list = gtk::ListBox::new();
        let user = adw::ActionRow::new();
        list.append(&user);
        let configuration = adw::ExpanderRow::new();
        let logout = gtk::Button::new();
        let actions = AccountActions::new(
            job.clone(),
            &gio::SimpleActionGroup::new(),
            &configuration,
            &user,
            &list,
            &logout,
        );
        assert!(actions.group.is_action_enabled("toggle"));
        actions.group.activate_action("toggle", None);
        assert_eq!(job.snapshot().phase, Phase::Paused);
        assert!(!job.running());
        let wait = gtk::glib::MainLoop::new(None, false);
        let finish = wait.clone();
        gtk::glib::timeout_add_local_once(Duration::from_millis(250), move || finish.quit());
        wait.run();
        assert_eq!(job.snapshot().phase, Phase::Paused);
        assert!(!job.running());
        let operation = actions.model.item_link(0, "section").unwrap();
        let label = operation
            .item_attribute_value(1, "label", None)
            .unwrap()
            .get::<String>()
            .unwrap();
        assert_eq!(label, "Resume synchronization");
    }

    #[test]
    #[ignore = "requires a desktop display"]
    fn native_menus_keep_account_targets_and_configuration_navigation() {
        adw::init().unwrap();
        let list = gtk::ListBox::new();
        let pages = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let selected = Rc::new(Cell::new(-1));
        let observed = selected.clone();
        list.connect_row_selected(move |_, row| observed.set(row.map(|r| r.index()).unwrap_or(-1)));
        let mut rows = vec![];
        let mut configurations = vec![];
        let mut counts = vec![];
        let mut commands = vec![];
        for id in ["one", "two"] {
            let user = adw::ActionRow::builder().title(id).build();
            list.append(&user);
            let configuration = adw::ExpanderRow::new();
            pages.append(&configuration);
            let saved = RestoredAccount {
                account: AccountSummary {
                    id: AccountId(id.into()),
                    provider_subject: id.into(),
                    email: None,
                },
                error: None,
                folder: None,
            };
            let job = Controller::attach(Arc::new(DriveSyncEngine::new()), &saved);
            let folders = gio::SimpleActionGroup::new();
            let counter = Rc::new(Cell::new(0));
            let observed = counter.clone();
            let open = gio::SimpleAction::new("open-local", None);
            open.connect_activate(move |_, _| observed.set(observed.get() + 1));
            folders.add_action(&open);
            let logout = gtk::Button::new();
            pages.append(&logout);
            let actions = AccountActions::new(job, &folders, &configuration, &user, &list, &logout);
            actions.install(&user);
            user.add_suffix(&actions.menu());
            assert!(!actions.group.is_action_enabled("sync"));
            assert!(!actions.group.is_action_enabled("toggle"));
            commands.push(actions.group.clone());
            counts.push(counter);
            rows.push(user);
            configurations.push(configuration);
        }
        list.select_row(Some(&rows[1]));
        rows[0]
            .activate_action("account-controls.open-local", None)
            .unwrap();
        assert_eq!((counts[0].get(), counts[1].get()), (1, 0));
        assert_eq!(selected.get(), 1);
        configurations[0].set_sensitive(false);
        assert!(commands[0].is_action_enabled("open-local"));
        commands[0].activate_action("open-local", None);
        assert_eq!((counts[0].get(), counts[1].get()), (2, 0));
        configurations[0].set_sensitive(true);
        rows[0]
            .activate_action("account-controls.configure", None)
            .unwrap();
        assert_eq!(selected.get(), 0);
        assert!(configurations[0].is_expanded());
        assert!(!configurations[1].is_expanded());
        let weak_group = commands[0].downgrade();
        commands.clear();
        drop(rows);
        drop(list);
        assert!(
            weak_group.upgrade().is_none(),
            "observers must not retain removed account groups"
        );
    }
}
