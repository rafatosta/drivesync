//! Account preferences with local nicknames and completed folder choices.
use crate::ui::components::remote_folder_picker::{
    engine_source::EngineFolderSource, RemoteFolderPicker, RemoteFolderPickerConfig,
    RemoteFolderPickerResult,
};
use crate::{
    application::Context,
    services::configuration::{self, Configuration},
};
use adw::{gtk, prelude::*};
use drivesync_engine::{Account, Engine, ListAccountsRequest};
use gettextrs::gettext;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub(super) fn build(
    context: &Context,
    navigation: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    page.set_widget_name("preferences-accounts");
    let status = adw::PreferencesGroup::builder()
        .title(gettext("Accounts"))
        .description(gettext("Loading saved configuration…"))
        .build();
    page.add(&status);
    let add = adw::PreferencesGroup::new();
    add.set_header_suffix(Some(&crate::ui::components::main_menu::add_account_button()));
    page.add(&add);
    let Some(engine) = context.engine.borrow().clone() else {
        status.set_description(Some(&gettext(
            "The account service is unavailable. Try again.",
        )));
        return page;
    };
    let weak = page.downgrade();
    let context = context.clone();
    let navigation = navigation.downgrade();
    let toasts = toasts.downgrade();
    gtk::glib::spawn_future_local(async move {
        let accounts = engine
            .list_accounts(ListAccountsRequest { provider: None })
            .await;
        let Some(page) = weak.upgrade() else {
            return;
        };
        match accounts {
            Ok(accounts) => {
                let mut found = false;
                for account in accounts.accounts {
                    match configuration::load(&account.id).await {
                        Ok(Some(value)) => {
                            page.add(&editor(&engine, &account, value));
                            if let (Some(nav), Some(toasts)) =
                                (navigation.upgrade(), toasts.upgrade())
                            {
                                page.add(&account_actions(&context, &account, &nav, &toasts));
                            }
                            found = true;
                        }
                        Ok(None) => (),
                        Err(_) => {
                            status.set_description(Some(&gettext("Saved configuration could not be read. Your account has been preserved.")));
                            return;
                        }
                    }
                }
                if found {
                    page.remove(&status);
                } else {
                    status.set_description(Some(&gettext("No completed configuration.")));
                }
            }
            Err(e) => status.set_description(Some(&crate::core::messages::engine_error(&e))),
        }
    });
    page
}

