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
    Dashboard,
}
/// Selects the initial screen according to saved accounts and configuration.
///
/// An empty list opens welcome. Every saved account opens the dashboard,
/// where folder configuration is edited inline.
pub fn initial_destination(accounts: &[RestoredAccount]) -> InitialDestination {
    if accounts.is_empty() {
        InitialDestination::Accounts
    } else {
        InitialDestination::Dashboard
    }
}
/// Restores the dashboard with the newly authorized account selected.
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
                        nav.replace(&[crate::dashboard::build_selected(
                            engine.clone(),
                            accounts,
                            &nav,
                            &toasts,
                            Some(&account.id.0),
                        )]);
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
