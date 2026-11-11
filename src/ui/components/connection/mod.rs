//! Shared sign-in window using only the root public engine API.
mod controller;
mod dialog;
use crate::application::Context;
use adw::{gtk, prelude::*};
use drivesync_engine::AccountId;
/// Presents a single connection window. None adds; Some reauthorizes one account.
pub(crate) fn show(
    context: &Context,
    navigation: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    account_id: Option<AccountId>,
) -> gtk::Window {
    if account_id.is_none() {
        return crate::ui::setup::show(context, navigation, toasts, None);
    }
    for window in gtk::Window::list_toplevels() {
        if window.widget_name() == "google-account-connection" {
            if let Ok(window) = window.downcast::<adw::Window>() {
                window.present();
                return window.upcast();
            }
        }
    }
    let parent = navigation.root().and_downcast::<gtk::Window>();
    let view = dialog::build(parent.as_ref());
    controller::bind(context, navigation, toasts, account_id, &view);
    view.window().present();
    view.window().clone().upcast()
}
/// Registers account connection independently of the selected root page.
pub(crate) fn install_action(
    context: &Context,
    navigation: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
) {
    let group = gtk::gio::SimpleActionGroup::new();
    let action = gtk::gio::SimpleAction::new("open", None);
    let context = context.clone();
    let weak_nav = navigation.downgrade();
    let weak_toasts = toasts.downgrade();
    action.connect_activate(move |_, _| {
        if let (Some(nav), Some(toasts)) = (weak_nav.upgrade(), weak_toasts.upgrade()) {
            show(&context, &nav, &toasts, None);
        }
    });
    group.add_action(&action);
    navigation.insert_action_group("connection", Some(&group));
}