fn editor(engine: &Engine, account: &Account, value: Configuration) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(
            account
                .email
                .as_deref()
                .or(account.display_name.as_deref())
                .unwrap_or("Google Drive"),
        )
        .description(gettext("Changes are checked before they are saved."))
        .build();
    group.add(&nickname_editor(engine, account));
    let current = Rc::new(RefCell::new(value.clone()));
    let saved = Rc::new(RefCell::new(value));
    let busy = Rc::new(Cell::new(false));
    let local = crate::ui::row(&gettext("Local folder"), "", "folder-symbolic");
    local.set_widget_name("preferences-local-folder");
    let remote = crate::ui::row(&gettext("Remote folder"), "", "folder-remote-symbolic");
    remote.set_widget_name("preferences-remote-folder");
    let choose_local = gtk::Button::with_label(&gettext("Choose…"));
    let choose_remote = gtk::Button::with_label(&gettext("Choose…"));
    for button in [&choose_local, &choose_remote] {
        button.set_valign(gtk::Align::Center);
    }
    local.add_suffix(&choose_local);
    remote.add_suffix(&choose_remote);
    group.add(&remote);
    group.add(&local);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let cancel = gtk::Button::with_label(&gettext("Cancel"));
    let save = gtk::Button::with_label(&gettext("Save"));
    save.set_widget_name("preferences-save-configuration");
    save.add_css_class("suggested-action");
    actions.append(&cancel);
    actions.append(&save);
    group.set_header_suffix(Some(&actions));
    let feedback = gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .margin_top(12)
        .build();
    group.add(&feedback);
    let render: Rc<dyn Fn()> = {
        let group = group.downgrade();
        let local = local.downgrade();
        let remote = remote.downgrade();
        let save = save.downgrade();
        let cancel = cancel.downgrade();
        let current = current.clone();
        let saved = saved.clone();
        let busy = busy.clone();
        Rc::new(move || {
            if let Some(group) = group.upgrade() {
                group.set_sensitive(!busy.get());
            }
            let value = current.borrow();
            if let Some(local) = local.upgrade() {
                local.set_subtitle(&crate::services::setup::display_path(&value.local_uri));
            }
            if let Some(remote) = remote.upgrade() {
                remote.set_subtitle(&value.remote_path);
            }
            let changed = *value != *saved.borrow();
            if let Some(save) = save.upgrade() {
                save.set_sensitive(changed);
            }
            if let Some(cancel) = cancel.upgrade() {
                cancel.set_sensitive(changed);
            }
        })
    };
    render();
    cancel.connect_clicked({
        let current = current.clone();
        let saved = saved.clone();
        let render = render.clone();
        let feedback = feedback.downgrade();
        move |_| {
            *current.borrow_mut() = saved.borrow().clone();
            if let Some(label) = feedback.upgrade() {
                label.set_label("");
            }
            render();
        }
    });
    save.connect_clicked({
        let engine = engine.clone();
        let current = current.clone();
        let saved = saved.clone();
        let busy = busy.clone();
        let render = render.clone();
        let feedback = feedback.downgrade();
        move |_| {
            if busy.replace(true) {
                return;
            }
            let value = current.borrow().clone();
            render();
            let engine = engine.clone();
            let current = current.clone();
            let saved = saved.clone();
            let busy = busy.clone();
            let render = render.clone();
            let feedback = feedback.clone();
            gtk::glib::spawn_future_local(async move {
                let result = configuration::save_checked(&engine, value).await;
                busy.set(false);
                let message = match result {
                    Ok(value) => {
                        *saved.borrow_mut() = value.clone();
                        *current.borrow_mut() = value;
                        gettext("Configuration saved.")
                    }
                    Err(e) => e,
                };
                if let Some(label) = feedback.upgrade() {
                    label.set_label(&message);
                }
                render();
            });
        }
    });
    choose_local.connect_clicked({
        let group = group.downgrade();
        let current = current.clone();
        let busy = busy.clone();
        let render = render.clone();
        move |_| {
            let Some(parent) = group
                .upgrade()
                .and_then(|g| g.root())
                .and_downcast::<gtk::Window>()
            else {
                return;
            };
            if busy.replace(true) {
                return;
            }
            render();
            let group = group.clone();
            let current = current.clone();
            let busy = busy.clone();
            let render = render.clone();
            gtk::glib::spawn_future_local(async move {
                let dialog = gtk::FileDialog::builder()
                    .title(gettext("Choose a local folder"))
                    .modal(true)
                    .build();
                let result = dialog.select_folder_future(Some(&parent)).await;
                busy.set(false);
                if group.upgrade().is_some_and(|g| g.is_mapped()) {
                    if let Ok(file) = result {
                        current.borrow_mut().local_uri = file.uri().into();
                    }
                    render();
                }
            });
        }
    });
    let picker: Rc<RefCell<Option<RemoteFolderPicker>>> = Rc::default();
    group.connect_unmap({
        let picker = picker.clone();
        move |_| {
            let pending = picker.borrow_mut().take();
            if let Some(p) = pending {
                p.cancel();
            }
        }
    });
    choose_remote.connect_clicked({
        let group = group.downgrade();
        let engine = engine.clone();
        let current = current.clone();
        let busy = busy.clone();
        let render = render.clone();
        let feedback = feedback.downgrade();
        let picker = picker.clone();
        let account = account.clone();
        move |_| {
            let Some(parent) = group
                .upgrade()
                .and_then(|g| g.root())
                .and_downcast::<gtk::Window>()
            else {
                return;
            };
            if busy.replace(true) {
                return;
            }
            render();
            let group = group.clone();
            let engine = engine.clone();
            let current = current.clone();
            let busy = busy.clone();
            let render = render.clone();
            let feedback = feedback.clone();
            let picker = picker.clone();
            let account = account.clone();
            gtk::glib::spawn_future_local(async move {
                let result = EngineFolderSource::open(engine, account.id.clone()).await;
                if !group.upgrade().is_some_and(|g| g.is_mapped()) {
                    busy.set(false);
                    return;
                }
                match result {
                    Ok((root, source)) => {
                        let provider = crate::services::setup::list_providers()
                            .into_iter()
                            .find(|p| p.id == account.provider);
                        let Some(provider) = provider else {
                            busy.set(false);
                            render();
                            return;
                        };
                        let dialog = RemoteFolderPicker::new(
                            RemoteFolderPickerConfig {
                                provider_name: provider.name,
                                provider_icon_svg: provider.icon_svg.into(),
                                account_label: account.email.unwrap_or_default(),
                                root,
                            },
                            source,
                        );
                        let holder = Rc::downgrade(&picker);
                        dialog.connect_response(move |result| {
                            if let RemoteFolderPickerResult::Selected(location) = result {
                                let mut value = current.borrow_mut();
                                value.remote_id = location.folder.id.0;
                                value.remote_path = location
                                    .path
                                    .iter()
                                    .map(|f| f.name.as_str())
                                    .collect::<Vec<_>>()
                                    .join(" / ");
                            }
                            busy.set(false);
                            render();
                            if let Some(holder) = holder.upgrade() {
                                holder.borrow_mut().take();
                            }
                        });
                        dialog.present(&parent);
                        *picker.borrow_mut() = Some(dialog);
                    }
                    Err(e) => {
                        busy.set(false);
                        if let Some(label) = feedback.upgrade() {
                            label.set_label(&crate::core::messages::engine_error(&e));
                        }
                        render();
                    }
                }
            });
        }
    });
    group
}

