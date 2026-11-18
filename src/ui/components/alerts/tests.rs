//! Isolated review, confirmation, cancellation and stale-intent protection.
use super::*;
use std::cell::RefCell;
fn find(widget: &gtk::Widget, name: &str) -> gtk::Widget {
    fn search(widget: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
        if widget.widget_name() == name {
            return Some(widget.clone());
        }
        let mut child = widget.first_child();
        while let Some(w) = child {
            if let Some(found) = search(&w, name) {
                return Some(found);
            }
            child = w.next_sibling();
        }
        None
    }
    search(widget, name).unwrap_or_else(|| panic!("Missing widget {name}"))
}
fn button(nav: &adw::NavigationView, name: &str) -> gtk::Button {
    find(nav.visible_page().unwrap().upcast_ref(), name)
        .downcast()
        .unwrap()
}
#[test]
#[ignore = "requires an isolated display"]
fn alerts_navigation_and_single_category_layout() {
    adw::init().unwrap();
    let nav = adw::NavigationView::new();
    let home = adw::NavigationPage::new(&gtk::Box::new(gtk::Orientation::Vertical, 0), "Home");
    nav.add(&home);
    let received = Rc::new(RefCell::new(Vec::new()));
    let output = received.clone();
    let alerts = Alerts::new(&nav, move |id, action| {
        output.borrow_mut().push((id.to_owned(), action.to_owned()))
    });
    let conflict = AlertItem {
        id: "account/conflict".into(),
        account: "Marina · Pessoal".into(),
        title: "<b>literal</b>".into(),
        detail: "Changed on both sides".into(),
        details: AlertDetails::Conflict {
            local: FileVersion {
                modified: None,
                bytes: None,
            },
            remote: FileVersion {
                modified: Some("10:42".into()),
                bytes: Some(2_600_000),
            },
        },
    };
    let problem = AlertItem {
        id: "account/problem".into(),
        details: AlertDetails::Problem(ProblemCategory::Permission),
        ..conflict.clone()
    };
    alerts.update(&[conflict.clone(), problem.clone()]);
    assert!(alerts.conflicts.button.is_visible());
    assert!(alerts.problems.button.is_visible());
    assert!(alerts.widget().is_homogeneous());
    alerts.conflicts.button.emit_clicked();
    let open = || {
        alerts
            .conflicts
            .list
            .row_at_index(0)
            .unwrap()
            .downcast::<adw::ActionRow>()
            .unwrap()
            .emit_by_name::<()>("activated", &[])
    };
    open();
    assert_eq!(
        nav.visible_page().unwrap().tag().as_deref(),
        Some("alert-review")
    );
    assert!(
        find(nav.visible_page().unwrap().upcast_ref(), "resolve-both")
            .downcast::<gtk::CheckButton>()
            .unwrap()
            .is_active()
    );
    button(&nav, "review-cancel").emit_clicked();
    assert!(received.borrow().is_empty());
    for action in ["both", "local", "remote"] {
        open();
        find(
            nav.visible_page().unwrap().upcast_ref(),
            &format!("resolve-{action}"),
        )
        .downcast::<gtk::CheckButton>()
        .unwrap()
        .set_active(true);
        let confirm = button(&nav, "review-confirm");
        confirm.emit_clicked();
        confirm.emit_clicked();
        assert_eq!(
            received.borrow().last().unwrap(),
            &(conflict.id.clone(), action.to_owned())
        );
    }
    assert_eq!(received.borrow().len(), 3);
    open();
    let stale = button(&nav, "review-confirm");
    alerts.update(std::slice::from_ref(&problem));
    stale.emit_clicked();
    assert_eq!(received.borrow().len(), 3);
    assert!(!alerts.conflicts.button.is_visible());
    assert!(alerts.problems.button.is_visible());
    assert!(alerts.conflicts.empty.is_visible());
    nav.pop();
    alerts.problems.button.emit_clicked();
    for (category, expected) in [
        (ProblemCategory::Authentication, "reconnect"),
        (ProblemCategory::Permission, "permissions"),
        (ProblemCategory::Storage, "storage"),
        (ProblemCategory::LocalFolder, "folder"),
    ] {
        alerts.update(&[AlertItem {
            details: AlertDetails::Problem(category),
            ..problem.clone()
        }]);
        alerts
            .problems
            .list
            .row_at_index(0)
            .unwrap()
            .downcast::<adw::ActionRow>()
            .unwrap()
            .emit_by_name::<()>("activated", &[]);
        button(&nav, "review-confirm").emit_clicked();
        assert_eq!(
            received.borrow().last().unwrap(),
            &(problem.id.clone(), expected.into())
        );
    }
    alerts
        .problems
        .list
        .row_at_index(0)
        .unwrap()
        .downcast::<adw::ActionRow>()
        .unwrap()
        .emit_by_name::<()>("activated", &[]);
    nav.pop();
    assert_eq!(received.borrow().len(), 7);
    alerts.update(&[AlertItem {
        details: AlertDetails::Problem(ProblemCategory::Synchronization),
        ..problem.clone()
    }]);
    alerts
        .problems
        .list
        .row_at_index(0)
        .unwrap()
        .downcast::<adw::ActionRow>()
        .unwrap()
        .emit_by_name::<()>("activated", &[]);
    assert!(!button(&nav, "review-confirm").is_visible());
    nav.pop();
    alerts.update(&[conflict]);
    assert!(alerts.conflicts.button.is_visible());
    assert!(!alerts.problems.button.is_visible());
    alerts.update(&[]);
    assert!(!alerts.widget().is_visible());
    assert!(alerts.problems.empty.is_visible());
    nav.pop();
    assert_eq!(nav.visible_page(), Some(home));
}
