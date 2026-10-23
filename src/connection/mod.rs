//! Shared browser-based Google connection window for first and later accounts.
mod auth_flow;
mod view;

use adw::{gtk, prelude::*};
use drivesync_engine::{Cancellation, DriveSyncEngine};
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// Presents one transient connection window, preserving the underlying page.
/// Closing the window cancels pending authorization through the existing token.
pub fn show(
    engine: Arc<DriveSyncEngine>,
    navigation: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    cancellation: Rc<RefCell<Option<Cancellation>>>,
) -> adw::Window {
    let parent = navigation.root().and_downcast::<gtk::Window>();
    for window in gtk::Window::list_toplevels() {
        if window.widget_name() == "google-account-connection" {
            if let Ok(window) = window.downcast::<adw::Window>() {
                window.present();
                return window;
            }
        }
    }
    let view = view::build(parent.as_ref());
    auth_flow::bind(engine, navigation, toasts, cancellation, &view);
    view.window.present();
    view.window
}

/// Registers the shared connection action on navigation without starting OAuth.
pub fn install_action(
    engine: Arc<DriveSyncEngine>,
    navigation: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    cancellation: Rc<RefCell<Option<Cancellation>>>,
) {
    let actions = gtk::gio::SimpleActionGroup::new();
    let action = gtk::gio::SimpleAction::new("open", None);
    let nav = navigation.downgrade();
    let toasts = toasts.downgrade();
    action.connect_activate(move |_, _| {
        if let (Some(nav), Some(toasts)) = (nav.upgrade(), toasts.upgrade()) {
            show(engine.clone(), &nav, &toasts, cancellation.clone());
        }
    });
    actions.add_action(&action);
    navigation.insert_action_group("connection", Some(&actions));
}

/// Renders the waiting presentation in fixtures without starting real OAuth.
#[cfg(test)]
pub(crate) fn waiting_preview(parent: Option<&gtk::Window>) -> adw::Window {
    let view = view::build(parent);
    view.set_waiting(true);
    view.window
}
