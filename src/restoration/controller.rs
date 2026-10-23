//! Asynchronous account restoration and root-page replacement.
use super::view::RestorationView;
use adw::{gtk, prelude::*};
use drivesync_engine::{Cancellation, DriveSyncEngine};
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// Installs account actions on the persistent navigation view, not on welcome.
/// Errors keep the neutral retry page; they never imply an empty account list.
pub(super) fn bind(
    engine: Arc<DriveSyncEngine>,
    navigation: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    cancellation: Rc<RefCell<Option<Cancellation>>>,
    view: &RestorationView,
    page: &adw::NavigationPage,
) {
    let actions = gtk::gio::SimpleActionGroup::new();
    crate::connection::install_action(engine.clone(), navigation, toasts, cancellation.clone());
    let weak_nav = navigation.downgrade();
    let weak_toasts = toasts.downgrade();
    let status = view.status.clone();
    let spinner = view.spinner.clone();
    let retry = view.retry.clone();
    let page = page.clone();
    let busy = Rc::new(std::cell::Cell::new(false));
    let refresh = Rc::new(move || {
        let (Some(nav), Some(toasts)) = (weak_nav.upgrade(), weak_toasts.upgrade()) else {
            return;
        };
        if busy.replace(true) {
            return;
        }
        // Replacement removes stale welcome/dashboard entries from back navigation.
        if nav.visible_page().is_some() {
            nav.replace(std::slice::from_ref(&page));
        }
        status.set_label("Checking saved accounts…");
        retry.set_visible(false);
        spinner.set_visible(true);
        spinner.start();
        let engine = engine.clone();
        let worker = engine.clone();
        let weak_nav = nav.downgrade();
        let weak_toasts = toasts.downgrade();
        let status = status.clone();
        let retry = retry.clone();
        let spinner = spinner.clone();
        let busy = busy.clone();
        let cancellation = cancellation.clone();
        crate::flow::work(
            move || worker.restore_accounts(),
            move |result| {
                busy.set(false);
                spinner.stop();
                spinner.set_visible(false);
                let (Some(nav), Some(toasts)) = (weak_nav.upgrade(), weak_toasts.upgrade()) else {
                    return;
                };
                match result {
                    Err(error) => {
                        status.set_label(&error.to_string());
                        retry.set_visible(true);
                    }
                    Ok(accounts) => {
                        super::show_accounts(engine, accounts, &nav, &toasts, cancellation)
                    }
                }
            },
        );
    });
    let weak_refresh = Rc::downgrade(&refresh);
    view.retry.connect_clicked(move |_| {
        if let Some(refresh) = weak_refresh.upgrade() {
            refresh();
        }
    });
    let action = gtk::gio::SimpleAction::new("refresh", None);
    let action_refresh = refresh.clone();
    action.connect_activate(move |_, _| action_refresh());
    actions.add_action(&action);
    navigation.insert_action_group("accounts", Some(&actions));
    refresh();
}
