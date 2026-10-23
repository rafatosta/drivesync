//! Account restoration before choosing first-run or existing-account navigation.
mod controller;
mod view;

use drivesync_engine::{Cancellation, DriveSyncEngine};
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// Builds a neutral loading page and registers reusable account actions.
/// Welcome is created only after a successful empty account-list result.
pub fn build(
    engine: Arc<DriveSyncEngine>,
    navigation: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    cancellation: Rc<RefCell<Option<Cancellation>>>,
) -> adw::NavigationPage {
    let view = view::build();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(&view.panel));
    let page = adw::NavigationPage::new(&toolbar, "DriveSync");
    controller::bind(engine, navigation, toasts, cancellation, &view, &page);
    page
}

/// Replaces stale navigation after a successful account restoration result.
/// Only an empty result creates welcome; account errors remain on dashboard.
pub(crate) fn show_accounts(
    engine: Arc<DriveSyncEngine>,
    accounts: Vec<drivesync_engine::RestoredAccount>,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    cancellation: Rc<RefCell<Option<Cancellation>>>,
) {
    match crate::flow::initial_destination(&accounts) {
        crate::flow::InitialDestination::Accounts => {
            let welcome = crate::welcome::build(engine, nav, toasts, cancellation);
            nav.replace(&[adw::NavigationPage::new(&welcome, "Welcome")]);
        }
        destination => {
            let setup = if destination == crate::flow::InitialDestination::FolderSetup {
                Some(accounts[0].account.clone())
            } else {
                None
            };
            nav.replace(&[crate::dashboard::build(
                engine.clone(),
                accounts,
                nav,
                toasts,
            )]);
            if let Some(account) = setup {
                nav.push(&crate::setup::build(engine, account, nav, toasts));
            }
        }
    }
}
