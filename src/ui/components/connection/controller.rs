//! Reauthorization coordination through semantic component events and services.
use super::dialog::ConnectionView;
use crate::{application::Context, services::accounts};
use adw::{gtk, prelude::*};
use drivesync_engine::{AccountId, GoogleSignInId};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
#[derive(Default)]
struct State {
    closed: Cell<bool>,
    busy: Cell<bool>,
    cancelled: Cell<bool>,
    id: RefCell<Option<GoogleSignInId>>,
    url: RefCell<Option<String>>,
}
/// Keeps late authorization results from retaining or reopening a dismissed view.
pub(super) fn bind(
    context: &Context,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    account_id: Option<AccountId>,
    view: &ConnectionView,
) {
    let state = Rc::new(State::default());
    let copying = state.clone();
    let copy_view = view.handle();
    view.on_copy(move || {
        if let Some(url) = copying.url.borrow().as_ref() {
            copy_view.copy_url(url);
        }
    });
    let closing = state.clone();
    let owner = context.clone();
    view.on_close(move || {
        closing.closed.set(true);
        closing.cancelled.set(true);
        closing.url.borrow_mut().take();
        if let (Some(engine), Some(id)) =
            (owner.engine.borrow().clone(), closing.id.borrow().clone())
        {
            accounts::cancel(&engine, id);
        }
    });
    let context = context.clone();
    let presentation = view.handle();
    let weak_nav = nav.downgrade();
    let weak_toasts = toasts.downgrade();
    view.on_connect(move || {
        let Some(engine) = context.engine.borrow().clone() else {
            return;
        };
        if state.busy.get() {
            state.cancelled.set(true);
            if let Some(id) = state.id.borrow().clone() {
                if !accounts::cancel(&engine, id) {
                    state.cancelled.set(false);
                    presentation.show_feedback(&gettextrs::gettext(
                        "Your account is being saved. Please wait.",
                    ));
                }
            }
            presentation.set_connect_enabled(false);
            return;
        }
        state.busy.set(true);
        state.cancelled.set(false);
        state.url.borrow_mut().take();
        presentation.set_waiting(true);
        let state = state.clone();
        let presentation = presentation.clone();
        let nav = weak_nav.clone();
        let toasts = weak_toasts.clone();
        let context = context.clone();
        let account_id = account_id.clone();
        gtk::glib::spawn_future_local(async move {
            let result = match accounts::begin(&engine, account_id).await {
                Ok(begin) => {
                    let id = begin.connection_id;
                    *state.id.borrow_mut() = Some(id.clone());
                    if state.closed.get() || state.cancelled.get() {
                        accounts::cancel(&engine, id.clone());
                    } else {
                        *context.pending.borrow_mut() = Some(id.clone());
                        *state.url.borrow_mut() = Some(begin.authorization_url.clone());
                        presentation.set_copy_enabled(true);
                        if accounts::open_authorization_url(&begin.authorization_url)
                            .await
                            .is_err()
                            && !state.closed.get()
                        {
                            presentation.show_feedback(&gettextrs::gettext(
                                "Could not open the browser. Copy the link to continue.",
                            ));
                        }
                    }
                    accounts::finish(&engine, id).await
                }
                Err(error) => Err(error),
            };
            if let Some(id) = state.id.borrow_mut().take() {
                if context.pending.borrow().as_ref() == Some(&id) {
                    context.pending.borrow_mut().take();
                }
            }
            state.busy.set(false);
            state.url.borrow_mut().take();
            presentation.set_copy_enabled(false);
            if !state.closed.get() {
                presentation.set_waiting(false);
                if let Err(error) = &result {
                    if !state.cancelled.get() {
                        presentation.show_feedback(&crate::core::messages::engine_error(error));
                    }
                }
            }
            if let Ok(result) = result {
                presentation.close();
                if let (Some(nav), Some(toasts)) = (nav.upgrade(), toasts.upgrade()) {
                    crate::application::navigation::connected(
                        &context,
                        result.account.id,
                        result.created,
                        &nav,
                        &toasts,
                    );
                }
            }
            state.cancelled.set(false);
        });
    });
}
