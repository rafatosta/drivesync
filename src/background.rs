//! Desktop background permission, discoverable notifications and window activation.
//!
//! A denied or unavailable portal never silently hides the application. No login
//! autostart is requested; automatic downloads continue without requiring a visible window.
use adw::{
    gtk::{gio, glib},
    prelude::*,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    time::Duration,
};

const PORTAL: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";
const INTERFACE: &str = "org.freedesktop.portal.Background";
const NOTICE: &str = "background";
thread_local! {
    static ACTIVE: Cell<usize> = const { Cell::new(0) };
    static IDLE_STATUS: RefCell<&'static str> = const { RefCell::new("Choose folders to enable synchronization") };
}

/// Installs application actions and intercepts close until background permission
/// has been obtained. All portal communication runs on the GTK main context.
pub(crate) fn install(app: &adw::Application, window: &adw::ApplicationWindow) {
    let show = gio::SimpleAction::new("show", None);
    let weak_window = window.downgrade();
    let weak_app = app.downgrade();
    show.connect_activate(move |_, _| {
        if let (Some(window), Some(app)) = (weak_window.upgrade(), weak_app.upgrade()) {
            app.withdraw_notification(NOTICE);
            window.present();
        }
    });
    app.add_action(&show);
    app.connect_activate(|app| app.activate_action("show", None));
    app.connect_shutdown(|app| app.withdraw_notification(NOTICE));

    let pending = Rc::new(Cell::new(false));
    window.connect_close_request(move |window| {
        if pending.replace(true) {
            return glib::Propagation::Stop;
        }
        let pending = pending.clone();
        let weak_window = window.downgrade();
        glib::spawn_future_local(async move {
            let allowed = request().await;
            pending.set(false);
            let Some(window) = weak_window.upgrade() else { return; };
            if allowed {
                let Some(app) = window.application() else { return; };
                let notice = gio::Notification::new("DriveSync continues running");
                notice.set_body(Some("DriveSync is running in the background. Open it to view synchronization status."));
                notice.set_default_action("app.show");
                notice.add_button("Open DriveSync", "app.show");
                notice.add_button("Quit", "app.quit");
                app.send_notification(Some(NOTICE), &notice);
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

/// Requests permission and waits for the portal response, subscribing before
/// sending the request to avoid a fast-response race. Timeout closes the request.
async fn request() -> bool {
    let Ok(bus) = gio::bus_get_future(gio::BusType::Session).await else {
        return false;
    };
    let Some(sender) = bus.unique_name() else {
        return false;
    };
    let token = format!("drivesync{}", glib::monotonic_time());
    let handle = format!(
        "/org/freedesktop/portal/desktop/request/{}/{}",
        sender.trim_start_matches(':').replace('.', "_"),
        token
    );
    let response = Rc::new(RefCell::new(None));
    let received = response.clone();
    let _subscription = bus.subscribe_to_signal(
        Some(PORTAL),
        Some("org.freedesktop.portal.Request"),
        Some("Response"),
        Some(&handle),
        None,
        gio::DBusSignalFlags::NONE,
        move |signal| {
            *received.borrow_mut() = Some(permission_granted(signal.parameters));
        },
    );
    let options = HashMap::from([
        ("handle_token", token.to_variant()),
        (
            "reason",
            "Continue downloads when the DriveSync window is closed".to_variant(),
        ),
        ("autostart", false.to_variant()),
    ]);
    if bus
        .call_future(
            Some(PORTAL),
            PATH,
            INTERFACE,
            "RequestBackground",
            Some(&("", options).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            10_000,
        )
        .await
        .is_err()
    {
        return false;
    }
    for _ in 0..600 {
        if let Some(allowed) = *response.borrow() {
            return allowed;
        }
        glib::timeout_future(Duration::from_millis(100)).await;
    }
    let _ = bus
        .call_future(
            Some(PORTAL),
            &handle,
            "org.freedesktop.portal.Request",
            "Close",
            None,
            None,
            gio::DBusCallFlags::NONE,
            2_000,
        )
        .await;
    false
}

/// Publishes automatic download activity in GNOME's background applications list.
/// Older portal versions may not support status messages; permission and window
/// visibility remain independent of this optional status update.
pub(crate) fn set_activity(active: bool) {
    ACTIVE.with(|count| {
        count.set(if active {
            count.get() + 1
        } else {
            count.get().saturating_sub(1)
        })
    });
    publish_status();
}

/// Updates the aggregate idle state without emitting repeated desktop requests.
pub(crate) fn set_idle_status(message: &'static str) {
    let changed = IDLE_STATUS.with(|current| current.replace(message) != message);
    if changed {
        publish_status();
    }
}

fn publish_status() {
    let active = ACTIVE.with(|count| count.get() > 0);
    glib::spawn_future_local(async move {
        let Ok(bus) = gio::bus_get_future(gio::BusType::Session).await else {
            return;
        };
        let message = if active {
            "Synchronization in progress"
        } else {
            IDLE_STATUS.with(|message| *message.borrow())
        };
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

/// Missing, malformed, cancelled or denied responses fail closed.
fn permission_granted(value: &glib::Variant) -> bool {
    value
        .get::<(u32, HashMap<String, glib::Variant>)>()
        .is_some_and(|(code, values)| {
            code == 0 && values.get("background").and_then(|v| v.get::<bool>()) == Some(true)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Runs only inside an explicitly selected desktop sandbox, without engine data.
    #[test]
    #[ignore = "requires an isolated desktop sandbox and background portal permission"]
    fn desktop_close_and_reopen() {
        assert_eq!(
            std::env::var("DRIVESYNC_BACKGROUND_TEST").as_deref(),
            Ok("1")
        );
        let app = adw::Application::builder()
            .application_id(crate::APP_ID)
            .build();
        let hidden = Rc::new(Cell::new(false));
        let reopened = Rc::new(Cell::new(false));
        let hidden_check = hidden.clone();
        let reopened_check = reopened.clone();
        app.connect_startup(move |app| {
            let window = adw::ApplicationWindow::builder()
                .application(app)
                .title("DriveSync background test")
                .build();
            install(app, &window);
            window.present();
            let weak = window.downgrade();
            glib::timeout_add_local_once(Duration::from_millis(300), move || {
                if let Some(window) = weak.upgrade() {
                    window.close();
                }
            });
            let app = app.clone();
            let hidden_check = hidden_check.clone();
            let reopened_check = reopened_check.clone();
            glib::timeout_add_local_once(Duration::from_secs(10), move || {
                hidden_check.set(!window.is_visible());
                app.activate_action("show", None);
                reopened_check.set(window.is_visible());
                app.quit();
            });
        });
        app.run_with_args::<&str>(&[]);
        assert!(hidden.get(), "background permission must hide the window");
        assert!(reopened.get(), "show action must restore the hidden window");
    }

    #[test]
    fn permission_requires_explicit_success_and_background_grant() {
        for (code, grant, expected) in [
            (0u32, true, true),
            (0, false, false),
            (1, true, false),
            (2, true, false),
        ] {
            let values = HashMap::from([("background", grant.to_variant())]);
            assert_eq!(permission_granted(&(code, values).to_variant()), expected);
        }
        assert!(!permission_granted(&"invalid".to_variant()));
        assert!(!permission_granted(
            &(0u32, HashMap::<String, glib::Variant>::new()).to_variant()
        ));
    }
}
