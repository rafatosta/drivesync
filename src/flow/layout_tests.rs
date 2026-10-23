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
                find_button(window.upcast_ref(), "Account Details…")
                    .unwrap()
                    .emit_clicked();
                let dialog = gtk::Window::list_toplevels()
                    .into_iter()
                    .find_map(|widget| widget.downcast::<adw::MessageDialog>().ok())
                    .expect("account details dialog");
                assert!(!dialog.is_response_enabled("save"));
                assert!(find_named(dialog.upcast_ref(), "account-display-name").is_some());
                dialog.response("cancel");
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
                let pending = drivesync_engine::Cancellation::new();
                *cancellation.borrow_mut() = Some(pending.clone());
                connection.close();
                assert!(
                    pending.is_cancelled(),
                    "closing must cancel pending authorization"
                );
                cancellation.borrow_mut().take();
                assert_eq!(nav.visible_page().as_ref(), Some(&page));
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
