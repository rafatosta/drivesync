//! Independent account presentation. Its public contract is data and events.
use adw::{gtk, prelude::*};
use drivesync_engine::{Account, AccountState};
use std::rc::Rc;
/// User intent without references to layout widgets.
pub(in crate::ui::dashboard) enum AccountEvent {
    Edit,
    Cancel,
    Save(String),
    Authorize,
    Disconnect,
}
/// Owns its complete layout; outside modules receive only its root and handle.
pub(in crate::ui::dashboard) struct AccountDetails {
    root: gtk::ScrolledWindow,
    heading: gtk::Label,
    avatar: adw::Avatar,
    editor: adw::PreferencesGroup,
    entry: adw::EntryRow,
    modify: gtk::Button,
    save: gtk::Button,
    cancel: gtk::Button,
    authorize: gtk::Button,
    disconnect: gtk::Button,
}
impl AccountDetails {
    pub fn new(account: &Account, edit: bool) -> Self {
        let content = crate::ui::content();
        let name = account
            .display_name
            .as_deref()
            .or(account.email.as_deref())
            .unwrap_or("Google Account");
        let avatar = adw::Avatar::new(80, Some(name), true);
        content.append(&avatar);
        let heading = gtk::Label::builder()
            .label(name)
            .wrap(true)
            .css_classes(["title-1"])
            .build();
        content.append(&heading);
        let identity = adw::PreferencesGroup::builder()
            .title(gettextrs::gettext("Identity"))
            .build();
        identity.add(&crate::ui::row(
            &gettextrs::gettext("Email"),
            account.email.as_deref().unwrap_or("—"),
            "mail-unread-symbolic",
        ));
        let state = match account.state {
            AccountState::Connected => "Authorized",
            AccountState::ReauthorizationRequired => "Sign-in required",
            AccountState::Unavailable => "Unavailable",
        };
        identity.add(&crate::ui::row(
            &gettextrs::gettext("Account status"),
            &gettextrs::gettext(state),
            "dialog-information-symbolic",
        ));
        content.append(&identity);
        let editor = adw::PreferencesGroup::builder()
            .title(gettextrs::gettext("Account name"))
            .description(gettextrs::gettext(
                "Choose a local name for this account. Your Google profile will not change.",
            ))
            .visible(edit)
            .build();
        editor.set_widget_name("account-name-editor");
        let entry = adw::EntryRow::builder()
            .title(gettextrs::gettext("Name"))
            .text(account.display_name.as_deref().unwrap_or(""))
            .build();
        entry.set_widget_name("account-display-name");
        editor.add(&entry);
        content.append(&editor);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        actions.set_halign(gtk::Align::Center);
        let modify = gtk::Button::with_label(&gettextrs::gettext("Edit account"));
        modify.set_visible(!edit);
        modify.set_widget_name("account-edit");
        let save = gtk::Button::with_label(&gettextrs::gettext("Save"));
        save.add_css_class("suggested-action");
        save.set_visible(edit);
        save.set_widget_name("account-save");
        let cancel = gtk::Button::with_label(&gettextrs::gettext("Cancel"));
        cancel.set_visible(edit);
        actions.append(&modify);
        actions.append(&save);
        actions.append(&cancel);
        content.append(&actions);

        let authorize = gtk::Button::with_label(&gettextrs::gettext("Sign in again"));
        authorize.set_halign(gtk::Align::Center);
        content.append(&authorize);
        let disconnect = gtk::Button::builder()
            .label(gettextrs::gettext("Disconnect account…"))
            .halign(gtk::Align::Center)
            .css_classes(["destructive-action"])
            .build();
        disconnect.set_widget_name("account-disconnect");
        content.append(&disconnect);
        Self {
            root: crate::ui::scroll(&content, 700),
            heading,
            avatar,
            editor,
            entry,
            modify,
            save,
            cancel,
            authorize,
            disconnect,
        }
    }
    pub fn widget(&self) -> &gtk::ScrolledWindow {
        &self.root
    }
    pub fn on_event(&self, callback: Rc<dyn Fn(AccountEvent)>) {
        let cb = callback.clone();
        self.modify.connect_clicked(move |_| cb(AccountEvent::Edit));
        let cb = callback.clone();
        self.cancel
            .connect_clicked(move |_| cb(AccountEvent::Cancel));
        let cb = callback.clone();
        let entry = self.entry.downgrade();
        self.save.connect_clicked(move |_| {
            if let Some(entry) = entry.upgrade() {
                cb(AccountEvent::Save(entry.text().to_string()));
            }
        });
        let cb = callback.clone();
        self.authorize
            .connect_clicked(move |_| cb(AccountEvent::Authorize));
        self.disconnect
            .connect_clicked(move |_| callback(AccountEvent::Disconnect));
    }
    pub fn handle(&self) -> AccountDetailsHandle {
        AccountDetailsHandle {
            heading: self.heading.downgrade(),
            avatar: self.avatar.downgrade(),
            editor: self.editor.downgrade(),
            entry: self.entry.downgrade(),
            modify: self.modify.downgrade(),
            save: self.save.downgrade(),
            cancel: self.cancel.downgrade(),
            authorize: self.authorize.downgrade(),
            disconnect: self.disconnect.downgrade(),
        }
    }
}
/// Weak presentation handle permits late operations to finish after dismissal.
#[derive(Clone)]
pub(in crate::ui::dashboard) struct AccountDetailsHandle {
    heading: gtk::glib::WeakRef<gtk::Label>,
    avatar: gtk::glib::WeakRef<adw::Avatar>,
    editor: gtk::glib::WeakRef<adw::PreferencesGroup>,
    entry: gtk::glib::WeakRef<adw::EntryRow>,
    modify: gtk::glib::WeakRef<gtk::Button>,
    save: gtk::glib::WeakRef<gtk::Button>,
    cancel: gtk::glib::WeakRef<gtk::Button>,
    authorize: gtk::glib::WeakRef<gtk::Button>,
    disconnect: gtk::glib::WeakRef<gtk::Button>,
}
impl AccountDetailsHandle {
    pub fn set_editing(&self, editing: bool) {
        if let Some(editor) = self.editor.upgrade() {
            editor.set_visible(editing);
        }
        if let Some(button) = self.modify.upgrade() {
            button.set_visible(!editing);
        }
        if let Some(button) = self.save.upgrade() {
            button.set_visible(editing);
        }
        if let Some(button) = self.cancel.upgrade() {
            button.set_visible(editing);
        }
    }
    pub fn set_saving(&self, saving: bool) {
        if let Some(entry) = self.entry.upgrade() {
            entry.set_sensitive(!saving);
        }
        for button in [&self.save, &self.cancel, &self.authorize, &self.disconnect] {
            if let Some(button) = button.upgrade() {
                button.set_sensitive(!saving);
            }
        }
    }
    pub fn set_draft(&self, name: Option<&str>) {
        if let Some(entry) = self.entry.upgrade() {
            entry.set_text(name.unwrap_or(""));
        }
    }
    pub fn set_account(&self, account: &Account) {
        let name = account
            .display_name
            .as_deref()
            .or(account.email.as_deref())
            .unwrap_or("Google Account");
        if let Some(heading) = self.heading.upgrade() {
            heading.set_label(name);
        }
        if let Some(avatar) = self.avatar.upgrade() {
            avatar.set_text(Some(name));
        }
        self.set_draft(account.display_name.as_deref());
    }
}
