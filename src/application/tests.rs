//! Isolated visual contract: account-only routing, explicit editing and app settings.
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
    if let Ok(directory) = std::env::var("DRIVESYNC_UI_LOCALE_DIR") {
        crate::core::i18n::init();
        gettextrs::bindtextdomain("drivesync", directory).unwrap();
    }
    adw::init().unwrap();
    // Headless captures inspect final pages without an interactive animation clock.
    gtk::Settings::default()
        .unwrap()
        .set_gtk_enable_animations(false);
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
    let dashboard = crate::ui::dashboard::build(&crate::application::Context::default(), &nav);
    assert!(find(dashboard.upcast_ref(), "dashboard-content")
        .unwrap()
        .first_child()
        .is_none());
    assert!(find(dashboard.upcast_ref(), "account-name-editor").is_none());
    capture(&window, &dashboard, "dashboard-empty");
    let dashboard = crate::ui::dashboard::fixture(&nav, &account);
    let accounts = find(dashboard.upcast_ref(), "dashboard-accounts")
        .unwrap()
        .downcast::<gtk::ListBox>()
        .unwrap();
    let title = find(dashboard.upcast_ref(), "dashboard-account-title")
        .unwrap()
        .downcast::<adw::WindowTitle>()
        .unwrap();
    accounts.select_row(accounts.row_at_index(1).as_ref());
    assert_eq!(title.title(), "Work");
    assert_eq!(title.subtitle(), "work@example.invalid");
    capture(&window, &dashboard, "dashboard-accounts");
    // Actions belong to their row even while another account is selected.
    window.set_content(Some(&dashboard));
    window.present();
    accounts.select_row(accounts.row_at_index(0).as_ref());
    let second_menu = find(
        accounts.row_at_index(1).unwrap().upcast_ref(),
        "sidebar-account-menu",
    )
    .unwrap()
    .downcast::<gtk::MenuButton>()
    .unwrap();
    let model = second_menu.menu_model().unwrap();
    assert_eq!(model.n_items(), 3);
    for (index, action) in ["local", "remote", "sign-out"].iter().enumerate() {
        assert_eq!(
            model
                .item_attribute_value(index as i32, "action", None)
                .unwrap()
                .get::<String>()
                .unwrap(),
            format!("account-menu.{action}")
        );
    }
    second_menu
        .activate_action("account-menu.sign-out", None)
        .unwrap();
    let dialog = gtk::Window::list_toplevels()
        .into_iter()
        .find(|widget| widget.widget_name() == "disconnect-account-confirmation")
        .unwrap()
        .downcast::<adw::MessageDialog>()
        .unwrap();
    assert_eq!(
        dialog
            .extra_child()
            .unwrap()
            .downcast::<gtk::Label>()
            .unwrap()
            .text(),
        "work@example.invalid"
    );
    dialog.response("cancel");
    dialog.close();
    assert!(second_menu.is_sensitive());
    assert_eq!(title.subtitle(), "fixture@example.invalid");
    let split = find(dashboard.upcast_ref(), "dashboard-split")
        .unwrap()
        .downcast::<adw::NavigationSplitView>()
        .unwrap();
    split.set_collapsed(true);
    accounts.row_at_index(0).unwrap().activate();
    assert!(split.shows_content());
    split.set_show_content(false);
    assert!(!split.shows_content());
    let preferences = crate::ui::preferences::test_build(&nav, &toasts);
    assert!(find(preferences.upcast_ref(), "preferences-theme").is_some());
    assert!(find(preferences.upcast_ref(), "preferences-accounts").is_some());
    assert!(find(preferences.upcast_ref(), "preferences-configuration").is_none());
    let sections = find(preferences.upcast_ref(), "preferences-list")
        .unwrap()
        .downcast::<gtk::ListBox>()
        .unwrap();
    assert!(sections.row_at_index(2).is_some());
    assert!(sections.row_at_index(3).is_none());
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
    // Capture the settled page after its transition and asynchronous icon loading.
    let settle = gtk::glib::MainLoop::new(None, false);
    let finish = settle.clone();
    gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(240), move || finish.quit());
    settle.run();
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
