//! Account metadata and explicit local-name editing through the public engine API.
use crate::application::Context;
use adw::{gtk, prelude::*};
use drivesync_engine::{Account, AccountState, UpdateAccountRequest};
use std::{cell::RefCell, rc::Rc};
/// Displays identity without interpreting stored authorization as live connectivity.
pub(super) fn build(
    account: &Account,
    context: &Context,
    sidebar: &adw::ActionRow,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    edit: bool,
) -> gtk::ScrolledWindow {
    let content = crate::ui::content();
    let current = Rc::new(RefCell::new(account.clone()));
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
    let toggle = {
        let editor = editor.downgrade();
        let modify = modify.downgrade();
        let save = save.downgrade();
        let cancel = cancel.downgrade();
        Rc::new(move |editing| {
            if let (Some(editor), Some(modify), Some(save), Some(cancel)) = (
                editor.upgrade(),
                modify.upgrade(),
                save.upgrade(),
                cancel.upgrade(),
            ) {
                editor.set_visible(editing);
                modify.set_visible(!editing);
                save.set_visible(editing);
                cancel.set_visible(editing);
            }
        })
    };
    let open_editor = toggle.clone();
    modify.connect_clicked(move |_| open_editor(true));
    let close_editor = toggle.clone();
    let last_saved = current.clone();
    let weak_entry = entry.downgrade();
    cancel.connect_clicked(move |_| {
        if let Some(entry) = weak_entry.upgrade() {
            entry.set_text(last_saved.borrow().display_name.as_deref().unwrap_or(""));
        }
        close_editor(false);
    });
    let owner = context.clone();
    let saved = current.clone();
    let ui = (
        entry.downgrade(),
        heading.downgrade(),
        avatar.downgrade(),
        sidebar.downgrade(),
        toasts.downgrade(),
        cancel.downgrade(),
    );
    save.connect_clicked(move |button| {
        let (Some(engine), Some(entry)) = (owner.engine.borrow().clone(), ui.0.upgrade()) else {
            return;
        };
        let value = entry.text().trim().to_owned();
        let display_name = if value.is_empty() { None } else { Some(value) };
        let id = saved.borrow().id.clone();
        button.set_sensitive(false);
        entry.set_sensitive(false);
        if let Some(cancel) = ui.5.upgrade() {
            cancel.set_sensitive(false);
        }
        let weak_button = button.downgrade();
        let ui = ui.clone();
        let saved = saved.clone();
        let toggle = toggle.clone();
        gtk::glib::spawn_future_local(async move {
            let result = engine
                .update_account(UpdateAccountRequest {
                    account_id: id,
                    display_name,
                })
                .await;
            if let Some(button) = weak_button.upgrade() {
                button.set_sensitive(true);
            }
            if let Some(entry) = ui.0.upgrade() {
                entry.set_sensitive(true);
            }
            if let Some(cancel) = ui.5.upgrade() {
                cancel.set_sensitive(true);
            }
            match result {
                Ok(result) => {
                    let name = result
                        .account
                        .display_name
                        .as_deref()
                        .or(result.account.email.as_deref())
                        .unwrap_or("Google Account");
                    if let Some(heading) = ui.1.upgrade() {
                        heading.set_label(name);
                    }
                    if let Some(avatar) = ui.2.upgrade() {
                        avatar.set_text(Some(name));
                    }
                    if let Some(sidebar) = ui.3.upgrade() {
                        sidebar.set_title(name);
                    }
                    *saved.borrow_mut() = result.account;
                    toggle(false);
                }
                Err(error) => {
                    if let Some(toasts) = ui.4.upgrade() {
                        toasts.add_toast(adw::Toast::new(&crate::core::messages::engine_error(
                            &error,
                        )));
                    }
                }
            }
        });
    });
    let authorize = gtk::Button::with_label(&gettextrs::gettext("Sign in again"));
    authorize.set_halign(gtk::Align::Center);
    let id = account.id.clone();
    let context = context.clone();
    let weak_nav = nav.downgrade();
    let weak_toasts = toasts.downgrade();
    authorize.connect_clicked(move |_| {
        if let (Some(nav), Some(toasts)) = (weak_nav.upgrade(), weak_toasts.upgrade()) {
            crate::ui::components::connection::show(&context, &nav, &toasts, Some(id.clone()));
        }
    });
    content.append(&authorize);
    crate::ui::scroll(&content, 700)
}
