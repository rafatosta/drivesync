//! Navigation decisions and transitions between account-related pages.
//!
//! Routing reflects saved folder configuration without equating stored
//! metadata with authenticated Google sessions.
use adw::prelude::*;
use drivesync_engine::{AccountSummary, DriveSyncEngine, RestoredAccount};
use super::worker::work;
use std::sync::Arc;

/// First screen to show after attempting to restore saved accounts.
#[derive(Debug, PartialEq, Eq)]
pub enum InitialDestination {
    Accounts,
    FolderSetup,
    Dashboard,
}
/// Selects the initial screen according to saved accounts and configuration.
///
/// A single verified account with no folder opens setup directly; an empty
/// list opens the account chooser, and all other combinations show dashboard.
pub fn initial_destination(accounts: &[RestoredAccount]) -> InitialDestination {
    if accounts.is_empty() {
        InitialDestination::Accounts
    } else if accounts.len() == 1 && accounts[0].folder.is_none() && accounts[0].error.is_none() {
        InitialDestination::FolderSetup
    } else {
        InitialDestination::Dashboard
    }
}
/// Builds the appropriate page for a selected restored account.
///
/// This selects dashboard when a folder is configured, otherwise setup.
/// It does not initiate authentication or file synchronization.
pub fn account_page(
    engine: Arc<DriveSyncEngine>,
    saved: RestoredAccount,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
) -> adw::NavigationPage {
    if saved.folder.is_some() {
        crate::dashboard::build(engine, vec![saved], nav, toasts)
    } else {
        crate::setup::build(engine, saved.account, nav, toasts)
    }
}
/// Resolves a selected account's saved folder before opening its page.
///
/// The storage/restoration operation runs off the GTK main thread. The
/// completion callback navigates on the main context and reports errors in
/// a toast. Navigation handles are weakly held while the job is running.
pub fn open_account(
    engine: Arc<DriveSyncEngine>,
    account: AccountSummary,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
) {
    let worker = engine.clone();
    let id = account.id.clone();
    let nav = nav.downgrade();
    let toasts = toasts.downgrade();
    work(
        move || {
            if worker.saved_folder(&id)?.is_some() {
                worker.restore_accounts().map(Some)
            } else {
                Ok(None)
            }
        },
        move |result| {
            if let (Some(nav), Some(toasts)) = (nav.upgrade(), toasts.upgrade()) {
                match result {
                    Ok(Some(accounts)) => {
                        nav.push(&crate::dashboard::build(engine, accounts, &nav, &toasts))
                    }
                    Ok(None) => nav.push(&crate::setup::build(engine, account, &nav, &toasts)),
                    Err(e) => toasts.add_toast(adw::Toast::new(&e.to_string())),
                }
            }
        },
    );
}
