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
        let session = Arc::new(
            Orchestrator::observe(
                engine.clone(),
                LocalDetectorScope {
                    account_id: AccountId(account.into()),
                    sync_id: SyncId(format!("test-{account}")),
                    local_root: root.clone(),
                },
                LocalDetectorSettings::default(),
            )
            .unwrap(),
        );
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
    assert!(personal.contains("Unavailable"));
    assert!(personal.contains("CREATE"));
    assert!(personal.contains("MODIFY"));
    assert!(personal.contains("RENAME"));
    assert!(personal.contains("personal-only.txt → renamed.txt"));
    assert!(personal.contains("Verify monitored folder"));
    assert!(personal.contains("RECHECK"));
    capture(&window, "detector-local-personal");
    selector.set_selected(1);
    settle();
    assert!(!texts(page.upcast_ref()).contains("personal-only.txt"));
    std::fs::write(base.join("work@example.invalid/work-only.txt"), b"two").unwrap();
    settle();
    assert!(texts(page.upcast_ref()).contains("work-only.txt"));
    selector.set_selected(0);
    settle();
    assert!(!texts(page.upcast_ref()).contains("work-only.txt"));
    window.set_default_size(360, 640);
    settle();
    capture(&window, "detector-local-narrow");
    assert!(
        page.width() <= 360,
        "page fits narrow window: actual width {}",
        page.width()
    );
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
