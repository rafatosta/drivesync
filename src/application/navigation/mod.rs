//! Central routing admits only accounts with freshly validated configurations.
use super::Context;
use crate::services::setup::{self, ConfigurationValidation};
use adw::{gtk, prelude::*};
use drivesync_engine::{Account, AccountId, ListAccountsRequest};
use std::rc::Rc;
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
/// Clears stale content immediately and checks directory readiness asynchronously.
/// No list of connected accounts, local draft or close event can bypass this gate.
pub(crate) fn show_accounts(
    context: &Context,
    accounts: Vec<Account>,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    selected: Option<&AccountId>,
    edit: bool,
) {
    let generation = context.navigation_generation.get().wrapping_add(1);
    context.navigation_generation.set(generation);
    let loading = adw::StatusPage::builder()
        .title(gettextrs::gettext(
            "Checking synchronization configuration…",
        ))
        .build();
    nav.replace(&[adw::NavigationPage::new(&loading, "DriveSync")]);
    let context = context.clone();
    let weak_nav = nav.downgrade();
    let weak_toasts = toasts.downgrade();
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
        let (Some(nav), Some(toasts)) = (weak_nav.upgrade(), weak_toasts.upgrade()) else {
            return;
        };
        match initial_destination(&checked) {
            InitialDestination::Setup => {
                let accounts: Vec<_> = checked.into_iter().map(|(account, _)| account).collect();
                let view = crate::ui::setup::landing::PendingAccounts::new(&accounts);
                let owner = context.clone();
                let navigation = nav.downgrade();
                let overlay = toasts.downgrade();
                view.on_resume(Rc::new(move |account| {
                    if let (Some(nav), Some(toasts)) = (navigation.upgrade(), overlay.upgrade()) {
                        crate::ui::setup::show(&owner, &nav, &toasts, Some(account));
                    }
                }));
                let owner = context.clone();
                let navigation = nav.downgrade();
                let overlay = toasts.downgrade();
                let presentation = view.handle();
                view.on_disconnect(Rc::new(move |account| {
                    let Some(nav) = navigation.upgrade() else { return; };
                    let Some(parent) = nav.root().and_downcast::<gtk::Window>() else { return; };
                    let id = account.id.clone();
                    presentation.set_busy(&id, true);
                    let presentation = presentation.clone(); let context = owner.clone(); let navigation = navigation.clone(); let overlay = overlay.clone();
                    super::accounts::disconnect(&owner, &parent, &account, Rc::new(move |event| {
                        presentation.set_busy(&id, false);
                        let (Some(nav), Some(toasts)) = (navigation.upgrade(), overlay.upgrade()) else { return; };
                        match event {
                            super::accounts::DisconnectEvent::Cancelled => (),
                            super::accounts::DisconnectEvent::Failed(message) => toasts.add_toast(adw::Toast::new(&message)),
                            super::accounts::DisconnectEvent::Disconnected { draft_removed } => {
                                if !draft_removed { toasts.add_toast(adw::Toast::new(&gettextrs::gettext("Account disconnected. The saved setup draft could not be removed."))); }
                                reload_accounts(&context, &nav, &toasts, None);
                            }
                        }
                    }));
                }));
                nav.replace(&[adw::NavigationPage::new(
                    view.widget(),
                    &gettextrs::gettext("Set up account"),
                )]);
                if selected.is_none() {
                    offer_initial_setup(&context, &nav, &toasts, accounts.first().cloned());
                }
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
/// Preserves hidden startup and offers an initial/resumed setup at most once.
fn offer_initial_setup(
    context: &Context,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    account: Option<Account>,
) {
    if context.setup_offered.get() {
        return;
    }
    let Some(window) = nav.root().and_downcast::<gtk::Window>() else {
        return;
    };
    let owner = context.clone();
    let navigation = nav.downgrade();
    let overlay = toasts.downgrade();
    let offer = Rc::new(move || {
        if owner.setup_offered.replace(true) {
            return;
        }
        if let (Some(nav), Some(toasts)) = (navigation.upgrade(), overlay.upgrade()) {
            crate::ui::setup::show(&owner, &nav, &toasts, account.clone());
        }
    });
    if window.is_visible() {
        offer();
    } else {
        window.connect_visible_notify(move |window| {
            if window.is_visible() {
                offer();
            }
        });
    }
}
/// Reloads the authoritative list after removal, connection or cancellation.
pub(crate) fn reload_accounts(
    context: &Context,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    selected: Option<AccountId>,
) {
    let Some(engine) = context.engine.borrow().clone() else {
        return;
    };
    let generation = context.navigation_generation.get().wrapping_add(1);
    context.navigation_generation.set(generation);
    let loading = adw::StatusPage::builder()
        .title(gettextrs::gettext(
            "Checking synchronization configuration…",
        ))
        .build();
    nav.replace(&[adw::NavigationPage::new(&loading, "DriveSync")]);
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
            Err(error) => {
                let status = adw::StatusPage::builder()
                    .title(gettextrs::gettext(
                        "Could not validate account configuration.",
                    ))
                    .description(crate::core::messages::engine_error(&error))
                    .build();
                let retry = gtk::Button::with_label(&gettextrs::gettext("Try again"));
                let navigation = nav.downgrade();
                let overlay = toasts.downgrade();
                retry.connect_clicked(move |_| {
                    if let (Some(nav), Some(toasts)) = (navigation.upgrade(), overlay.upgrade()) {
                        reload_accounts(&owner, &nav, &toasts, None);
                    }
                });
                status.set_child(Some(&retry));
                nav.replace(&[adw::NavigationPage::new(&status, "DriveSync")]);
            }
        }
    });
}
/// All post-authorization navigation must pass the same configuration gate.
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
