//! Isolated review navigation and intent delivery, without engine operations.
use super::*;
use std::cell::RefCell;
#[test]
#[ignore = "requires an isolated display"]
fn alerts_navigation_and_single_category_layout() {
    adw::init().unwrap();
    let navigation = adw::NavigationView::new();
    let home = adw::NavigationPage::new(&gtk::Box::new(gtk::Orientation::Vertical, 0), "Home");
    navigation.add(&home);
    let received = Rc::new(RefCell::new(Vec::new()));
    let output = received.clone();
    let alerts = Alerts::new(&navigation, move |id, action| {
        output.borrow_mut().push((id.to_owned(), action.to_owned()))
    });
    let conflict = AlertItem {
        id: "account/conflict".into(),
        kind: AlertKind::Conflict,
        title: "<b>literal</b>".into(),
        detail: "Computer and Drive".into(),
        actions: vec![AlertAction {
            id: "both".into(),
            label: "Keep both".into(),
        }],
    };
    let problem = AlertItem {
        kind: AlertKind::Problem,
        id: "account/problem".into(),
        ..conflict.clone()
    };
    alerts.update(&[conflict.clone(), problem.clone()]);
    assert!(alerts.conflicts.button.is_visible());
    assert!(alerts.problems.button.is_visible());
    assert!(alerts.widget().is_homogeneous());
    assert!(alerts.conflicts.button.hexpands());
    assert!(alerts.problems.button.hexpands());
    alerts.conflicts.button.emit_clicked();
    assert_eq!(
        navigation.visible_page().unwrap().tag().as_deref(),
        Some("alert-conflicts")
    );
    let row = alerts
        .conflicts
        .list
        .row_at_index(0)
        .unwrap()
        .downcast::<adw::ExpanderRow>()
        .unwrap();
    assert!(!row.uses_markup());
    row.set_expanded(true);
    // Locate the rendered action without coupling to libadwaita's private layout.
    fn click_action(widget: &gtk::Widget) -> bool {
        if let Some(button) = widget.downcast_ref::<gtk::Button>() {
            if button.label().as_deref() == Some("Keep both") {
                button.emit_clicked();
                return true;
            }
        }
        let mut child = widget.first_child();
        while let Some(w) = child {
            if click_action(&w) {
                return true;
            }
            child = w.next_sibling();
        }
        false
    }
    assert!(click_action(row.upcast_ref()));
    assert_eq!(
        *received.borrow(),
        vec![("account/conflict".into(), "both".into())]
    );
    alerts.update(&[problem]);
    assert!(!alerts.conflicts.button.is_visible());
    assert!(alerts.problems.button.is_visible());
    assert!(alerts.conflicts.empty.is_visible());
    navigation.pop();
    alerts.problems.button.emit_clicked();
    assert_eq!(
        navigation.visible_page().unwrap().tag().as_deref(),
        Some("alert-problems")
    );
    alerts.update(&[conflict]);
    assert!(alerts.conflicts.button.is_visible());
    assert!(!alerts.problems.button.is_visible());
    alerts.update(&[]);
    assert!(!alerts.widget().is_visible());
    assert!(alerts.problems.empty.is_visible());
    navigation.pop();
    assert_eq!(navigation.visible_page(), Some(home));
}
