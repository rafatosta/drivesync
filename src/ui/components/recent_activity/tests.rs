//! Isolated preview truncation, ordering, navigation and live snapshot replacement.
use super::*;
#[test]
#[ignore = "requires an isolated display"]
fn recent_activity_preview_and_history() {
    adw::init().unwrap();
    let nav = adw::NavigationView::new();
    let home = adw::NavigationPage::new(&gtk::Box::new(gtk::Orientation::Vertical, 0), "Home");
    nav.add(&home);
    let recent = RecentActivity::new(&nav);
    assert!(recent.empty.is_visible());
    assert!(!recent.more.is_sensitive());
    let items: Vec<_> = (0..5)
        .map(|index| ActivityItem {
            id: format!("account/{index}"),
            name: format!("<file {index}>"),
            path: "Documents/file".into(),
            account: "Test account".into(),
            kind: ActivityKind::Uploaded,
            bytes: if index == 0 { None } else { Some(0) },
            completed_at: 100 + index,
        })
        .collect();
    recent.update(&items, 200);
    assert!(!recent.empty.is_visible());
    assert!(recent.more.is_sensitive());
    assert_eq!(models::ordered(&items)[0].id, "account/4");
    assert!(recent.preview.row_at_index(2).is_some());
    assert!(recent.preview.row_at_index(3).is_none());
    let first = recent
        .preview
        .row_at_index(0)
        .unwrap()
        .downcast::<adw::ActionRow>()
        .unwrap();
    assert_eq!(first.title(), "<file 4>");
    assert!(!first.uses_markup());
    recent.more.emit_clicked();
    assert_eq!(
        nav.visible_page().unwrap().tag().as_deref(),
        Some("activity-history")
    );
    assert!(recent.history.row_at_index(4).is_some());
    let detail = recent
        .history
        .row_at_index(0)
        .unwrap()
        .downcast::<adw::ActionRow>()
        .unwrap();
    assert!(detail.subtitle().unwrap().contains("Test account"));
    assert!(detail.subtitle().unwrap().contains("Documents/file"));
    recent.update(&items[..1], 200);
    assert!(recent.history.row_at_index(1).is_none());
    recent.update(&[], 200);
    assert!(recent.history_empty.is_visible());
    assert!(recent.empty.is_visible());
    nav.pop();
    assert_eq!(nav.visible_page(), Some(home));
    let mut tied = items[..2].to_vec();
    tied[1].completed_at = tied[0].completed_at;
    assert_eq!(models::ordered(&tied)[0].id, "account/0");
}
