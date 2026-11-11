//! Initial configuration of one account in a dedicated modal dialog.
mod components;
mod controller;
mod dialog;
mod models;
mod pages;
use crate::application::Context;
use adw::{gtk, prelude::*};
use drivesync_engine::Account;
/// Presents or focuses the existing setup, preserving its in-progress choices.
pub(crate) fn show(
    context: &Context,
    navigation: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    account: Option<Account>,
) -> adw::Window {
    for window in gtk::Window::list_toplevels() {
        if window.widget_name() == "drivesync-account-setup" {
            if let Ok(window) = window.downcast::<adw::Window>() {
                window.present();
                return window;
            }
        }
    }
    let window = controller::Controller::attach(context, navigation, toasts, account);
    window.present();
    window
}
pub(crate) mod landing;
#[cfg(test)]
pub(crate) mod tests;
