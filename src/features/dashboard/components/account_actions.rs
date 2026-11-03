//! Account-local commands shared by sidebar and compact dashboard menus.
//! Widgets only dispatch to the existing controller and configuration workflows.
use crate::features::synchronization::automatic::{Controller, Phase};
use adw::{
    gtk::{self, gio},
    prelude::*,
};
use gettextrs::gettext;
use std::time::Duration;

/// One action group is installed on both account surfaces. No command resolves
/// its target from the selected sidebar row; callbacks retain their own account.
pub(in crate::features::dashboard) struct AccountActions {
    group: gio::SimpleActionGroup,
    model: gio::Menu,
}
impl AccountActions {
    pub fn new(
        job: Controller,
        folders: &gio::SimpleActionGroup,
        sign_out: &gtk::Button,
        configure_account: impl Fn() + 'static,
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
        let configure_account = std::rc::Rc::new(configure_account);
        for name in ["configure", "preferences"] {
            let action = gio::SimpleAction::new(name, None);
            let open = configure_account.clone();
            action.connect_activate(move |_, _| open());
            group.add_action(&action);
        }
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
    /// Routes recovery to the existing account-owned confirmation and worker flow.
    pub fn add_recovery(&self, button: &gtk::Button) {
        let action = gio::SimpleAction::new("recover-state", None);
        let weak_button = button.downgrade();
        action.connect_activate(move |_, _| {
            if let Some(button) = weak_button.upgrade() {
                if button.is_sensitive() {
                    button.emit_clicked();
                }
            }
        });
        button
            .bind_property("sensitive", &action, "enabled")
            .sync_create()
            .build();
        self.group.add_action(&action);
        let section = gio::Menu::new();
        section.append(
            Some(&gettext("Recover local state…")),
            Some("account-controls.recover-state"),
        );
        self.model.insert_section(3, None, &section);
    }

    pub fn install(&self, widget: &impl IsA<gtk::Widget>) {
        widget.insert_action_group("account-controls", Some(&self.group));
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
        let logout = gtk::Button::new();
        let actions =
            AccountActions::new(job.clone(), &gio::SimpleActionGroup::new(), &logout, || {});
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
        let configured = Rc::new(Cell::new(-1));
        let mut counts = vec![];
        let mut commands = vec![];
        for id in ["one", "two"] {
            let user = adw::ActionRow::builder().title(id).build();
            list.append(&user);
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
            let configured_account = configured.clone();
            let index = rows.len() as i32;
            let actions = AccountActions::new(job, &folders, &logout, move || {
                configured_account.set(index)
            });
            actions.install(&user);
            user.add_suffix(&actions.menu());
            assert!(!actions.group.is_action_enabled("sync"));
            assert!(!actions.group.is_action_enabled("toggle"));
            commands.push(actions.group.clone());
            counts.push(counter);
            rows.push(user);
        }
        list.select_row(Some(&rows[1]));
        rows[0]
            .activate_action("account-controls.open-local", None)
            .unwrap();
        assert_eq!((counts[0].get(), counts[1].get()), (1, 0));
        assert_eq!(selected.get(), 1);
        pages.set_sensitive(false);
        assert!(commands[0].is_action_enabled("open-local"));
        commands[0].activate_action("open-local", None);
        assert_eq!((counts[0].get(), counts[1].get()), (2, 0));
        pages.set_sensitive(true);
        rows[0]
            .activate_action("account-controls.configure", None)
            .unwrap();
        assert_eq!(configured.get(), 0);
        assert_eq!(
            selected.get(),
            1,
            "opening settings must not change dashboard selection"
        );
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
