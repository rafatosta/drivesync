//! Isolated visual contract: account-only routing, explicit editing and app settings.
use super::*;
use adw::{gtk, prelude::*};
use drivesync_engine::{Account, AccountId, AccountState, ProviderId, Timestamp};
fn find(widget: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    if widget.widget_name() == name {
        return Some(widget.clone());
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        if let Some(found) = find(&widget, name) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}
#[test]
#[ignore = "requires an isolated display and XDG profile"]
fn account_only_layout_and_initial_name_editor() {
    let profile = std::env::var("XDG_DATA_HOME").expect("isolated XDG profile required");
    assert!(profile.starts_with("/tmp/drivesync-ui-test-"));
    adw::init().unwrap();
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceLight);
    let app = adw::Application::builder()
        .application_id("com.rtosta.DriveSync.AccountLayoutFixture")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>).unwrap();
    let window = adw::ApplicationWindow::builder()
        .application(&app)
        .default_width(960)
        .default_height(720)
        .build();
    crate::ui::components::main_menu::install(&app, &window);
    assert!(app.lookup_action("about").is_some());

    let context = Context::default();
    let nav = adw::NavigationView::new();
    let toasts = adw::ToastOverlay::new();
    let account = Account {
        id: AccountId("fixture".into()),
        provider: ProviderId("google".into()),
        provider_user_id: "synthetic-subject".into(),
        email: Some("fixture@example.invalid".into()),
        display_name: Some("Fixture name".into()),
        state: AccountState::Connected,
        created_at: Timestamp { unix_millis: 0 },
        updated_at: Timestamp { unix_millis: 0 },
    };
    let page = crate::ui::dashboard::build(
        &context,
        vec![account.clone()],
        &nav,
        &toasts,
        Some(&account.id),
        true,
    );
    assert!(find(page.upcast_ref(), "account-name-editor")
        .unwrap()
        .is_visible());
    assert!(find(page.upcast_ref(), "account-save")
        .unwrap()
        .is_visible());
    assert!(!find(page.upcast_ref(), "account-edit")
        .unwrap()
        .is_visible());
    capture(&window, &page, "new-account");
    for removed in [
        "sidebar-activities",
        "sidebar-transfers",
        "sync-account-now",
        "account-settings-editor",
    ] {
        assert!(find(page.upcast_ref(), removed).is_none());
    }
    let page =
        crate::ui::dashboard::build(&context, vec![account.clone()], &nav, &toasts, None, false);
    assert!(!find(page.upcast_ref(), "account-name-editor")
        .unwrap()
        .is_visible());
    assert!(find(page.upcast_ref(), "account-edit")
        .unwrap()
        .is_visible());
    let button = find(page.upcast_ref(), "account-edit")
        .unwrap()
        .downcast::<gtk::Button>()
        .unwrap();
    button.emit_clicked();
    assert!(find(page.upcast_ref(), "account-name-editor")
        .unwrap()
        .is_visible());
    let preferences = crate::ui::preferences::test_build(&nav, &toasts);
    assert!(find(preferences.upcast_ref(), "preferences-theme").is_some());
    assert!(find(preferences.upcast_ref(), "preferences-keep-running").is_some());
    assert!(find(preferences.upcast_ref(), "preferences-interval").is_none());
    let keep = find(preferences.upcast_ref(), "preferences-keep-running")
        .unwrap()
        .downcast::<adw::SwitchRow>()
        .unwrap();
    keep.set_active(false);
    assert!(!crate::services::preferences::Preferences::load().keep_running);
    assert!(!find(preferences.upcast_ref(), "preferences-notify")
        .unwrap()
        .is_sensitive());
    capture(&window, &preferences, "preferences");
    crate::ui::setup::tests::exercise(&window, &account, capture_dialog);
    window.destroy();
}

