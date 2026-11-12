//! Initial configuration of one account in a dedicated modal dialog.
mod components;
mod controller;
mod dialog;
mod models;
mod pages;
use crate::application::Context;
use adw::{gtk, prelude::*};
use drivesync_engine::Account;
/// Mounts setup inside the existing application window, preserving startup visibility.
pub(crate) fn mount(
    context: &Context,
    navigation: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    account: Option<Account>,
) -> gtk::Window {
    if let Some(window) = gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|window| window.downcast::<gtk::Window>().ok())
        .find(|window| window.widget_name() == "drivesync-account-setup")
    {
        return window;
    }
    controller::Controller::attach(context, navigation, toasts, account)
}
/// Keeps completed account navigation visible while another account is configured.
pub(crate) fn show(
    context: &Context,
    navigation: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    account: Option<Account>,
) -> gtk::Window {
    if let Some(window) = gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|window| window.downcast::<gtk::Window>().ok())
        .find(|window| window.widget_name() == "drivesync-account-setup")
    {
        window.present();
        return window;
    }
    let window = if navigation
        .root()
        .is_some_and(|root| root.widget_name() == "drivesync-dashboard")
    {
        controller::Controller::attach_additional(context, navigation, toasts, account)
    } else {
        mount(context, navigation, toasts, account)
    };
    window.present();
    window
}
#[cfg(test)]
pub(crate) mod tests;
