//! Single-instance command routing and session-start feedback. The --background
//! mode never hides an already visible window or starts a second engine.
use super::{portal, publish_status};
use adw::{
    gtk::{gio, glib},
    prelude::*,
};
use std::{cell::Cell, rc::Rc};

/// Enables a registered boolean option so GApplication forwards each invocation
/// to the primary instance rather than inferring startup mode from process globals.
pub(crate) fn configure_application(app: &adw::Application) {
    app.set_flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE);
    app.add_main_option(
        "background",
        glib::Char::from(0),
        glib::OptionFlags::NONE,
        glib::OptionArg::None,
        "Start without presenting a window",
        None,
    );
}

/// Requests native startup registration once per primary process. A permission
/// failure opens the window and exposes its limitation without blocking manual use.
pub(crate) fn install_startup(
    app: &adw::Application,
    window: &adw::ApplicationWindow,
    toasts: &adw::ToastOverlay,
) {
    let first = Rc::new(Cell::new(true));
    let weak_window = window.downgrade();
    let weak_toasts = toasts.downgrade();
    app.connect_command_line(move |app, command| {
        let hidden = command.options_dict().contains("background");
        if !hidden {
            app.activate();
        }
        if first.replace(false) {
            let weak_window = weak_window.clone();
            let weak_toasts = weak_toasts.clone();
            glib::spawn_future_local(async move {
                let grant = portal::request().await;
                if let (Some(window), Some(toasts)) = (weak_window.upgrade(), weak_toasts.upgrade())
                {
                    apply_grant(&window, &toasts, grant);
                }
            });
        }
        glib::ExitCode::SUCCESS
    });
}

/// Startup success leaves visibility untouched: normal launches already opened
/// the window, while session launches remain hidden. Late grants cannot hide a
/// window that was reopened while the portal was answering.
fn apply_grant(window: &adw::ApplicationWindow, toasts: &adw::ToastOverlay, grant: portal::Grant) {
    if !grant.background || !grant.autostart {
        window.present();
        let message = if !grant.background {
            "Background execution was not allowed. Keep DriveSync open to synchronize."
        } else {
            "Automatic startup could not be enabled. Open DriveSync manually to synchronize."
        };
        toasts.add_toast(adw::Toast::new(message));
    } else {
        publish_status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::background::install;
    use std::{cell::RefCell, time::Duration};
    /// Both denial paths must expose the window; successful late replies must
    /// never hide a window the user has already reopened.
    #[test]
    #[ignore = "requires a desktop display"]
    fn denied_startup_and_late_success_preserve_manual_use() {
        adw::init().unwrap();
        let app = adw::Application::builder()
            .application_id("com.rtosta.DriveSync.StartupFixture")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let toasts = adw::ToastOverlay::new();
        let window = adw::ApplicationWindow::builder()
            .application(&app)
            .content(&toasts)
            .build();
        apply_grant(&window, &toasts, portal::Grant::default());
        assert!(window.is_visible());
        window.set_visible(false);
        apply_grant(
            &window,
            &toasts,
            portal::Grant {
                background: true,
                autostart: false,
            },
        );
        assert!(window.is_visible());
        apply_grant(
            &window,
            &toasts,
            portal::Grant {
                background: true,
                autostart: true,
            },
        );
        assert!(window.is_visible());
        window.destroy();
    }

    /// The real portal registers the login command, while the test only creates
    /// a native window. It never initializes the engine or reads credentials.
    #[test]
    #[ignore = "requires a desktop sandbox and explicit autostart opt-in"]
    fn session_launch_stays_hidden_and_launcher_activation_reopens() {
        assert_eq!(
            std::env::var("DRIVESYNC_BACKGROUND_TEST").as_deref(),
            Ok("1")
        );
        let app = adw::Application::builder()
            .application_id(crate::APP_ID)
            .build();
        configure_application(&app);
        let observed = Rc::new(RefCell::new(None));
        let capture = observed.clone();
        app.connect_startup(move |app| {
            let toasts = adw::ToastOverlay::new();
            let window = adw::ApplicationWindow::builder()
                .application(app)
                .content(&toasts)
                .build();
            install(app, &window);
            install_startup(app, &window, &toasts);
            let capture = capture.clone();
            let app = app.clone();
            glib::timeout_add_local_once(Duration::from_secs(3), move || {
                let hidden = !window.is_visible();
                app.activate();
                let visible = window.is_visible();
                *capture.borrow_mut() = Some((hidden, visible));
                app.quit();
            });
        });
        app.run_with_args(&["drivesync-startup-fixture", "--background"]);
        assert_eq!(*observed.borrow(), Some((true, true)));
    }
}
