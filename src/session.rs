use adw::{gtk, prelude::*};
use drivesync_engine::{AccountId, DriveSyncEngine};
use std::sync::Arc;

pub fn sign_out_button(
    engine: Arc<DriveSyncEngine>,
    id: AccountId,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
) -> gtk::Button {
    let button = gtk::Button::with_label("Sign Out");
    let weak_nav = nav.downgrade();
    let weak_toasts = toasts.downgrade();
    button.connect_clicked(move |button| {
        let parent = button.root().and_downcast::<gtk::Window>();
        let dialog = adw::MessageDialog::new(parent.as_ref(), Some("Sign Out of DriveSync?"),
            Some("Remove this account and its folder settings from DriveSync. Your local and Google Drive files will be kept."));
        dialog.add_responses(&[("cancel", "Cancel"), ("sign-out", "Sign Out")]);
        dialog.set_response_appearance("sign-out", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel")); dialog.set_close_response("cancel");
        let engine = engine.clone(); let id = id.clone();
        let weak_nav = weak_nav.clone(); let weak_toasts = weak_toasts.clone();
        dialog.connect_response(None, move |_, response| {
            if response != "sign-out" { return; }
            if let Some(nav) = weak_nav.upgrade() { nav.set_sensitive(false); }
            let worker = engine.clone(); let id = id.clone();
            let weak_nav = weak_nav.clone(); let weak_toasts = weak_toasts.clone();
            crate::flow::work(move || worker.sign_out(&id), move |result| {
                if let Some(nav) = weak_nav.upgrade() {
                    nav.set_sensitive(true);
                    if let Some(toasts) = weak_toasts.upgrade() {
                        match result {
                            Ok(()) => {
                                nav.pop_to_tag("welcome");
                                let _ = nav.activate_action("accounts.refresh", None);
                                toasts.add_toast(adw::Toast::new("Signed out. The account was removed from DriveSync. Your files have been kept."));
                            }
                            Err(error) => toasts.add_toast(adw::Toast::new(&error.to_string())),
                        }
                    }
                }
            });
        });
        dialog.present();
    });
    button
}
