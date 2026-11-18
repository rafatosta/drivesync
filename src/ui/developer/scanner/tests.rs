//! Startup without a console, shared account preparation and isolated graphical Scanner inspection.
use super::*;
use drivesync_engine::{inventory::*, models::*, orchestrator::PreparationState, EngineConfig};
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, time::Instant};
fn wait_until(test: impl Fn() -> bool) {
    let context = gtk::glib::MainContext::default();
    let end = Instant::now() + Duration::from_secs(10);
    while !test() {
        assert!(Instant::now() < end, "startup condition timed out");
        while context.pending() {
            context.iteration(false);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn settle() {
    let loop_ = gtk::glib::MainLoop::new(None, false);
    let done = loop_.clone();
    gtk::glib::timeout_add_local_once(Duration::from_millis(200), move || done.quit());
    loop_.run();
}
fn capture(window: &adw::ApplicationWindow, name: &str) {
    if let Some(path) = std::env::var_os("DRIVESYNC_UI_PREVIEW_DIR") {
        let snapshot = gtk::Snapshot::new();
        gtk::WidgetPaintable::new(Some(window)).snapshot(
            &snapshot,
            window.width() as f64,
            window.height() as f64,
        );
        let node = snapshot.to_node().expect("Scanner renders");
        fs::create_dir_all(&path).unwrap();
        window
            .renderer()
            .unwrap()
            .render_texture(&node, None)
            .save_to_png(PathBuf::from(path).join(format!("{name}.png")))
            .unwrap();
    }
}
#[test]
#[ignore = "requires isolated Wayland display and XDG profile"]
fn startup_without_console_and_scanner_page_share_real_account_preparation() {
    let profile = std::env::var("XDG_DATA_HOME").expect("isolated profile");
    assert!(profile.starts_with("/tmp/drivesync-ui-test-"));
    adw::init().unwrap();
    gtk::Settings::default()
        .unwrap()
        .set_gtk_enable_animations(false);
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceLight);
    let base = PathBuf::from(profile).join("scanner-startup");
    fs::create_dir_all(base.join("accounts")).unwrap();
    fs::set_permissions(base.join("accounts"), fs::Permissions::from_mode(0o700)).unwrap();
    let ids = ["personal@example.invalid", "work@example.invalid"];
    let records:serde_json::Map<String,serde_json::Value>=ids.into_iter().map(|id|(id.into(),serde_json::json!({"account":{"id":id,"provider":"google","provider_user_id":id,"email":id,"display_name":if id.starts_with("personal"){"Personal"}else{"Work"},"state":"ReauthorizationRequired","created_at":{"unix_millis":0},"updated_at":{"unix_millis":0}},"key":format!("google:{id}:synthetic")}))).collect();
    let accounts = base.join("accounts/accounts.json");
    fs::write(&accounts,serde_json::to_vec(&serde_json::json!({"version":1,"application":"org.example.ScannerStartupUI","intent":null,"records":records})).unwrap()).unwrap();
    fs::set_permissions(accounts, fs::Permissions::from_mode(0o600)).unwrap();
    for id in ids {
        let root = base.join(id);
        fs::create_dir_all(root.join("folder")).unwrap();
        fs::write(root.join("existing.txt"), b"existing user file").unwrap();
        fs::write(root.join("folder/child.txt"), b"child").unwrap();
        let config = crate::services::configuration::Configuration {
            account_id: AccountId(id.into()),
            local_uri: gtk::gio::File::for_path(root).uri().into(),
            remote_id: "root".into(),
            remote_path: "Meu Drive".into(),
        };
        gtk::glib::MainContext::default()
            .block_on(crate::services::configuration::seed_for_test(&config));
    }
    let locked = base.join(ids[1]).join("folder");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let context = Context::default();
    let app = adw::Application::builder()
        .application_id("org.example.ScannerStartupUI")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>).unwrap();
    let navigation = adw::NavigationView::new();
    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&navigation));
    let window = adw::ApplicationWindow::builder()
        .application(&app)
        .default_width(1000)
        .default_height(760)
        .content(&toasts)
        .build();
    // Exercise the actual opening/restoration hook. No developer widget exists yet.
    navigation.add(&crate::ui::dashboard::restoration::build(
        EngineConfig {
            data_directory: base.join("accounts"),
            application_id: "org.example.ScannerStartupUI".into(),
            google_oauth_file: None,
        },
        &context,
        &navigation,
        &toasts,
    ));
    window.present();
    wait_until(|| {
        context.detectors.borrow().len() == 2
            && context.detectors.borrow().values().all(|s| {
                matches!(
                    s.orchestrator.preparation_status().state,
                    PreparationState::ReadyForAnalyzer
                        | PreparationState::Partial
                        | PreparationState::Failed
                )
            })
    });
    let first = context
        .detectors
        .borrow()
        .get(ids[0])
        .unwrap()
        .orchestrator
        .clone();
    let second = context
        .detectors
        .borrow()
        .get(ids[1])
        .unwrap()
        .orchestrator
        .clone();
    assert_eq!(
        first.preparation_status().state,
        PreparationState::ReadyForAnalyzer
    );
    assert_eq!(second.preparation_status().state, PreparationState::Partial);
    assert!(first.diagnostics().confirmed_events.is_some());
    assert!(second.diagnostics().confirmed_events.is_some());
    assert_eq!(
        first.diagnostics().delivery,
        Some(drivesync_engine::detectors::DeliveryState::Enabled)
    );
    let before = first.preparation_status().scanner.unwrap().started_at;
    let (root, view) = create(&context);
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new("Scanner", "Developer Console")));
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&root));
    window.set_content(Some(&toolbar));
    view.refresh();
    wait_until(|| !view.busy.get());
    settle();
    assert_eq!(
        first.preparation_status().scanner.unwrap().started_at,
        before,
        "opening the page must reuse startup's scan"
    );
    assert_eq!(view.status.text(), "Scan complete");
    assert_eq!(view.files.text(), "2");
    assert_eq!(view.directories.text(), "2");
    assert_eq!(view.recorded.text(), "3");
    assert!(view.gate.text().contains("delivered to the Analyzer"));
    capture(&window, "scanner-complete");
    let inventory = InventoryManager::open(OpenInventoryRequest {
        local_root: base.join(ids[0]),
        expected: SyncIdentity {
            sync_id: SyncId(format!("developer-unified:{}", ids[0])),
            account_id: AccountId(ids[0].into()),
            remote_root_id: "root".into(),
        },
        create_if_missing: false,
    })
    .unwrap()
    .inventory;
    assert_eq!(
        inventory
            .list_items(ListItemsRequest {
                filter: ItemFilter {
                    statuses: vec![SyncStatus::PendingReconciliation],
                    ..Default::default()
                },
                pagination: Default::default()
            })
            .unwrap()
            .page
            .entries
            .len(),
        3
    );
    view.accounts.set_selected(1);
    settle();
    assert_eq!(view.status.text(), "Partial scan");
    assert!(view.gate.text().contains("Analysis suspended"));
    assert!(view.list.first_child().is_some());
    capture(&window, "scanner-partial");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).unwrap();
    gtk::glib::MainContext::default()
        .block_on(second.prepare())
        .unwrap();
    wait_until(|| second.preparation_status().state == PreparationState::ReadyForAnalyzer);
    settle();
    assert_eq!(view.status.text(), "Scan complete");
    assert!(second.diagnostics().confirmed_events.is_some());
    view.accounts.set_selected(0);
    settle();
    let count = first.diagnostics().confirmed_events.unwrap();
    fs::write(base.join(ids[0]).join("captured-while-paused.txt"), b"new").unwrap();
    wait_until(|| {
        first
            .diagnostics()
            .confirmed_events
            .is_some_and(|n| n > count)
    });
    assert!(first.diagnostics().confirmed_events.is_some());
    window.set_default_size(440, 760);
    settle();
    assert!(
        window.width() <= 460,
        "Scanner page must fit a narrow window"
    );
    capture(&window, "scanner-narrow");
    // Remove an account with a preparation task in flight; it must not resurrect a session.
    gtk::glib::MainContext::default()
        .block_on(detectors::remove(&context, &AccountId(ids[0].into())));
    assert!(!context.detectors.borrow().contains_key(ids[0]));
    assert_eq!(first.preparation_status().state, PreparationState::Stopped);
    gtk::glib::MainContext::default()
        .block_on(detectors::remove(&context, &AccountId(ids[1].into())));
    detectors::shutdown(&context);
    window.close();
}
