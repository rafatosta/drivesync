//! Fresh configuration checks choose the one primary window's presentation.
use super::Context;
use crate::services::setup::{self, ConfigurationValidation};
use adw::{gtk, prelude::*};
use drivesync_engine::{Account, AccountId, ListAccountsRequest};
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum InitialDestination {
    Setup,
    Dashboard,
}
pub(crate) fn initial_destination(
    accounts: &[(Account, ConfigurationValidation)],
) -> InitialDestination {
    if accounts
        .iter()
        .any(|(_, validation)| validation.is_complete())
    {
        InitialDestination::Dashboard
    } else {
        InitialDestination::Setup
    }
}
/// Removes abandoned incomplete accounts before starting a fresh setup. It never
/// constructs a second setup window, resume screen or list of pending accounts.
pub(crate) fn show_accounts(
    context: &Context,
    accounts: Vec<Account>,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    selected: Option<&AccountId>,
    edit: bool,
) {
    if nav.root().is_none() {
        return;
    }
    let generation = context.navigation_generation.get().wrapping_add(1);
    context.navigation_generation.set(generation);
    checking(nav);
    let context = context.clone();
    let navigation = nav.downgrade();
    let overlay = toasts.downgrade();
    let selected = selected.cloned();
    gtk::glib::spawn_future_local(async move {
        let mut checked = Vec::new();
        for account in accounts {
            let validation = setup::validate_configuration(&account).await;
            checked.push((account, validation));
        }
        if context.navigation_generation.get() != generation {
            return;
        }
        // A previous process may have stopped during consent or cleanup. Never
        // resume its account or draft: remove only incomplete local registrations.
        for (account, validation) in &checked {
            if validation.is_complete() {
                continue;
            }
            let engine = context.engine.borrow().clone();
            let result = match engine {
                Some(engine) => crate::services::accounts::disconnect(&engine, account.id.clone())
                    .await
                    .map_err(|error| crate::core::messages::engine_error(&error)),
                None => Err(gettextrs::gettext(
                    "The account service is unavailable. Try again.",
                )),
            };
            let failure = match result {
                Ok(true) => None,
                Ok(false) => Some(gettextrs::gettext(
                    "The saved setup could not be discarded. Try again.",
                )),
                Err(message) => Some(message),
            };
            if let Some(message) = failure {
                if let (Some(nav), Some(toasts)) = (navigation.upgrade(), overlay.upgrade()) {
                    show_error(&context, &nav, &toasts, &message);
                }
                return;
            }
        }
        if context.navigation_generation.get() != generation {
            return;
        }
        let (Some(nav), Some(toasts)) = (navigation.upgrade(), overlay.upgrade()) else {
            return;
        };
        match initial_destination(&checked) {
            InitialDestination::Setup => {
                crate::ui::setup::mount(&context, &nav, &toasts, None);
            }
            InitialDestination::Dashboard => {
                let ready = checked
                    .into_iter()
                    .filter_map(|(account, validation)| validation.is_complete().then_some(account))
                    .collect();
                nav.replace(&[crate::ui::dashboard::build(
                    &context,
                    ready,
                    &nav,
                    &toasts,
                    selected.as_ref(),
                    edit,
                )]);
            }
        }
    });
}
fn checking(nav: &adw::NavigationView) {
    let loading = adw::StatusPage::builder()
        .title(gettextrs::gettext(
            "Checking synchronization configuration…",
        ))
        .build();
    nav.replace(&[adw::NavigationPage::new(&loading, "DriveSync")]);
}
fn show_error(
    context: &Context,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    message: &str,
) {
    let status = adw::StatusPage::builder()
        .title(gettextrs::gettext(
            "Could not validate account configuration.",
        ))
        .description(message)
        .build();
    let retry = gtk::Button::with_label(&gettextrs::gettext("Try again"));
    let navigation = nav.downgrade();
    let overlay = toasts.downgrade();
    let owner = context.clone();
    retry.connect_clicked(move |_| {
        if let (Some(nav), Some(toasts)) = (navigation.upgrade(), overlay.upgrade()) {
            reload_accounts(&owner, &nav, &toasts, None);
        }
    });
    status.set_child(Some(&retry));
    nav.replace(&[adw::NavigationPage::new(&status, "DriveSync")]);
}
/// Shared reload for completed dashboard account actions. Setup keeps its
/// routing view detached and owns its own logout; it cannot navigate by closing.
pub(crate) fn reload_accounts(
    context: &Context,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    selected: Option<AccountId>,
) {
    if nav.root().is_none() {
        return;
    }
    let Some(engine) = context.engine.borrow().clone() else {
        return;
    };
    let generation = context.navigation_generation.get().wrapping_add(1);
    context.navigation_generation.set(generation);
    checking(nav);
    let navigation = nav.downgrade();
    let overlay = toasts.downgrade();
    let owner = context.clone();
    gtk::glib::spawn_future_local(async move {
        let result = engine
            .list_accounts(ListAccountsRequest { provider: None })
            .await;
        if owner.navigation_generation.get() != generation {
            return;
        }
        let (Some(nav), Some(toasts)) = (navigation.upgrade(), overlay.upgrade()) else {
            return;
        };
        match result {
            Ok(result) => show_accounts(
                &owner,
                result.accounts,
                &nav,
                &toasts,
                selected.as_ref(),
                false,
            ),
            Err(error) => show_error(
                &owner,
                &nav,
                &toasts,
                &crate::core::messages::engine_error(&error),
            ),
        }
    });
}
/// Reauthorization still uses the same fresh readiness gate.
pub(crate) fn connected(
    context: &Context,
    id: AccountId,
    _created: bool,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
) {
    reload_accounts(context, nav, toasts, Some(id));
}
#[cfg(test)]
mod tests;
