//! GTK4/Libadwaita folder setup screen.
//!
//! This module builds the interface independently of its input handlers.
//! The screen reuses the engine facade to validate and save folder pairs;
//! it does not implement synchronization or persist settings itself.
mod actions;
mod view;

use drivesync_engine::{AccountSummary, DriveSyncEngine};
use std::sync::Arc;

/// Builds the folder setup page and connects its actions.
///
/// The widget tree and callbacks are created on the GTK main thread. Blocking
/// reads and configuration writes are delegated through `crate::flow::work`.
pub fn build(
    engine: Arc<DriveSyncEngine>,
    account: AccountSummary,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
) -> adw::NavigationPage {
    let view = view::build(engine.clone(), &account, nav, toasts);
    actions::bind(engine, account, nav, toasts, &view);
    view.page
}
