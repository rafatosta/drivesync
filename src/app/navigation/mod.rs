//! Routing based on local account metadata, without assuming current connectivity.
use super::Context;
use adw::{gtk, prelude::*};
use drivesync_engine::{Account, AccountId, ListAccountsRequest};
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum InitialDestination {
    Welcome,
    Dashboard,
}
pub(crate) fn initial_destination(accounts: &[Account]) -> InitialDestination {
    if accounts.is_empty() {
        InitialDestination::Welcome
    } else {
        InitialDestination::Dashboard
    }
}
/// Replaces the root page so stale welcome/editor views do not remain in history.
pub(crate) fn show_accounts(
    context: &Context,
    accounts: Vec<Account>,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    selected: Option<&AccountId>,
    edit: bool,
) {
    let page = match initial_destination(&accounts) {
        InitialDestination::Welcome => adw::NavigationPage::new(
            &crate::features::welcome::build(),
            &gettextrs::gettext("Welcome"),
        ),
        InitialDestination::Dashboard => {
            crate::features::dashboard::build(context, accounts, nav, toasts, selected, edit)
        }
    };
    nav.replace(&[page]);
}
/// Reloads the authoritative list after connection, selecting and editing a new account.
pub(crate) fn connected(
    context: &Context,
    id: AccountId,
    created: bool,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
) {
    let Some(engine) = context.engine.borrow().clone() else {
        return;
    };
    let weak_nav = nav.downgrade();
    let weak_toasts = toasts.downgrade();
    let context = context.clone();
    gtk::glib::spawn_future_local(async move {
        let result = engine
            .list_accounts(ListAccountsRequest { provider: None })
            .await;
        let (Some(nav), Some(toasts)) = (weak_nav.upgrade(), weak_toasts.upgrade()) else {
            return;
        };
        match result {
            Ok(result) => {
                show_accounts(&context, result.accounts, &nav, &toasts, Some(&id), created)
            }
            Err(error) => toasts.add_toast(adw::Toast::new(&crate::core::messages::engine_error(
                &error,
            ))),
        }
    });
}
#[cfg(test)]
mod tests;