/// Capture optional previews of the real account widgets, using synthetic data.
fn capture(window: &adw::ApplicationWindow, child: &impl IsA<gtk::Widget>, name: &str) {
    let Ok(output) = std::env::var("DRIVESYNC_UI_PREVIEW_DIR") else {
        return;
    };
    window.set_content(Some(child));
    window.present();
    let mut rendered = None;
    for _ in 0..20 {
        window.queue_draw();
        let loop_ = gtk::glib::MainLoop::new(None, false);
        let finish = loop_.clone();
        gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(50), move || {
            finish.quit()
        });
        loop_.run();
        let snapshot = gtk::Snapshot::new();
        gtk::WidgetPaintable::new(Some(window)).snapshot(
            &snapshot,
            f64::from(window.width()),
            f64::from(window.height()),
        );
        if let Some(node) = snapshot.to_node() {
            rendered = Some(node);
            break;
        }
    }
    let node = rendered.unwrap_or_else(|| {
        let child = window.child().expect("native content exists");
        child.allocate(child.width(), child.height(), -1, None);
        child.queue_draw();
        let snapshot = gtk::Snapshot::new();
        let background = if adw::StyleManager::default().is_dark() {
            gtk::gdk::RGBA::new(0.14, 0.14, 0.15, 1.0)
        } else {
            gtk::gdk::RGBA::new(0.98, 0.98, 0.98, 1.0)
        };
        snapshot.append_color(
            &background,
            &gtk::graphene::Rect::new(0.0, 0.0, window.width() as f32, window.height() as f32),
        );
        window.snapshot_child(&child, &snapshot);
        snapshot.to_node().expect("native content renders")
    });
    std::fs::create_dir_all(&output).unwrap();
    window
        .renderer()
        .unwrap()
        .render_texture(&node, None)
        .save_to_png(std::path::Path::new(&output).join(format!("{name}.png")))
        .unwrap();
    window.set_content(None::<&gtk::Widget>);
}

/// Captures the real modal setup without accessing personal services.
fn capture_dialog(window: &gtk::Window, name: &str) {
    let output = std::env::var("DRIVESYNC_UI_PREVIEW_DIR").ok();
    window.present();
    let mut rendered = None;
    for _ in 0..20 {
        window.queue_draw();
        let loop_ = gtk::glib::MainLoop::new(None, false);
        let finish = loop_.clone();
        gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(30), move || {
            finish.quit()
        });
        loop_.run();
        let snapshot = gtk::Snapshot::new();
        gtk::WidgetPaintable::new(Some(window)).snapshot(
            &snapshot,
            f64::from(window.width()),
            f64::from(window.height()),
        );
        if let Some(node) = snapshot.to_node() {
            rendered = Some(node);
            break;
        }
    }
    let node = rendered.unwrap_or_else(|| {
        let child = window.child().expect("setup content exists");
        child.allocate(child.width(), child.height(), -1, None);
        child.queue_draw();
        let snapshot = gtk::Snapshot::new();
        let background = if adw::StyleManager::default().is_dark() {
            gtk::gdk::RGBA::new(0.14, 0.14, 0.15, 1.0)
        } else {
            gtk::gdk::RGBA::new(0.98, 0.98, 0.98, 1.0)
        };
        snapshot.append_color(
            &background,
            &gtk::graphene::Rect::new(0.0, 0.0, window.width() as f32, window.height() as f32),
        );
        window.snapshot_child(&child, &snapshot);
        snapshot.to_node().expect("setup renders")
    });
    // Window paintables may omit the background on the headless backend.
    // Unmapped application windows can still have allocated child render nodes.
    let background = if adw::StyleManager::default().is_dark() {
        gtk::gdk::RGBA::new(0.14, 0.14, 0.15, 1.0)
    } else {
        gtk::gdk::RGBA::new(0.98, 0.98, 0.98, 1.0)
    };
    let bounds = node.bounds();
    let width = (window.width() as f32).max(bounds.x() + bounds.width());
    let height = (window.height() as f32).max(bounds.y() + bounds.height());
    let composed = gtk::Snapshot::new();
    composed.append_color(
        &background,
        &gtk::graphene::Rect::new(0.0, 0.0, width, height),
    );
    composed.append_node(&node);
    let node = composed
        .to_node()
        .expect("preview includes window background");
    let Some(output) = output else {
        return;
    };
    std::fs::create_dir_all(&output).unwrap();
    window
        .renderer()
        .unwrap()
        .render_texture(&node, None)
        .save_to_png(std::path::Path::new(&output).join(format!("{name}.png")))
        .unwrap();
}
