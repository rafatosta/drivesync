//! First-run welcome screen shown only after an empty saved-account result.
mod view;

use adw::prelude::*;
use drivesync_engine::{Cancellation, DriveSyncEngine};
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// Opens the same account-connection window used by the dashboard.
pub fn build(
    engine: Arc<DriveSyncEngine>,
    navigation: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    cancellation: Rc<RefCell<Option<Cancellation>>>,
) -> adw::ToolbarView {
    let view = view::build();
    let nav = navigation.downgrade();
    let toasts = toasts.downgrade();
    view.connect.connect_clicked(move |_| {
        if let (Some(nav), Some(toasts)) = (nav.upgrade(), toasts.upgrade()) {
            crate::connection::show(engine.clone(), &nav, &toasts, cancellation.clone());
        }
    });
    view.toolbar
}
