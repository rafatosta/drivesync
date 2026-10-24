//! Isolated native lifecycle fixtures; no account or filesystem engine access.
use super::*;
use std::time::Duration;
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
