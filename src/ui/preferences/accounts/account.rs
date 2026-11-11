//! Account page composes presentation and binds its independent controller.
use crate::application::Context;
use adw::gtk;
use drivesync_engine::Account;
use std::rc::Rc;
/// Displays local account metadata without claiming real-time connectivity.
pub(super) fn build(
    account: &Account,
    context: &Context,
    updated: Rc<dyn Fn(Account)>,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    edit: bool,
) -> gtk::ScrolledWindow {
    let view = super::components::AccountDetails::new(account, edit);
    super::controller::bind_account(&view, account, context, nav, toasts, updated);
    view.widget().clone()
}
