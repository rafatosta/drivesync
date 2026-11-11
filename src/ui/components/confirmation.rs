//! Account disconnection confirmation. Emits user intent without performing I/O.
use adw::{gtk, prelude::*};
use gettextrs::gettext;
/// Makes the account identity explicit and explains the local-only operation.
pub(crate) fn disconnect(parent: &gtk::Window, identity: &str, callback: impl Fn(bool) + 'static) {
    let dialog = adw::MessageDialog::new(Some(parent), Some(&gettext("Disconnect account?")), Some(&gettext("This removes the account and its saved setup from DriveSync. Files on this computer and in the cloud are kept.")));
    dialog.set_destroy_with_parent(true);
    dialog.set_body_use_markup(false);
    let identity = gtk::Label::builder()
        .label(identity)
        .wrap(true)
        .selectable(true)
        .build();
    dialog.set_extra_child(Some(&identity));
    dialog.add_responses(&[
        ("cancel", &gettext("Cancel")),
        ("disconnect", &gettext("Disconnect")),
    ]);
    dialog.set_response_appearance("disconnect", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    dialog.set_widget_name("disconnect-account-confirmation");
    dialog.connect_response(None, move |_, response| callback(response == "disconnect"));
    dialog.present();
}
