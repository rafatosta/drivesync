use super::*;
use drivesync_engine::{LocalItemKind, LocalNotice, LocalScope};
use std::{
    path::{Path, PathBuf},
    time::Instant,
};
fn find(widget: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    if widget.widget_name() == name {
        return Some(widget.clone());
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Some(found) = find(&current, name) {
            return Some(found);
        }
        child = current.next_sibling();
    }
    None
}
fn wait(mut condition: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(4);
    while !condition() {
        assert!(Instant::now() < end, "UI observation timed out");
        gtk::glib::MainContext::default().iteration(false);
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn titles(list: &gtk::ListBox) -> Vec<String> {
    let mut result = Vec::new();
    let mut index = 0;
    while let Some(row) = list.row_at_index(index) {
        result.push(
            row.downcast::<adw::ActionRow>()
                .unwrap()
                .title()
                .to_string(),
        );
        index += 1;
    }
    result
}
#[test]
#[ignore = "requires a desktop display and isolated XDG_DATA_HOME"]
fn history_observes_real_changes_switches_roots_and_bounds_rows() {
    adw::init().unwrap();
    let data = PathBuf::from(std::env::var_os("XDG_DATA_HOME").unwrap());
    assert!(
        data.starts_with("/tmp")
            && data
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("drivesync-ui-")
    );
    let first = data.join("activity-one");
    let second = data.join("activity-two");
    std::fs::create_dir_all(&first).unwrap();
    std::fs::create_dir_all(&second).unwrap();
    let selected = Rc::new(RefCell::new(first.clone()));
    let selection = selected.clone();
    let history = LocalHistory::build(move || {
        let scope = LocalScope::new("fixture", "account", &selection.borrow()).unwrap();
        Observation::start(move || Ok(Some(scope)))
    });
    let list = find(history.widget().upcast_ref(), "local-activity-list")
        .unwrap()
        .downcast::<gtk::ListBox>()
        .unwrap();
    let status = find(history.widget().upcast_ref(), "local-activity-status")
        .unwrap()
        .downcast::<gtk::Label>()
        .unwrap();
    let empty = find(history.widget().upcast_ref(), "local-activity-empty")
        .unwrap()
        .downcast::<gtk::Label>()
        .unwrap();
    let window = adw::Window::builder()
        .default_width(600)
        .default_height(430)
        .build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    for set in [
        gtk::Widget::set_margin_top,
        gtk::Widget::set_margin_bottom,
        gtk::Widget::set_margin_start,
        gtk::Widget::set_margin_end,
    ] {
        set(content.upcast_ref(), 24);
    }
    content.append(history.widget());
    window.set_content(Some(&content));
    window.present();
    wait(|| status.text() == gettextrs::gettext("Observing local changes."));
    std::fs::write(first.join("documento.txt"), b"preserved").unwrap();
    wait(|| titles(&list).iter().any(|t| t == "documento.txt"));
    assert!(list
        .row_at_index(0)
        .unwrap()
        .downcast::<adw::ActionRow>()
        .unwrap()
        .subtitle()
        .unwrap()
        .starts_with(&gettext("File")));

    std::fs::rename(first.join("documento.txt"), first.join("renomeado.txt")).unwrap();
    wait(|| titles(&list).first().is_some_and(|t| t == "renomeado.txt"));
    assert!(!empty.is_visible());
    let first_row = list
        .row_at_index(0)
        .unwrap()
        .downcast::<adw::ActionRow>()
        .unwrap();
    assert!(first_row.subtitle().unwrap().contains(&gettext("Renamed")));

    if let Some(output) = std::env::var_os("DRIVESYNC_HISTORY_SCREENSHOT") {
        wait(|| window.width() > 0 && window.is_mapped());
        let frame = gtk::glib::MainLoop::new(None, false);
        let finish = frame.clone();
        gtk::glib::timeout_add_local_once(Duration::from_millis(200), move || finish.quit());
        frame.run();
        let snapshot = gtk::Snapshot::new();
        gtk::WidgetPaintable::new(Some(&window)).snapshot(
            &snapshot,
            window.width() as f64,
            window.height() as f64,
        );
        window
            .renderer()
            .unwrap()
            .render_texture(snapshot.to_node().expect("rendered history"), None)
            .save_to_png(Path::new(&output))
            .unwrap();
    }
    *selected.borrow_mut() = second.clone();
    (history.restart_callback())();
    assert!(titles(&list).is_empty());
    wait(|| status.text() == gettextrs::gettext("Observing local changes."));
    std::fs::write(first.join("old-root.txt"), b"old").unwrap();
    std::fs::write(second.join("new-root.txt"), b"new").unwrap();
    wait(|| titles(&list).iter().any(|t| t == "new-root.txt"));
    assert!(!titles(&list).iter().any(|t| t == "old-root.txt"));
    let scope = LocalScope::new("fixture", "account", &second).unwrap();
    let batch = LocalBatch {
        scope,
        state: LocalWatchState::Incomplete,
        notices: (0..205)
            .map(|i| LocalNotice {
                path: format!("file-{i}").into(),
                object_type: LocalItemKind::Unknown,
                activity: LocalActivity::Created,
            })
            .collect(),
    };
    render(&list, &empty, &status, &batch);
    let rows = titles(&list);
    assert_eq!(rows.len(), 200);
    assert_eq!(rows[0], "file-204");
    assert_eq!(rows[199], "file-5");
    assert!(status.has_css_class("warning"));
    assert_eq!(
        std::fs::read(first.join("renomeado.txt")).unwrap(),
        b"preserved"
    );
    let weak = history.widget().downgrade();
    content.remove(history.widget());
    drop(history);
    wait(|| weak.upgrade().is_none());
    window.close();
}
#[test]
#[ignore = "requires a desktop display and isolated XDG_DATA_HOME"]
fn dashboard_observes_both_accounts_without_selection_or_network() {
    adw::init().unwrap();
    let data = PathBuf::from(std::env::var_os("XDG_DATA_HOME").unwrap());
    assert!(
        data.starts_with("/tmp")
            && data
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("drivesync-ui-")
    );
    let client = "history-ui.apps.googleusercontent.com";
    std::env::set_var("DRIVESYNC_GOOGLE_CLIENT_ID", client);
    let metadata = data.join("drivesync");
    std::fs::create_dir_all(&metadata).unwrap();
    let mut saved = Vec::new();
    for (subject, name) in [
        ("history-a", "Primeira conta"),
        ("history-b", "Segunda conta"),
    ] {
        let root = data.join(subject);
        std::fs::create_dir_all(&root).unwrap();
        saved.push(drivesync_engine::RestoredAccount {
            account: drivesync_engine::AccountSummary {
                id: AccountId(format!("google:{subject}")),
                provider_subject: subject.into(),
                email: Some(name.into()),
            },
            error: None,
            folder: Some(drivesync_engine::FolderConfiguration {
                account_id: AccountId(format!("google:{subject}")),
                remote: drivesync_engine::DriveFolder {
                    id: format!("remote-{subject}"),
                    name: "Test".into(),
                },
                local: root.clone(),
                local_display: root,
            }),
        });
    }
    let profiles: Vec<_> = saved
        .iter()
        .map(|s| {
            serde_json::json!({"id":s.account.id.0,
        "provider_subject":s.account.provider_subject,"email":s.account.email})
        })
        .collect();
    let folders: Vec<_> = saved.iter().map(|s| { let f = s.folder.as_ref().unwrap();
        serde_json::json!({"account_id":f.account_id.0,"remote":{"id":f.remote.id,"name":f.remote.name},
            "local":f.local,"local_display":f.local_display}) }).collect();
    std::fs::write(
        metadata.join("accounts.json"),
        serde_json::to_vec(&serde_json::json!(
        {"version":2,"clients":{client:profiles},"folders":{client:folders}}))
        .unwrap(),
    )
    .unwrap();
    let nav = adw::NavigationView::new();
    let toasts = adw::ToastOverlay::new();
    let page = crate::features::dashboard::build(
        Arc::new(DriveSyncEngine::new()),
        saved.clone(),
        &nav,
        &toasts,
    );
    let details = find(page.upcast_ref(), "account-details")
        .unwrap()
        .downcast::<gtk::Stack>()
        .unwrap();
    let content_a = details.child_by_name(&saved[0].account.id.0).unwrap();
    let content_b = details.child_by_name(&saved[1].account.id.0).unwrap();
    let list_a = find(&content_a, "local-activity-list")
        .unwrap()
        .downcast::<gtk::ListBox>()
        .unwrap();
    let list_b = find(&content_b, "local-activity-list")
        .unwrap()
        .downcast::<gtk::ListBox>()
        .unwrap();
    let status_a = find(&content_a, "local-activity-status")
        .unwrap()
        .downcast::<gtk::Label>()
        .unwrap();
    let status_b = find(&content_b, "local-activity-status")
        .unwrap()
        .downcast::<gtk::Label>()
        .unwrap();
    let window = adw::Window::builder()
        .default_width(960)
        .default_height(700)
        .build();
    window.set_content(Some(&page));
    window.present();
    wait(|| {
        status_a.text() == gettextrs::gettext("Observing local changes.")
            && status_b.text() == gettextrs::gettext("Observing local changes.")
    });
    std::fs::write(
        saved[0].folder.as_ref().unwrap().local.join("primeiro.txt"),
        b"a",
    )
    .unwrap();
    std::fs::write(
        saved[1].folder.as_ref().unwrap().local.join("segundo.txt"),
        b"b",
    )
    .unwrap();
    wait(|| {
        titles(&list_a).iter().any(|n| n == "primeiro.txt")
            && titles(&list_b).iter().any(|n| n == "segundo.txt")
    });
    assert!(!titles(&list_a).iter().any(|n| n == "segundo.txt"));
    assert!(!titles(&list_b).iter().any(|n| n == "primeiro.txt"));
    if let Some(output) = std::env::var_os("DRIVESYNC_HISTORY_SCREENSHOT") {
        let frame = gtk::glib::MainLoop::new(None, false);
        let finish = frame.clone();
        gtk::glib::timeout_add_local_once(Duration::from_millis(250), move || finish.quit());
        frame.run();
        let snapshot = gtk::Snapshot::new();
        gtk::WidgetPaintable::new(Some(&window)).snapshot(
            &snapshot,
            window.width() as f64,
            window.height() as f64,
        );
        window
            .renderer()
            .unwrap()
            .render_texture(snapshot.to_node().expect("rendered dashboard"), None)
            .save_to_png(Path::new(&output).with_file_name("local-history-dashboard.png"))
            .unwrap();
    }
    window.set_content(None::<&gtk::Widget>);
    window.close();
}

#[test]
fn move_labels_distinguish_name_and_parent_changes_without_content_claims() {
    for (from, to, expected) in [
        ("a/old", "a/new", "Renamed"),
        ("a/name", "b/name", "Moved"),
        ("a/old", "b/new", "Moved and renamed"),
    ] {
        let notice = LocalNotice {
            path: to.into(),
            object_type: LocalItemKind::Unknown,
            activity: LocalActivity::Moved { from: from.into() },
        };
        assert_eq!(activity_label(&notice), gettext(expected));
    }
    let notice = LocalNotice {
        path: "file".into(),
        object_type: LocalItemKind::File,
        activity: LocalActivity::Written,
    };
    assert_eq!(activity_label(&notice), gettext("Write detected"));
}
