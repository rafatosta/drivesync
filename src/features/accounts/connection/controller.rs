//! GTK opens the browser; the engine owns callback, exchange and persistence.
use super::dialog::{self, ConnectionView};
use crate::app::Context;
use adw::{gtk, prelude::*};
use drivesync_engine::{
    AccountId, BeginGoogleSignInRequest, CancelGoogleSignInRequest, FinishGoogleSignInRequest,
    GoogleSignInId,
};
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
/// Binds weak widget references so late engine results never retain a closed view.
pub(super) fn bind(
    context: &Context,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    account_id: Option<AccountId>,
    view: &ConnectionView,
) {
    let state = Rc::new(State::default());
    let copy_state = state.clone();
    view.copy.connect_clicked(move |button| {
        if let Some(url) = copy_state.url.borrow().as_ref() {
            button.clipboard().set_text(url);
        }
    });
    let closing = state.clone();
    let owner = context.clone();
    view.window.connect_close_request(move |_| {
        closing.closed.set(true);
        closing.cancelled.set(true);
        closing.url.borrow_mut().take();
        if let (Some(engine), Some(id)) =
            (owner.engine.borrow().clone(), closing.id.borrow().clone())
        {
            engine.cancel_google_sign_in(CancelGoogleSignInRequest { connection_id: id });
        }
        gtk::glib::Propagation::Proceed
    });
    let context = context.clone();
    let widgets = (
        view.window.downgrade(),
        view.connect.downgrade(),
        view.copy.downgrade(),
        view.subtitle.downgrade(),
        view.spinner.downgrade(),
        view.feedback.downgrade(),
        nav.downgrade(),
        toasts.downgrade(),
    );
    view.connect.connect_clicked(move |button| {
        let Some(engine) = context.engine.borrow().clone() else {
            return;
        };
        if state.busy.get() {
            state.cancelled.set(true);
            if let Some(id) = state.id.borrow().clone() {
                let result =
                    engine.cancel_google_sign_in(CancelGoogleSignInRequest { connection_id: id });
                if !result.cancelled {
                    state.cancelled.set(false);
                    if let Some(feedback) = widgets.5.upgrade() {
                        feedback.set_label(&gettextrs::gettext(
                            "Your account is being saved. Please wait.",
                        ));
                        feedback.set_visible(true);
                    }
                }
            }
            button.set_sensitive(false);
            return;
        }
        state.busy.set(true);
        state.cancelled.set(false);
        state.url.borrow_mut().take();
        if let (Some(subtitle), Some(spinner), Some(feedback)) = (
            widgets.3.upgrade(),
            widgets.4.upgrade(),
            widgets.5.upgrade(),
        ) {
            dialog::set_waiting(button, &subtitle, &spinner, &feedback, true);
        }
        let state = state.clone();
        let widgets = widgets.clone();
        let context = context.clone();
        let account_id = account_id.clone();
        gtk::glib::spawn_future_local(async move {
            let begin = engine
                .begin_google_sign_in(BeginGoogleSignInRequest { account_id })
                .await;
            let result = match begin {
                Ok(begin) => {
                    let id = begin.connection_id;
                    *state.id.borrow_mut() = Some(id.clone());
                    if state.closed.get() || state.cancelled.get() {
                        engine.cancel_google_sign_in(CancelGoogleSignInRequest {
                            connection_id: id.clone(),
                        });
                    } else {
                        *context.pending.borrow_mut() = Some(id.clone());
                        *state.url.borrow_mut() = Some(begin.authorization_url.clone());
                        if let Some(copy) = widgets.2.upgrade() {
                            copy.set_sensitive(true);
                        }
                        let launched = gtk::gio::AppInfo::launch_default_for_uri_future(
                            &begin.authorization_url,
                            None::<&gtk::gio::AppLaunchContext>,
                        )
                        .await;
                        if launched.is_err() && !state.closed.get() {
                            if let Some(feedback) = widgets.5.upgrade() {
                                feedback.set_label(&gettextrs::gettext(
                                    "Could not open the browser. Copy the link to continue.",
                                ));
                                feedback.set_visible(true);
                            }
                        }
                    }
                    engine
                        .finish_google_sign_in(FinishGoogleSignInRequest { connection_id: id })
                        .await
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
            if let Some(copy) = widgets.2.upgrade() {
                copy.set_sensitive(false);
            }
            if !state.closed.get() {
                if let (Some(button), Some(subtitle), Some(spinner), Some(feedback)) = (
                    widgets.1.upgrade(),
                    widgets.3.upgrade(),
                    widgets.4.upgrade(),
                    widgets.5.upgrade(),
                ) {
                    dialog::set_waiting(&button, &subtitle, &spinner, &feedback, false);
                    if let Err(error) = &result {
                        if !state.cancelled.get() {
                            feedback.set_label(&crate::core::messages::engine_error(error));
                            feedback.set_visible(true);
                        }
                    }
                }
            }
            if let Ok(result) = result {
                if let Some(window) = widgets.0.upgrade() {
                    window.close();
                }
                if let (Some(nav), Some(toasts)) = (widgets.6.upgrade(), widgets.7.upgrade()) {
                    crate::app::navigation::connected(
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
