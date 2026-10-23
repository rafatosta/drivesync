//! Navigation decisions and transitions between account-related pages.
//!
//! Routing reflects saved folder configuration without equating stored
//! metadata with authenticated Google sessions.
use super::worker::work;
use adw::prelude::*;
use drivesync_engine::{AccountSummary, DriveSyncEngine, RestoredAccount};
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
/// list opens welcome, and all other combinations show dashboard.
pub fn initial_destination(accounts: &[RestoredAccount]) -> InitialDestination {
    if accounts.is_empty() {
        InitialDestination::Accounts
    } else if accounts.len() == 1 && accounts[0].folder.is_none() && accounts[0].error.is_none() {
        InitialDestination::FolderSetup
    } else {
        InitialDestination::Dashboard
    }
}
/// Restores the dashboard after authorization, then opens setup if required.
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
    let nav = nav.downgrade();
    let toasts = toasts.downgrade();
    work(
        move || worker.restore_accounts(),
        move |result| {
            if let (Some(nav), Some(toasts)) = (nav.upgrade(), toasts.upgrade()) {
                match result {
                    Ok(accounts) => {
                        let needs_setup = accounts.iter().any(|saved| {
                            saved.account.id == account.id
                                && saved.folder.is_none()
                                && saved.error.is_none()
                        });
                        nav.replace(&[crate::dashboard::build(
                            engine.clone(),
                            accounts,
                            &nav,
                            &toasts,
                        )]);
                        if needs_setup {
                            nav.push(&crate::setup::build(engine, account, &nav, &toasts));
                        }
                    }
                    Err(e) => {
                        toasts.add_toast(adw::Toast::new(&e.to_string()));
                        let _ = nav.activate_action("accounts.refresh", None);
                    }
                }
            }
        },
    );
}
