//! Desktop background permission, discoverable notifications and window activation.
//!
//! A denied or unavailable portal never silently hides the application. Automatic
//! startup is requested through the same portal and always uses a hidden mode.
mod portal;
mod startup;
pub(crate) use startup::{configure_application, install_startup};

use adw::{
    gtk::{gio, glib},
    prelude::*,
};
use std::{cell::Cell, collections::HashMap, rc::Rc};

const PORTAL: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";
const INTERFACE: &str = "org.freedesktop.portal.Background";
const NOTICE: &str = "background";

/// Installs application actions and intercepts close until background permission
/// has been obtained. All portal communication runs on the GTK main context.
pub(crate) fn install(app: &adw::Application, window: &adw::ApplicationWindow) {
    let activations = Rc::new(Cell::new(0u64));
    let shown = activations.clone();
    let show = gio::SimpleAction::new("show", None);
    let weak_window = window.downgrade();
    let weak_app = app.downgrade();
    show.connect_activate(move |_, _| {
        if let (Some(window), Some(app)) = (weak_window.upgrade(), weak_app.upgrade()) {
            shown.set(shown.get().wrapping_add(1));
            app.withdraw_notification(NOTICE);
            window.present();
        }
    });
    app.add_action(&show);
    app.connect_activate(|app| app.activate_action("show", None));
    app.connect_shutdown(|app| app.withdraw_notification(NOTICE));

    let pending = Rc::new(Cell::new(false));
    window.connect_close_request(move |window| {
        if !crate::services::preferences::Preferences::load().keep_running {
            if let Some(app) = window.application() {
                app.quit();
            }
            return glib::Propagation::Proceed;
        }
        if pending.replace(true) {
            return glib::Propagation::Stop;
        }
        let pending = pending.clone();
        let activations = activations.clone();
        let requested_at = activations.get();
        let weak_window = window.downgrade();
        glib::spawn_future_local(async move {
            let allowed = portal::request().await.background;
            pending.set(false);
            if activations.get() != requested_at { return; }
            let Some(window) = weak_window.upgrade() else { return; };
            if allowed {
                let Some(app) = window.application() else { return; };
                if crate::services::preferences::Preferences::load().notify_on_close {
                    let notice = gio::Notification::new("DriveSync continues running");
                    notice.set_body(Some("DriveSync is running in the background."));
                    notice.set_default_action("app.show");
                    notice.add_button("Open DriveSync", "app.show");
                    notice.add_button("Quit", "app.quit");
                    app.send_notification(Some(NOTICE), &notice);
                }
                window.set_visible(false);
                publish_status();
            } else {
                let dialog = adw::MessageDialog::new(Some(&window), Some("Keep DriveSync open?"), Some("Background execution was not allowed or is unavailable. Keep this window open to continue using DriveSync, or quit the application."));
                dialog.add_responses(&[("keep", "Keep Open"), ("quit", "Quit")]);
                dialog.set_default_response(Some("keep"));
                dialog.set_close_response("keep");
                dialog.connect_response(None, |dialog, response| {
                    if response == "quit" {
                        if let Some(window) = dialog.transient_for() {
                            if let Some(app) = window.application() { app.quit(); }
                        }
                    }
                });
                dialog.present();
            }
        });
        glib::Propagation::Stop
    });
}

fn publish_status() {
    glib::spawn_future_local(async move {
        let Ok(bus) = gio::bus_get_future(gio::BusType::Session).await else {
            return;
        };
        let message = "Manage connected accounts";
        let options = HashMap::from([("message", message.to_variant())]);
        let _ = bus
            .call_future(
                Some(PORTAL),
                PATH,
                INTERFACE,
                "SetStatus",
                Some(&(options,).to_variant()),
                None,
                gio::DBusCallFlags::NONE,
                2_000,
            )
            .await;
    });
}

#[cfg(test)]
mod tests;
