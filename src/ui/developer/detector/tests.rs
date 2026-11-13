//! Real detector snapshots rendered on an isolated GTK display.
use super::*;
use drivesync_engine::{
    detectors::*, models::SyncId, AccountId, Engine, EngineConfig, Orchestrator,
};
use std::sync::Arc;
fn settle() {
    let loop_ = gtk::glib::MainLoop::new(None, false);
    let finish = loop_.clone();
    gtk::glib::timeout_add_local_once(Duration::from_millis(650), move || finish.quit());
    loop_.run();
}
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
fn texts(widget: &gtk::Widget) -> String {
    let mut output = widget
        .clone()
        .downcast::<gtk::Label>()
        .map(|l| l.text().to_string())
        .unwrap_or_default();
    let mut child = widget.first_child();
    while let Some(widget) = child {
        output.push_str(&texts(&widget));
        child = widget.next_sibling();
    }
    output
}
fn capture(window: &adw::Window, name: &str) {
    let output = std::env::var("DRIVESYNC_UI_PREVIEW_DIR").ok();
    let snapshot = gtk::Snapshot::new();
    gtk::WidgetPaintable::new(Some(window)).snapshot(
        &snapshot,
        window.width() as f64,
        window.height() as f64,
    );
    let node = snapshot.to_node().unwrap_or_else(|| {
        let child = window.child().unwrap();
        child.allocate(child.width(), child.height(), -1, None);
        let snapshot = gtk::Snapshot::new();
        snapshot.append_color(
            &gtk::gdk::RGBA::new(0.98, 0.98, 0.98, 1.0),
            &gtk::graphene::Rect::new(0.0, 0.0, window.width() as f32, window.height() as f32),
        );
        window.snapshot_child(&child, &snapshot);
        snapshot.to_node().expect("native detector content renders")
    });
    if let Some(output) = output {
        std::fs::create_dir_all(&output).unwrap();
        window
            .renderer()
            .unwrap()
            .render_texture(&node, None)
            .save_to_png(std::path::Path::new(&output).join(format!("{name}.png")))
            .unwrap();
    }
}
#[test]
#[ignore = "requires an isolated display and XDG profile"]
fn live_account_selection_and_detector_layout() {
    let profile = std::env::var("XDG_DATA_HOME").expect("isolated profile");
    assert!(profile.starts_with("/tmp/drivesync-ui-test-"));
    adw::init().unwrap();
    gtk::Settings::default()
        .unwrap()
        .set_gtk_enable_animations(false);
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceLight);
    let base = std::path::PathBuf::from(profile).join("detector-fixture");
    std::fs::create_dir_all(&base).unwrap();
    let context = Context::default();
    // Independently synthetic account metadata; no real profile or keyring is consulted.
    use std::os::unix::fs::PermissionsExt;
    let account_directory = base.join("accounts");
    std::fs::create_dir(&account_directory).unwrap();
    std::fs::set_permissions(&account_directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut records = serde_json::Map::new();
    for id in ["personal@example.invalid", "work@example.invalid"] {
        records.insert(id.into(), serde_json::json!({
            "account": { "id":id, "provider":"google", "provider_user_id":format!("subject-{id}"),
                "email":id, "display_name":id, "state":"ReauthorizationRequired",
                "created_at":{"unix_millis":0}, "updated_at":{"unix_millis":0} },
            "key":format!("google:{id}:synthetic")
        }));
    }
    let account_file = account_directory.join("accounts.json");
    std::fs::write(
        &account_file,
        serde_json::to_vec(&serde_json::json!({
            "version":1, "application":"org.example.DetectorUI", "records":records, "intent":null
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::set_permissions(&account_file, std::fs::Permissions::from_mode(0o600)).unwrap();
    let engine = gtk::glib::MainContext::default()
        .block_on(Engine::open(EngineConfig {
            data_directory: base.join("accounts"),
            application_id: "org.example.DetectorUI".into(),
            google_oauth_file: None,
        }))
        .unwrap();
    for account in ["personal@example.invalid", "work@example.invalid"] {
        let root = base.join(account);
        std::fs::create_dir(&root).unwrap();
        let mut session = Orchestrator::observe_persistent(
            engine.clone(),
            LocalDetectorScope {
                account_id: AccountId(account.into()),
                sync_id: SyncId(format!("test-{account}")),
                local_root: root.clone(),
            },
            LocalDetectorSettings::default(),
            base.join("queues"),
            DeliveryState::Enabled,
        )
        .unwrap();
        session
            .attach_remote(RemoteDetectorCreateRequest {
                connector: engine.clone(),
                scope: RemoteDetectorScope {
                    account_id: AccountId(account.into()),
                    sync_id: SyncId(format!("remote-test-{account}")),
                    remote_root_id: "fixture-root".into(),
                },
                settings: RemoteDetectorSettings::default(),
                data_directory: base.join("remote-queues"),
            })
            .unwrap();
        let session = Arc::new(session);
        session
            .start(LocalStartRequest {
                mode: LocalStartMode::Initial,
                requested_at: Timestamp { unix_millis: 0 },
            })
            .unwrap();
        context.detectors.borrow_mut().insert(
            account.into(),
            detectors::Session {
                root,
                orchestrator: session,
                remote_root_id: "fixture-root".into(),
                remote_error: None,
            },
        );
    }
    let page = build_inner(&context, false);
    let window = adw::Window::builder()
        .default_width(1000)
        .default_height(720)
        .content(&page)
        .build();
    window.present();
    settle();
    let metrics = find(page.upcast_ref(), "detector-metrics")
        .unwrap()
        .downcast::<gtk::FlowBox>()
        .unwrap();
    let first = metrics.child_at_index(0).unwrap();
    let last_metric = metrics.child_at_index(3).unwrap();
    assert_eq!(
        first.allocation().y(),
        last_metric.allocation().y(),
        "wide layout uses one metric row"
    );
    let event_scroll = find(page.upcast_ref(), "detector-event-scroll").unwrap();
    assert!(
        event_scroll.height() > 360,
        "most height is available for events"
    );
    let selector = find(page.upcast_ref(), "detector-account")
        .unwrap()
        .downcast::<gtk::DropDown>()
        .unwrap();
    assert_eq!(selector.selected(), 0);
    let file =
        std::fs::File::create(base.join("personal@example.invalid/personal-only.txt")).unwrap();
    settle();
    drop(file);
    std::fs::write(
        base.join("personal@example.invalid/personal-only.txt"),
        b"one",
    )
    .unwrap();
    settle();
    std::fs::rename(
        base.join("personal@example.invalid/personal-only.txt"),
        base.join("personal@example.invalid/renamed.txt"),
    )
    .unwrap();
    settle();
    let personal = texts(page.upcast_ref());
    assert!(personal.contains("Monitoring"));
    assert!(personal.contains("personal-only.txt"));
    assert_eq!(
        find(page.upcast_ref(), "detector-pending")
            .unwrap()
            .downcast::<gtk::Label>()
            .unwrap()
            .text(),
        "0"
    );
    assert!(personal.contains("CREATE"));
    assert!(personal.contains("MODIFY"));
    assert!(personal.contains("RENAME"));
    assert!(personal.contains("personal-only.txt → renamed.txt"));
    assert!(personal.contains("Verify monitored folder"));
    assert!(personal.contains("RECHECK"));
    assert!(personal.contains("Capture started; initial verification required"));
    let session = context
        .detectors
        .borrow()
        .get("personal@example.invalid")
        .unwrap()
        .orchestrator
        .clone();
    let before_save = session.diagnostics().events_detected;
    std::fs::write(
        base.join("personal@example.invalid/ordinary-source"),
        b"atomic replacement",
    )
    .unwrap();
    std::fs::rename(
        base.join("personal@example.invalid/ordinary-source"),
        base.join("personal@example.invalid/renamed.txt"),
    )
    .unwrap();
    settle();
    let after_save = session.diagnostics();
    assert_eq!(after_save.events_detected, before_save + 1);
    assert_eq!(
        after_save.events.last().unwrap().event.kind,
        drivesync_engine::models::FileEventKind::Modified
    );
    assert!(!texts(page.upcast_ref()).contains("ordinary-source"));
    let before_gio_save = session.diagnostics().events_detected;
    gtk::glib::MainContext::default().block_on(async {
        gtk::gio::File::for_path(base.join("personal@example.invalid/renamed.txt"))
            .replace_contents_future(
                b"saved through GIO".to_vec(),
                None,
                false,
                gtk::gio::FileCreateFlags::NONE,
            )
            .await
            .unwrap();
    });
    settle();
    let after_gio_save = session.diagnostics();
    assert_eq!(
        after_gio_save.events_detected,
        before_gio_save + 1,
        "GIO replacement must produce one normalized action: {:?}",
        after_gio_save.events
    );
    assert_eq!(
        after_gio_save.events.last().unwrap().event.kind,
        drivesync_engine::models::FileEventKind::Modified
    );
    std::fs::create_dir(base.join("personal@example.invalid/new-folder")).unwrap();
    settle();
    let typed = session.diagnostics();
    assert!(typed
        .events
        .iter()
        .any(|event| event.entry_kind == Some(drivesync_engine::models::LocalEntryKind::Regular)));
    assert!(
        typed
            .events
            .iter()
            .any(|event| event.entry_kind
                == Some(drivesync_engine::models::LocalEntryKind::Directory))
    );
    let delivery_button = find(page.upcast_ref(), "detector-delivery-toggle")
        .unwrap()
        .downcast::<gtk::Button>()
        .unwrap();
    assert_eq!(
        delivery_button.icon_name().as_deref(),
        Some("media-playback-pause-symbolic")
    );
    delivery_button.emit_clicked();
    settle();
    assert_eq!(session.diagnostics().delivery, Some(DeliveryState::Paused));
    std::fs::write(
        base.join("personal@example.invalid/queued-while-paused"),
        b"durable",
    )
    .unwrap();
    settle();
    assert_eq!(session.diagnostics().pending_events, Some(1));
    assert_eq!(
        delivery_button.icon_name().as_deref(),
        Some("media-playback-start-symbolic")
    );
    capture(&window, "detector-delivery-paused");
    selector.set_selected(1);
    settle();
    assert_eq!(
        delivery_button.icon_name().as_deref(),
        Some("media-playback-pause-symbolic")
    );
    assert_eq!(
        context
            .detectors
            .borrow()
            .get("work@example.invalid")
            .unwrap()
            .orchestrator
            .diagnostics()
            .delivery,
        Some(DeliveryState::Enabled)
    );
    selector.set_selected(0);
    settle();
    delivery_button.emit_clicked();
    settle();
    assert_eq!(session.diagnostics().pending_events, Some(0));
    assert_eq!(session.diagnostics().delivery, Some(DeliveryState::Enabled));
    capture(&window, "detector-local-personal");
    let tabs = find(page.upcast_ref(), "detector-tabs")
        .unwrap()
        .downcast::<gtk::StackSwitcher>()
        .unwrap()
        .stack()
        .unwrap();
    tabs.set_visible_child_name("remote");
    settle();
    let remote = texts(page.upcast_ref());
    assert!(remote.contains("AuthorizationRequired"));
    assert!(remote.contains("Next poll:"));
    assert!(remote.contains("Interval:"));
    assert!(delivery_button.is_sensitive());
    delivery_button.emit_clicked();
    settle();
    assert_eq!(
        session.remote_diagnostics().unwrap().queue.delivery,
        DeliveryState::Paused
    );
    assert_eq!(session.diagnostics().delivery, Some(DeliveryState::Enabled));
    capture(&window, "detector-remote-paused");
    selector.set_selected(1);
    settle();
    assert_eq!(
        delivery_button.icon_name().as_deref(),
        Some("media-playback-pause-symbolic")
    );
    selector.set_selected(0);
    settle();
    delivery_button.emit_clicked();
    settle();
    assert_eq!(
        session.remote_diagnostics().unwrap().queue.delivery,
        DeliveryState::Enabled
    );
    tabs.set_visible_child_name("local");
    settle();
    selector.set_selected(1);
    settle();
    assert!(!texts(page.upcast_ref()).contains("personal-only.txt"));
    std::fs::write(base.join("work@example.invalid/work-only.txt"), b"two").unwrap();
    settle();
    assert!(texts(page.upcast_ref()).contains("work-only.txt"));
    selector.set_selected(0);
    settle();
    assert!(!texts(page.upcast_ref()).contains("work-only.txt"));
    // A mapped window may keep its compositor-chosen size despite a new default.
    window.set_content(None::<&gtk::Widget>);
    let narrow = adw::Window::builder()
        .default_width(360)
        .default_height(640)
        .content(&page)
        .build();
    narrow.present();
    settle();
    capture(&narrow, "detector-local-narrow");
    assert!(
        metrics.child_at_index(3).unwrap().allocation().y()
            > metrics.child_at_index(0).unwrap().allocation().y(),
        "narrow layout wraps metrics"
    );
    assert!(
        event_scroll.height() > 180,
        "narrow layout retains event space"
    );
    assert!(
        page.width() <= 360,
        "page fits narrow window: actual width {}",
        page.width()
    );
    tabs.set_visible_child_name("remote");
    settle();
    assert!(page.width() <= 360);
    assert!(
        event_scroll.height() > 180,
        "remote diagnostics remain compact when collapsed"
    );
    capture(&narrow, "detector-remote-narrow");
    let details = find(page.upcast_ref(), "detector-polling-diagnostics")
        .unwrap()
        .downcast::<gtk::Expander>()
        .unwrap();
    assert!(!details.is_expanded());
    details.set_expanded(true);
    settle();
    assert!(texts(page.upcast_ref()).contains("Duration:"));
    details.set_expanded(false);
    tabs.set_visible_child_name("local");
    let first = context
        .detectors
        .borrow()
        .get("personal@example.invalid")
        .unwrap()
        .orchestrator
        .clone();
    gtk::glib::MainContext::default().block_on(detectors::remove(
        &context,
        &AccountId("personal@example.invalid".into()),
    ));
    assert_eq!(
        first.status(LocalStatusRequest {}).unwrap().state,
        LocalDetectorState::Stopped
    );
    assert_eq!(
        context
            .detectors
            .borrow()
            .get("work@example.invalid")
            .unwrap()
            .orchestrator
            .status(LocalStatusRequest {})
            .unwrap()
            .state,
        LocalDetectorState::Running
    );
    narrow.destroy();
    window.destroy();
    gtk::glib::MainContext::default().block_on(detectors::remove(
        &context,
        &AccountId("work@example.invalid".into()),
    ));
    drop(first);
    drop(context);
    drop(engine);
    std::fs::remove_dir_all(base).unwrap();
}
