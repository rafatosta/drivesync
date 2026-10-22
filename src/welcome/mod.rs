//! First-run DriveSync screen and Google account connection.
//!
//! - [`view`] builds GTK/Libadwaita widgets, presentation and disclosure.
//! - [`auth_flow`] connects UI events to the engine OAuth workflow.
//!
//! No Google credentials are handled directly by the view.
mod auth_flow;
mod view;

use drivesync_engine::{Cancellation, DriveSyncEngine};
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// Builds the welcome screen and wires the Google authorization flow.
///
/// The UI is constructed on the GTK main thread; potentially blocking
/// account operations run in a worker managed by `auth_flow`.
pub fn build(
    engine: Arc<DriveSyncEngine>,
    navigation: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    cancellation: Rc<RefCell<Option<Cancellation>>>,
) -> adw::ToolbarView {
    let view = view::build(engine.clone(), navigation, toasts);
    auth_flow::bind(engine, navigation, toasts, cancellation, &view);
    view.toolbar
}
