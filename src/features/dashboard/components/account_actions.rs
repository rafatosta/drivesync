//! Account-scoped folder, configuration and sign-out actions.
use adw::{
    gtk::{self, gio},
    prelude::*,
};
use gettextrs::gettext;

pub(in crate::features::dashboard) struct AccountActions {
    group: gio::SimpleActionGroup,
    model: gio::Menu,
}

impl AccountActions {
    pub fn new(
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
        let configure = gio::SimpleAction::new("configure", None);
        configure.connect_activate(move |_, _| configure_account());
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
        let session = gio::Menu::new();
        session.append(
            Some(&gettext("Sign Out")),
            Some("account-controls.sign-out"),
        );
        for section in [&paths, &settings, &session] {
            model.append_section(None, section);
        }
        Self { group, model }
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
    use std::{cell::Cell, rc::Rc};

    #[test]
    #[ignore = "requires a desktop display"]
    fn account_actions_keep_their_own_targets_without_sync_commands() {
        adw::init().unwrap();
        let list = gtk::ListBox::new();
        let opened = Rc::new(Cell::new(0));
        let configured = Rc::new(Cell::new(0));
        let mut rows = Vec::new();
        for index in 1..=2 {
            let row = adw::ActionRow::new();
            list.append(&row);
            let folders = gio::SimpleActionGroup::new();
            let open = gio::SimpleAction::new("open-local", None);
            let opened = opened.clone();
            open.connect_activate(move |_, _| opened.set(index));
            folders.add_action(&open);
            let sign_out = gtk::Button::new();
            let configured = configured.clone();
            let actions = AccountActions::new(&folders, &sign_out, move || configured.set(index));
            assert!(!actions.group.has_action("sync"));
            assert!(!actions.group.has_action("toggle"));
            actions.install(&row);
            rows.push(row);
        }
        list.select_row(Some(&rows[1]));
        rows[0]
            .activate_action("account-controls.open-local", None)
            .unwrap();
        rows[0]
            .activate_action("account-controls.configure", None)
            .unwrap();
        assert_eq!(opened.get(), 1);
        assert_eq!(configured.get(), 1);
        assert_eq!(list.selected_row().unwrap().index(), 1);
    }
}
