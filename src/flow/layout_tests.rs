//! Opt-in visual regression fixtures for welcome/setup/dashboard.
//!
//! These tests require a desktop display and an isolated XDG_DATA_HOME.
use adw::{gtk, prelude::*};
use drivesync_engine::{AccountSummary, DriveSyncEngine, RestoredAccount};
use std::{sync::Arc, time::Duration};

fn find_button(widget: &gtk::Widget, label: &str) -> Option<gtk::Button> {
    if let Some(button) = widget.downcast_ref::<gtk::Button>() {
        if button.label().as_deref() == Some(label) {
            return Some(button.clone());
        }
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        if let Some(button) = find_button(&widget, label) {
            return Some(button);
        }
        child = widget.next_sibling();
    }
    None
}
fn find_named(widget: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    if widget.widget_name() == name {
        return Some(widget.clone());
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        if let Some(found) = find_named(&widget, name) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}
#[test]
#[ignore = "requires a desktop display and an isolated XDG_DATA_HOME"]
fn folder_setup_and_dashboard_layouts() {
    adw::init().unwrap();
    let output =
        std::env::var("DRIVESYNC_UI_PREVIEW_DIR").expect("explicit preview output directory");
    let data = std::env::var("XDG_DATA_HOME").expect("isolated profile directory");
    assert!(data.starts_with("/tmp/drivesync-ui-"));
    std::env::set_var(
        "DRIVESYNC_GOOGLE_CLIENT_ID",
        "ui-test.apps.googleusercontent.com",
    );
    let metadata = std::path::Path::new(&data).join("drivesync");
    std::fs::create_dir_all(&metadata).unwrap();
    std::fs::write(metadata.join("accounts.json"), r#"{"version":2,"clients":{"ui-test.apps.googleusercontent.com":[{"id":"google:layout-fixture","provider_subject":"layout-fixture","email":"preview@example.com"},{"id":"google:second-fixture","provider_subject":"second-fixture","email":"second@example.com"}]},"folders":{}}"#).unwrap();
    let engine = Arc::new(DriveSyncEngine::new());
    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_icon_theme_name(Some("Adwaita"));
    }
    for width in [1000, 360] {
        for screen in ["welcome", "setup", "dashboard"] {
            let nav = adw::NavigationView::new();
            let toasts = adw::ToastOverlay::new();
            let cancellation = std::rc::Rc::new(std::cell::RefCell::new(None));
            crate::connection::install_action(engine.clone(), &nav, &toasts, cancellation.clone());
            toasts.set_child(Some(&nav));
            let root = adw::NavigationPage::builder()
                .title("Layout fixture")
                .child(&gtk::Box::new(gtk::Orientation::Vertical, 0))
                .tag("welcome")
                .build();
            nav.add(&root);
            let account = AccountSummary {
                id: drivesync_engine::AccountId("google:layout-fixture".into()),
                provider_subject: "layout-fixture".into(),
                email: Some("preview@example.com".into()),
            };
            let page = if screen == "welcome" {
                let welcome =
                    crate::welcome::build(engine.clone(), &nav, &toasts, cancellation.clone());
                root.set_title("Welcome");
                root.set_child(Some(&welcome));
                root.clone()
            } else if screen == "dashboard" {
                crate::dashboard::build(
                    engine.clone(),
                    vec![
                        RestoredAccount {
                            account,
                            error: None,
                            folder: Some(drivesync_engine::FolderConfiguration {
                                account_id: drivesync_engine::AccountId(
                                    "google:layout-fixture".into(),
                                ),
                                remote: drivesync_engine::DriveFolder {
                                    id: "layout-folder".into(),
                                    name: "Projects".into(),
                                },
                                local: "/home/preview/DriveSync/Projects".into(),
                                local_display: "/home/preview/DriveSync/Projects".into(),
                            }),
                        },
                        RestoredAccount {
                            account: AccountSummary {
                                id: drivesync_engine::AccountId("google:second-fixture".into()),
                                provider_subject: "second-fixture".into(),
                                email: Some("second@example.com".into()),
                            },
                            error: None,
                            folder: None,
                        },
                    ],
                    &nav,
                    &toasts,
                )
            } else {
                crate::setup::build(engine.clone(), account, &nav, &toasts)
            };
            if screen != "welcome" {
                nav.push(&page);
            }
            let window = adw::Window::builder()
                .default_width(width)
                .default_height(800)
                .content(&toasts)
                .build();
            window.present();
            let wait = gtk::glib::MainLoop::new(None, false);
            let finish = wait.clone();
            gtk::glib::timeout_add_local_once(Duration::from_millis(350), move || finish.quit());
            wait.run();
            assert!(
                window.width() <= width + 10,
                "layout overflow: {} > {}",
                window.width(),
                width
            );
            if screen == "dashboard" {
                let list = find_named(window.upcast_ref(), "account-list")
                    .unwrap()
                    .downcast::<gtk::ListBox>()
                    .unwrap();
                let details = find_named(window.upcast_ref(), "account-details")
                    .unwrap()
                    .downcast::<gtk::Stack>()
                    .unwrap();
                let split = find_named(window.upcast_ref(), "dashboard-split")
                    .unwrap()
                    .downcast::<adw::NavigationSplitView>()
                    .unwrap();
                assert_eq!(split.is_collapsed(), width < 660);
                assert!(
                    split.width() <= window.width(),
                    "dashboard content overflow"
                );
                let start = find_named(window.upcast_ref(), "start-synchronization").unwrap();
                assert!(!start.is_sensitive());
                assert!(find_button(window.upcast_ref(), "Account Details…").is_none());
                let entry = find_named(window.upcast_ref(), "account-display-name")
                    .unwrap()
                    .downcast::<adw::EntryRow>()
                    .unwrap();
                assert!(entry.is_sensitive());
                let id = drivesync_engine::AccountId("google:layout-fixture".into());
                for label in ["Principal", "", "Principal"] {
                    entry.set_text(label);
                    entry.emit_by_name::<()>("apply", &[]);
                    let wait = gtk::glib::MainLoop::new(None, false);
                    let finish = wait.clone();
                    gtk::glib::timeout_add_local_once(Duration::from_millis(150), move || {
                        finish.quit()
                    });
                    wait.run();
                    let stored = DriveSyncEngine::new().account_description(&id).unwrap();
                    assert_eq!(
                        stored.as_deref(),
                        if label.is_empty() { None } else { Some(label) }
                    );
                    assert!(entry.is_sensitive());
                    assert_eq!(
                        list.row_at_index(0)
                            .unwrap()
                            .downcast::<adw::ActionRow>()
                            .unwrap()
                            .title(),
                        if label.is_empty() {
                            "preview@example.com"
                        } else {
                            label
                        }
                    );
                }
                entry.set_text(&"x".repeat(129));
                entry.emit_by_name::<()>("apply", &[]);
                let wait = gtk::glib::MainLoop::new(None, false);
                let finish = wait.clone();
                gtk::glib::timeout_add_local_once(Duration::from_millis(150), move || {
                    finish.quit()
                });
                wait.run();
                assert!(entry.is_sensitive(), "failed saves remain editable");
                assert_eq!(
                    engine.account_description(&id).unwrap().as_deref(),
                    Some("Principal")
                );
                entry.set_text("Principal");
                entry.emit_by_name::<()>("apply", &[]);
                assert_eq!(nav.visible_page().as_ref(), Some(&page));
                assert!(list.row_at_index(1).is_some());
                list.select_row(list.row_at_index(1).as_ref());
                assert_eq!(
                    details.visible_child_name().as_deref(),
                    Some("google:second-fixture")
                );
                list.select_row(list.row_at_index(0).as_ref());
                assert_eq!(
                    details.visible_child_name().as_deref(),
                    Some("google:layout-fixture")
                );
            }
            // A stack switch invalidates the paintable until the next frame.
            let frame = gtk::glib::MainLoop::new(None, false);
            let finish = frame.clone();
            gtk::glib::timeout_add_local_once(Duration::from_millis(100), move || finish.quit());
            frame.run();
            let snapshot = gtk::Snapshot::new();
            gtk::WidgetPaintable::new(Some(&window)).snapshot(
                &snapshot,
                window.width() as f64,
                window.height() as f64,
            );
            let node = snapshot.to_node().expect("rendered layout");
            let texture = window.renderer().unwrap().render_texture(&node, None);
            texture
                .save_to_png(format!("{output}/{}-{width}.png", screen))
                .unwrap();
            if screen == "welcome" || screen == "dashboard" {
                if screen == "welcome" {
                    find_button(window.upcast_ref(), "Connect Google Account")
                        .unwrap()
                        .emit_clicked();
                } else {
                    find_named(window.upcast_ref(), "add-account")
                        .unwrap()
                        .downcast::<gtk::Button>()
                        .unwrap()
                        .emit_clicked();
                }
                let connection = gtk::Window::list_toplevels()
                    .into_iter()
                    .find_map(|w| {
                        if w.widget_name() == "google-account-connection" {
                            w.downcast::<adw::Window>().ok()
                        } else {
                            None
                        }
                    })
                    .expect("shared account connection window");
                assert!(connection.is_modal());
                assert_eq!(nav.visible_page().as_ref(), Some(&page));
                assert!(find_button(connection.upcast_ref(), "Sign In…").is_some());
                assert!(!find_button(connection.upcast_ref(), "Copy Link")
                    .unwrap()
                    .is_sensitive());
                assert!(
                    cancellation.borrow().is_none(),
                    "opening must not start OAuth"
                );
                let same =
                    crate::connection::show(engine.clone(), &nav, &toasts, cancellation.clone());
                assert_eq!(same, connection, "reuse the open connection window");
                let frame = gtk::glib::MainLoop::new(None, false);
                let finish = frame.clone();
                gtk::glib::timeout_add_local_once(Duration::from_millis(150), move || {
                    finish.quit()
                });
                frame.run();
                let snapshot = gtk::Snapshot::new();
                gtk::WidgetPaintable::new(Some(&connection)).snapshot(
                    &snapshot,
                    connection.width() as f64,
                    connection.height() as f64,
                );
                let node = snapshot.to_node().expect("connection window rendered");
                connection
                    .renderer()
                    .unwrap()
                    .render_texture(&node, None)
                    .save_to_png(format!("{output}/connection-{width}.png"))
                    .unwrap();
                assert!(
                    connection.height() < 570,
                    "connection should fit its content"
                );
                let cancel_action = drivesync_engine::Cancellation::new();
                *cancellation.borrow_mut() = Some(cancel_action.clone());
                find_button(connection.upcast_ref(), "Sign In…")
                    .unwrap()
                    .emit_clicked();
                assert!(
                    cancel_action.is_cancelled(),
                    "primary action must cancel pending login"
                );
                assert!(!find_button(connection.upcast_ref(), "Cancelling…")
                    .unwrap()
                    .is_sensitive());
                cancellation.borrow_mut().take();
                let pending = drivesync_engine::Cancellation::new();
                *cancellation.borrow_mut() = Some(pending.clone());
                connection.close();
                assert!(
                    pending.is_cancelled(),
                    "closing must cancel pending authorization"
                );
                cancellation.borrow_mut().take();
                assert_eq!(nav.visible_page().as_ref(), Some(&page));
                let waiting = crate::connection::waiting_preview(Some(window.upcast_ref()));
                waiting.present();
                let frame = gtk::glib::MainLoop::new(None, false);
                let finish = frame.clone();
                gtk::glib::timeout_add_local_once(Duration::from_millis(150), move || {
                    finish.quit()
                });
                frame.run();
                assert!(find_button(waiting.upcast_ref(), "Cancel")
                    .unwrap()
                    .is_sensitive());
                assert!(find_button(waiting.upcast_ref(), "Sign In…").is_none());
                let subtitle = find_named(waiting.upcast_ref(), "connection-subtitle").unwrap();
                let spinner = find_named(waiting.upcast_ref(), "connection-spinner").unwrap();
                assert_eq!(
                    subtitle.clone().downcast::<gtk::Label>().unwrap().text(),
                    "Complete authorization in your web browser, then return here."
                );
                assert_eq!(subtitle.next_sibling().as_ref(), Some(&spinner));
                assert!(spinner.downcast::<gtk::Spinner>().unwrap().is_spinning());
                let snapshot = gtk::Snapshot::new();
                gtk::WidgetPaintable::new(Some(&waiting)).snapshot(
                    &snapshot,
                    waiting.width() as f64,
                    waiting.height() as f64,
                );
                let node = snapshot.to_node().unwrap();
                waiting
                    .renderer()
                    .unwrap()
                    .render_texture(&node, None)
                    .save_to_png(format!("{output}/connection-waiting-{width}.png"))
                    .unwrap();
                waiting.close();
            }
            if screen == "welcome" {
                assert!(find_button(window.upcast_ref(), "Connect Google Account").is_some());
                for obsolete in ["Reconnect Google Account", "Check Saved Accounts Again"] {
                    assert!(find_button(window.upcast_ref(), obsolete).is_none());
                }
            } else {
                // Cancelling the product confirmation must leave the page usable.
                find_button(window.upcast_ref(), "Sign Out")
                    .expect("logout action")
                    .emit_clicked();
                let dialog = gtk::Window::list_toplevels()
                    .into_iter()
                    .find_map(|widget| widget.downcast::<adw::MessageDialog>().ok())
                    .expect("logout confirmation");
                dialog.response("cancel");
                assert!(nav.is_sensitive());
                assert_eq!(nav.visible_page().as_ref(), Some(&page));
            }
            if screen == "dashboard" {
                let cached = RestoredAccount {
                    account: AccountSummary {
                        id: drivesync_engine::AccountId("google:cached".into()),
                        provider_subject: "cached".into(),
                        email: Some("cached@example.com".into()),
                    },
                    error: Some(drivesync_engine::AuthError::Network),
                    folder: None,
                };
                crate::restoration::show_accounts(
                    engine.clone(),
                    vec![cached],
                    &nav,
                    &toasts,
                    cancellation.clone(),
                );
                assert_eq!(nav.visible_page().unwrap().title(), "Dashboard");
                assert!(
                    !nav.pop(),
                    "saved accounts must not expose welcome through Back"
                );
                crate::restoration::show_accounts(
                    engine.clone(),
                    vec![],
                    &nav,
                    &toasts,
                    cancellation.clone(),
                );
                assert_eq!(nav.visible_page().unwrap().title(), "Welcome");
            }
            window.close();
        }
    }
}
