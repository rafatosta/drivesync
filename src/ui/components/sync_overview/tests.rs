//! Isolated GTK contract checks and visual capture.
use super::*;

#[test]
#[ignore = "Requires an isolated GTK display"]
fn visual_contract() {
    unsafe {
        gettextrs::setlocale(gettextrs::LocaleCategory::LcAll, "");
    }
    if let Ok(path) = std::env::var("DRIVESYNC_PREVIEW_LOCALEDIR") {
        gettextrs::bindtextdomain("drivesync", path).unwrap();
        gettextrs::textdomain("drivesync").unwrap();
    }
    adw::init().unwrap();
    let view = SyncOverview::new();
    let window = gtk::Window::builder()
        .default_width(480)
        .child(view.widget())
        .build();
    window.present();
    let original = view.root.first_child().unwrap();
    let calls = Rc::new(std::cell::Cell::new(0));
    let received = calls.clone();
    view.connect_attention_requested(move || received.set(received.get() + 1));
    let states = [
        SyncOverviewState::Checking {
            checked_items: Some(2480),
        },
        SyncOverviewState::Syncing {
            completed_items: 1248,
            total_items: Some(1560),
            remaining_items: Some(312),
            remaining_bytes: Some(840_000_000),
            progress: Some(0.8),
        },
        SyncOverviewState::Completed {
            last_sync: Some(SystemTime::now()),
        },
        SyncOverviewState::Attention {
            pending_items: Some(3),
        },
    ];
    for dark in [false, true] {
        adw::StyleManager::default().set_color_scheme(if dark {
            adw::ColorScheme::ForceDark
        } else {
            adw::ColorScheme::ForceLight
        });
        for (i, state) in states.iter().enumerate() {
            view.set_state(state.clone());
            assert_eq!(view.root.first_child().unwrap(), original);
            assert_eq!(view.tick.borrow().is_some(), i == 0);
            assert_eq!(view.progress.is_visible(), i < 2);
            assert_eq!(view.action.is_visible(), i == 3);
            if i == 1 {
                assert_eq!(view.progress.fraction(), 0.8);
            }
            let until = std::time::Instant::now() + std::time::Duration::from_millis(300);
            while std::time::Instant::now() < until {
                while glib::MainContext::default().pending() {
                    glib::MainContext::default().iteration(false);
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            let paintable = gtk::WidgetPaintable::new(Some(&window));
            let snapshot = gtk::Snapshot::new();
            paintable.snapshot(&snapshot, window.width() as f64, window.height() as f64);
            let node = snapshot.to_node().unwrap();
            let texture = window.renderer().unwrap().render_texture(&node, None);
            texture
                .save_to_png(format!(
                    "{}/_build/sync-overview-{}-{i}.png",
                    env!("CARGO_MANIFEST_DIR"),
                    if dark { "dark" } else { "light" }
                ))
                .unwrap();
        }
    }
    view.action.emit_clicked();
    assert_eq!(calls.get(), 1);
    for progress in [
        None,
        Some(f64::NAN),
        Some(f64::INFINITY),
        Some(0.0),
        Some(1.0),
        Some(-1.0),
        Some(2.0),
    ] {
        view.set_state(SyncOverviewState::Syncing {
            completed_items: 0,
            total_items: Some(0),
            remaining_items: Some(0),
            remaining_bytes: Some(0),
            progress,
        });
        assert_eq!(
            view.tick.borrow().is_some(),
            progress.is_none_or(|n| !n.is_finite())
        );
        assert!(!view.details.text().is_empty());
    }
    view.set_state(SyncOverviewState::Checking {
        checked_items: None,
    });
    assert!(!view.detail_revealer.reveals_child());
    view.set_state(SyncOverviewState::Checking {
        checked_items: Some(0),
    });
    assert!(view.detail_revealer.reveals_child());
    view.set_state(SyncOverviewState::Completed { last_sync: None });
    assert!(view.tick.borrow().is_none());
    assert!(!view.detail_revealer.reveals_child());
    window.close();
}
