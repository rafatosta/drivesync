//! Account actions coordinate services through a layout-independent view contract.
use super::components::{AccountDetails, AccountEvent};
use crate::{application::Context, services::accounts};
use adw::{gtk, prelude::*};
use drivesync_engine::Account;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
/// Binds one account's actions. No callback receives another component's widgets.
pub(super) fn bind_account(
    view: &AccountDetails,
    account: &Account,
    context: &Context,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    updated: Rc<dyn Fn(Account)>,
) {
    let current = Rc::new(RefCell::new(account.clone()));
    let busy = Rc::new(Cell::new(false));
    let presentation = view.handle();
    let context = context.clone();
    let nav = nav.downgrade();
    let toasts = toasts.downgrade();
    view.on_event(Rc::new(move |event| {
        if busy.get() {
            return;
        }
        match event {
            AccountEvent::Edit => presentation.set_editing(true),
            AccountEvent::Cancel => {
                presentation.set_draft(current.borrow().display_name.as_deref());
                presentation.set_editing(false);
            }
            AccountEvent::Save(text) => {
                let Some(engine) = context.engine.borrow().clone() else {
                    return;
                };
                let name = text.trim().to_owned();
                let name = if name.is_empty() { None } else { Some(name) };
                let id = current.borrow().id.clone();
                busy.set(true);
                presentation.set_saving(true);
                let current = current.clone();
                let busy = busy.clone();
                let presentation = presentation.clone();
                let toasts = toasts.clone();
                let updated = updated.clone();
                gtk::glib::spawn_future_local(async move {
                    let result = accounts::rename(&engine, id, name).await;
                    busy.set(false);
                    presentation.set_saving(false);
                    match result {
                        Ok(account) => {
                            presentation.set_account(&account);
                            presentation.set_editing(false);
                            *current.borrow_mut() = account.clone();
                            updated(account);
                        }
                        Err(error) => {
                            if let Some(toasts) = toasts.upgrade() {
                                toasts.add_toast(adw::Toast::new(
                                    &crate::core::messages::engine_error(&error),
                                ));
                            }
                        }
                    }
                });
            }
            AccountEvent::Authorize => {
                if let (Some(nav), Some(toasts)) = (nav.upgrade(), toasts.upgrade()) {
                    crate::ui::components::connection::show(
                        &context,
                        &nav,
                        &toasts,
                        Some(current.borrow().id.clone()),
                    );
                }
            }
            AccountEvent::Disconnect => {
                let Some(navigation) = nav.upgrade() else { return; };
                let Some(parent) = navigation.root().and_downcast::<gtk::Window>() else { return; };
                busy.set(true);
                presentation.set_saving(true);
                let owner = context.clone(); let nav = nav.clone(); let toasts = toasts.clone();
                let busy = busy.clone(); let presentation = presentation.clone();
                crate::application::accounts::disconnect(&context, &parent, &current.borrow(), Rc::new(move |event| {
                    busy.set(false);
                    presentation.set_saving(false);
                    let (Some(nav), Some(toasts)) = (nav.upgrade(), toasts.upgrade()) else { return; };
                    match event {
                        crate::application::accounts::DisconnectEvent::Cancelled => (),
                        crate::application::accounts::DisconnectEvent::Failed(message) => toasts.add_toast(adw::Toast::new(&message)),
                        crate::application::accounts::DisconnectEvent::Disconnected { draft_removed } => {
                            if !draft_removed { toasts.add_toast(adw::Toast::new(&gettextrs::gettext("Account disconnected. The saved setup draft could not be removed."))); }
                            crate::application::navigation::reload_accounts(&owner, &nav, &toasts, None);
                        }
                    }
                }));
            }
        }
    }));
}
