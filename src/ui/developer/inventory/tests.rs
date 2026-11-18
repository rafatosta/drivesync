//! Real SQLite table inspection on an isolated GTK display and profile.
use super::*;
use drivesync_engine::{inventory::*, models as m};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
fn settle(view: &View) {
    let loop_ = gtk::glib::MainLoop::new(None, false);
    let done = loop_.clone();
    let started = Instant::now();
    let weak = view.root.clone();
    gtk::glib::timeout_add_local_once(Duration::from_millis(150), move || {
        let _ = weak;
        done.quit();
    });
    loop_.run();
    assert!(started.elapsed() < Duration::from_secs(5));
}
fn wait(view: &View) {
    let context = gtk::glib::MainContext::default();
    let start = Instant::now();
    while view.state.borrow().loading && start.elapsed() < Duration::from_secs(5) {
        context.iteration(false);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!view.state.borrow().loading, "inspection did not complete");
    settle(view);
}
fn seed(root: &std::path::Path, count: usize) {
    std::fs::create_dir_all(root).unwrap();
    let inventory = InventoryManager::open(OpenInventoryRequest {
        local_root: root.to_owned(),
        expected: SyncIdentity {
            sync_id: m::SyncId("inventory-fixture".into()),
            account_id: m::AccountId("account-a".into()),
            remote_root_id: "root-a".into(),
        },
        create_if_missing: true,
    })
    .unwrap()
    .inventory;
    for n in 0..count {
        let reference = m::RemoteFileRef {
            account_id: m::AccountId("account-a".into()),
            file_id: m::RemoteFileId(format!("remote-{n}")),
        };
        let remote = m::RemoteFile {
            reference: reference.clone(),
            provider: m::ProviderId("google".into()),
            name: m::MetadataValue::Known(format!("Example document {n:03}.pdf")),
            kind: m::MetadataValue::Known(m::RemoteEntryKind::File),
            mime_type: m::MetadataValue::Known("application/pdf".into()),
            parent_ids: m::MetadataValue::Known(vec![m::RemoteFileId("root-a".into())]),
            trashed: m::MetadataValue::Known(false),
            version: m::MetadataValue::Known("42".into()),
            created_time: m::MetadataValue::NotCollected,
            modified_time: m::MetadataValue::NotCollected,
            size: m::MetadataValue::Known(2048),
            md5_checksum: m::MetadataValue::NotCollected,
            sha256_checksum: m::MetadataValue::NotCollected,
            provider_data: m::MetadataValue::NotCollected,
        };
        let time = m::Timestamp { unix_millis: 10 };
        let observed = m::File {
            id: m::FileId(format!("event-{n}")),
            causal_order: None,
            sync_id: m::SyncId("inventory-fixture".into()),
            observed_at: time,
            scope: m::FileScope::Remote(reference.clone()),
            coverage: m::ObservationCoverage::Partial,
            event: m::FileEvent {
                action: m::FileEventAction::StateChanged,
                local: None,
                local_presence: None,
                remote: Some(m::RemoteObservation {
                    source: m::RemoteObservationSource::Changes,
                    removed: Some(false),
                    time: None,
                    change_type: None,
                    drive_id: None,
                }),
                uncertainty: None,
                previous: None,
                current: Some(m::FileLocation::Remote {
                    reference,
                    parent_id: Some(m::RemoteFileId("root-a".into())),
                    name: Some(format!("Example document {n:03}.pdf")),
                }),
            },
            data: Some(m::FileData::Remote(remote)),
        };
        inventory
            .record_remote(RecordRemoteObservation {
                selector: ObservationTarget::NewItem,
                expected_revision: None,
                observation: RemoteObservation {
                    observed,
                    presence: PresenceState::Present,
                    proof: ObservationProof {
                        source: ObservationSource::RemoteChange,
                        observed_at: time,
                        producer_generation: None,
                        complete_scope: false,
                    },
                },
            })
            .unwrap();
    }
}
fn capture(window: &adw::Window, name: &str) {
    let Some(output) = std::env::var_os("DRIVESYNC_UI_PREVIEW_DIR") else {
        return;
    };
    let snapshot = gtk::Snapshot::new();
    gtk::WidgetPaintable::new(Some(window)).snapshot(
        &snapshot,
        window.width() as f64,
        window.height() as f64,
    );
    let node = snapshot.to_node().expect("inventory window renders");
    std::fs::create_dir_all(&output).unwrap();
    window
        .renderer()
        .unwrap()
        .render_texture(&node, None)
        .save_to_png(PathBuf::from(output).join(format!("{name}.png")))
        .unwrap();
}
#[test]
#[ignore = "requires an isolated display and XDG profile"]
fn inventory_table_selection_pagination_and_account_isolation() {
    let profile = std::env::var("XDG_DATA_HOME").expect("isolated profile");
    assert!(profile.starts_with("/tmp/drivesync-ui-test-"));
    adw::init().unwrap();
    gtk::Settings::default()
        .unwrap()
        .set_gtk_enable_animations(false);
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceLight);
    let base = PathBuf::from(profile).join("inventory-fixture");
    let account_a = base.join("account-a");
    seed(&account_a, 101);
    let account_b = base.join("account-b");
    std::fs::create_dir_all(&account_b).unwrap();
    let (root, view) = create(&Context::default());
    let request = inspection::InspectInventoryRequest {
        local_root: account_a.clone(),
        account_id: m::AccountId("account-a".into()),
        remote_root_id: "root-a".into(),
        table: InventoryTable::Items,
        after_rowid: 0,
    };
    *view.state.borrow_mut() = State {
        scopes: vec![
            ConfiguredInventory {
                account_id: "account-a".into(),
                label: "Personal · example-a@example.test".into(),
                request: Ok(request.clone()),
            },
            ConfiguredInventory {
                account_id: "account-b".into(),
                label: "Work · example-b@example.test".into(),
                request: Ok(inspection::InspectInventoryRequest {
                    local_root: account_b.clone(),
                    account_id: m::AccountId("account-b".into()),
                    remote_root_id: "root-b".into(),
                    ..request
                }),
            },
        ],
        ..Default::default()
    };
    view.accounts.set_model(Some(&gtk::StringList::new(&[
        "Personal · example-a@example.test",
        "Work · example-b@example.test",
    ])));
    view.accounts.set_selected(0);
    view.reset();
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new(
        "Inventory",
        "Developer Console",
    )));
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&root));
    let window = adw::Window::builder()
        .default_width(1100)
        .default_height(780)
        .content(&toolbar)
        .build();
    window.present();
    wait(&view);
    assert_eq!(view.store.n_items(), 100);
    assert!(view.next.is_sensitive());
    assert!(!view.previous.is_sensitive());
    view.next.emit_clicked();
    wait(&view);
    assert_eq!(view.store.n_items(), 1);
    assert!(view.previous.is_sensitive());
    assert!(!view.next.is_sensitive());
    view.previous.emit_clicked();
    wait(&view);
    assert_eq!(view.store.n_items(), 100);
    view.tables.set_selected(3);
    wait(&view);
    assert_eq!(view.state.borrow().columns, vec!["item", "provider"]);
    assert!(view
        .detail
        .buffer()
        .text(
            &view.detail.buffer().start_iter(),
            &view.detail.buffer().end_iter(),
            false
        )
        .contains("google"));
    capture(&window, "inventory-remote-files");
    view.tables.set_selected(5);
    wait(&view);
    assert!(view.state.borrow().columns.contains(&"latest".to_owned()));
    view.grid.emit_by_name::<()>("activate", &[&0_u32]);
    settle(&view);
    capture(&window, "inventory-metadata");
    view.tables.set_selected(7);
    wait(&view);
    assert_eq!(view.store.n_items(), 0);
    assert_eq!(view.status.text(), "This table is empty.");
    // Rapid selection changes must never publish a late response for another scope/table.
    view.tables.set_selected(1);
    view.accounts.set_selected(1);
    wait(&view);
    assert_eq!(view.store.n_items(), 0);
    assert!(view.status.text().contains("has not been created"));
    assert!(!account_b.join(".drivesync").exists());
    capture(&window, "inventory-missing");
    view.accounts.set_selected(0);
    view.tables.set_selected(3);
    wait(&view);
    window.set_default_size(520, 780);
    settle(&view);
    assert!(
        window.width() <= 540,
        "narrow view expands unexpectedly: {}",
        window.width()
    );
    capture(&window, "inventory-narrow");
    let weak = Rc::downgrade(&view);
    view.tables.set_selected(1);
    drop(view);
    window.close();
    drop(window);
    drop(root);
    drop(toolbar);
    drop(header);
    let context = gtk::glib::MainContext::default();
    for _ in 0..20 {
        context.iteration(false);
    }
    assert!(
        weak.upgrade().is_none(),
        "view leaked after the page was destroyed"
    );
}