/// Saves the local nickname independently of folder access or authorization checks.
fn nickname_editor(engine: &Engine, account: &Account) -> adw::EntryRow {
    let row = adw::EntryRow::builder()
        .title(gettext("Account nickname (optional)"))
        .text(account.display_name.as_deref().unwrap_or(""))
        .show_apply_button(true)
        .build();
    row.set_widget_name("preferences-account-nickname");
    let engine = engine.clone();
    let id = account.id.clone();
    row.connect_apply(move |row| {
        let name = row.text().trim().to_owned();
        let name = if name.is_empty() { None } else { Some(name) };
        row.set_sensitive(false);
        let weak = row.downgrade();
        let engine = engine.clone();
        let id = id.clone();
        gtk::glib::spawn_future_local(async move {
            let result = crate::services::accounts::rename(&engine, id, name).await;
            let Some(row) = weak.upgrade() else {
                return;
            };
            row.set_sensitive(true);
            match result {
                Ok(account) => {
                    row.set_text(account.display_name.as_deref().unwrap_or(""));
                    row.remove_css_class("error");
                    row.set_tooltip_text(None);
                }
                Err(error) => {
                    row.add_css_class("error");
                    row.set_tooltip_text(Some(&crate::core::messages::engine_error(&error)));
                }
            }
        });
    });
    row
}

/// Keeps authorization and local disconnection available beside each account.
fn account_actions(
    context: &Context,
    account: &Account,
    navigation: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    let authorize = adw::ActionRow::builder()
        .activatable(true)
        .title(gettext("Sign in again"))
        .build();
    group.add(&authorize);
    let disconnect = adw::ActionRow::builder()
        .activatable(true)
        .title(gettext("Disconnect account…"))
        .build();
    disconnect.add_css_class("destructive-action");
    group.add(&disconnect);
    let owner = context.clone();
    let id = account.id.clone();
    let nav = navigation.downgrade();
    let overlay = toasts.downgrade();
    authorize.connect_activated(move |_| {
        if let (Some(nav), Some(toasts)) = (nav.upgrade(), overlay.upgrade()) {
            crate::ui::components::connection::show(&owner, &nav, &toasts, Some(id.clone()));
        }
    });
    let owner = context.clone();
    let account = account.clone();
    let nav = navigation.downgrade();
    let overlay = toasts.downgrade();
    disconnect.connect_activated(move |row| {
        let Some(nav_widget) = nav.upgrade() else {
            return;
        };
        let Some(parent) = nav_widget.root().and_downcast::<gtk::Window>() else {
            return;
        };
        row.set_sensitive(false);
        let row = row.downgrade();
        let nav = nav.clone();
        let overlay = overlay.clone();
        let context = owner.clone();
        crate::application::accounts::disconnect(
            &owner,
            &parent,
            &account,
            Rc::new(move |event| {
                if let Some(row) = row.upgrade() {
                    row.set_sensitive(true);
                }
                let (Some(nav), Some(toasts)) = (nav.upgrade(), overlay.upgrade()) else {
                    return;
                };
                use crate::application::accounts::DisconnectEvent;
                match event {
                    DisconnectEvent::Cancelled => (),
                    DisconnectEvent::Failed(message) => toasts.add_toast(adw::Toast::new(&message)),
                    DisconnectEvent::Disconnected { draft_removed } => {
                        if !draft_removed {
                            toasts.add_toast(adw::Toast::new(&gettext(
                                "Account disconnected. The saved setup draft could not be removed.",
                            )));
                        }
                        crate::application::navigation::reload_accounts(
                            &context, &nav, &toasts, None,
                        );
                    }
                }
            }),
        );
    });
    group
}
