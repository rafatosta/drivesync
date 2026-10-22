//! Startup account restoration section embedded in the welcome screen.
//!
//! Separates the status widgets from asynchronous verification and routing.
mod controller;
mod view;

use adw::gtk;
use drivesync_engine::DriveSyncEngine;
use std::sync::Arc;

/// Builds the restoration status controls and begins the initial refresh.
///
/// Restoration happens asynchronously and does not block GTK.
pub fn build(
    engine: Arc<DriveSyncEngine>,
    navigation: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    connect: &gtk::Button,
) -> gtk::Box {
    let view = view::build();
    controller::bind(engine, navigation, toasts, connect, &view);
    view.panel
}
