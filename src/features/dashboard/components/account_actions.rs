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
