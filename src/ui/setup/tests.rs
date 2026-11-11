//! Display-only setup checks. No OAuth, portals or real account storage is used.
use super::{
    dialog::SetupDialog,
    models::{SetupState, Step},
};
use crate::{application::Context, services::setup::SetupDraft};
use adw::{gtk, prelude::*};
use drivesync_engine::Account;

/// Called on the same GTK thread as the other application layout fixtures.
pub(crate) fn exercise(
    window: &adw::ApplicationWindow,
    account: &Account,
    capture: impl Fn(&adw::Window, &str),
) {
    let providers = crate::services::setup::list_providers();
    // A redraw triggered by either side of a GTK group transition must not
    // interfere with the other button becoming active.
    let mut other = providers[0].clone();
    other.id = drivesync_engine::ProviderId("fixture-other".into());
    let selector = std::rc::Rc::new(super::pages::ProviderPage::new(&[
        providers[0].clone(),
        other.clone(),
    ]));
    let selected = std::rc::Rc::new(std::cell::RefCell::new(None));
    let current = selected.clone();
    let weak_selector = std::rc::Rc::downgrade(&selector);
    selector.on_selected(std::rc::Rc::new(move |id, active| {
        if active {
            *current.borrow_mut() = Some(id);
        } else if current.borrow().as_ref() == Some(&id) {
            current.borrow_mut().take();
        }
        weak_selector
            .upgrade()
            .unwrap()
            .set_selected(current.borrow().as_ref(), true);
    }));
    let mut checks = Vec::new();
    collect_checks(selector.widget().upcast_ref(), &mut checks);
    assert_eq!(checks.len(), 2);
    checks[0].set_active(true);
    checks[1].set_active(true);
    assert!(!checks[0].is_active());
    assert!(checks[1].is_active());
    assert_eq!(*selected.borrow(), Some(other.id));
    let previous = selected.borrow().clone();
    selector.set_selected(None, true);
    assert_eq!(
        *selected.borrow(),
        previous,
        "rendering must not emit user selection events"
    );

    let (dialog, view) = SetupDialog::new(Some(window.upcast_ref()), &providers);
    let mut state = SetupState::default();
    view.render(&state);
    assert!(!find(&dialog, "setup-continue").is_sensitive());
    capture(&dialog, "setup-provider");
    state.select_provider(providers[0].clone());
    view.render(&state);
    assert!(find(&dialog, "setup-continue").is_sensitive());
    state.step = Step::Connection;
    view.render(&state);
    assert!(!find(&dialog, "setup-continue").is_sensitive());
    assert!(find(&dialog, "setup-connect").is_visible());
    capture(&dialog, "setup-connection");
    state.account = Some(account.clone());
    state.step = Step::Synchronization;
    view.render(&state);
    assert!(!find(&dialog, "setup-continue").is_sensitive());
    state.draft = SetupDraft {
        local_uri: Some("file:///tmp/drivesync-fixture/Google%20Drive".into()),
    };
    view.render(&state);
    assert!(find(&dialog, "setup-continue").is_sensitive());
    capture(&dialog, "setup-synchronization");
    state.step = Step::Review;
    view.render(&state);
    assert!(!find(&dialog, "setup-start").is_sensitive());
    assert!(!find(&dialog, "setup-continue").is_visible());
    capture(&dialog, "setup-review");
    dialog.set_default_size(360, 480);
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
    capture(&dialog, "setup-review-small-dark");
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceLight);
    dialog.destroy();

    // Drive real controller events; updating the selection must not panic by
    // borrowing mutable state while presentation emits a selection notification.
    let nav = adw::NavigationView::new();
    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&nav));
    nav.add(&adw::NavigationPage::new(
        &gtk::Box::new(gtk::Orientation::Vertical, 0),
        "Fixture",
    ));
    window.set_content(Some(&toasts));
    let setup = super::show(&Context::default(), &nav, &toasts, None);
    let same = super::show(&Context::default(), &nav, &toasts, None);
    assert_eq!(setup, same);
    let check = find_type::<gtk::CheckButton>(setup.upcast_ref()).expect("provider selection");
    check.set_active(true);
    let next = find(&setup, "setup-continue")
        .downcast::<gtk::Button>()
        .unwrap();
    assert!(next.is_sensitive());
    check.set_active(false);
    assert!(
        !next.is_sensitive(),
        "clearing the provider must disable Continue"
    );
    next.emit_clicked();
    let stack = find_type::<gtk::Stack>(setup.upcast_ref()).unwrap();
    assert_eq!(stack.visible_child_name().as_deref(), Some("provider"));
    check.set_active(true);
    assert!(next.is_sensitive());
    next.emit_clicked();
    assert!(find(&setup, "setup-connect").is_visible());
    assert!(!next.is_sensitive());
    setup.close();

    // Resume a persisted account-specific draft and exercise the asynchronous
    // directory check before displaying review. All paths belong to this fixture.
    let uri = gtk::gio::File::for_path(gtk::glib::user_data_dir())
        .uri()
        .to_string();
    gtk::glib::MainContext::default()
        .block_on(crate::services::setup::save_draft(
            &account.id,
            &SetupDraft {
                local_uri: Some(uri),
            },
        ))
        .unwrap();
    let resumed = super::show(&Context::default(), &nav, &toasts, Some(account.clone()));
    let next = find(&resumed, "setup-continue")
        .downcast::<gtk::Button>()
        .unwrap();
    wait_until(|| next.is_sensitive());
    next.emit_clicked();
    wait_until(|| !next.is_visible());
    assert!(!find(&resumed, "setup-start").is_sensitive());
    resumed.close();

    let mut expired = account.clone();
    expired.state = drivesync_engine::AccountState::ReauthorizationRequired;
    let authorization = super::show(&Context::default(), &nav, &toasts, Some(expired));
    assert!(find(&authorization, "setup-connect").is_visible());
    assert!(!find(&authorization, "setup-continue").is_sensitive());
    authorization.close();

    // An empty profile offers setup only when its main window becomes visible.
    window.set_visible(false);
    crate::application::navigation::show_accounts(
        &Context::default(),
        vec![],
        &nav,
        &toasts,
        None,
        false,
    );
    assert!(!gtk::Window::list_toplevels()
        .iter()
        .any(|window| window.widget_name() == "drivesync-account-setup" && window.is_visible()));
    window.present();
    let offered = gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|window| window.downcast::<adw::Window>().ok())
        .find(|window| window.widget_name() == "drivesync-account-setup" && window.is_visible())
        .expect("first account setup appears with the main window");
    offered.close();
    window.set_content(None::<&gtk::Widget>);
}
fn find(window: &adw::Window, name: &str) -> gtk::Widget {
    fn search(widget: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
        if widget.widget_name() == name {
            return Some(widget.clone());
        }
        let mut child = widget.first_child();
        while let Some(widget) = child {
            if let Some(found) = search(&widget, name) {
                return Some(found);
            }
            child = widget.next_sibling();
        }
        None
    }
    search(window.upcast_ref(), name).unwrap_or_else(|| panic!("missing widget {name}"))
}
fn find_type<T: IsA<gtk::Widget> + gtk::glib::object::IsClass>(widget: &gtk::Widget) -> Option<T> {
    if let Ok(value) = widget.clone().downcast::<T>() {
        return Some(value);
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        if let Some(found) = find_type::<T>(&widget) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}

fn wait_until(condition: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !condition() {
        assert!(
            std::time::Instant::now() < deadline,
            "setup operation timed out"
        );
        let loop_ = gtk::glib::MainLoop::new(None, false);
        let finish = loop_.clone();
        gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(10), move || {
            finish.quit()
        });
        loop_.run();
    }
}

fn collect_checks(widget: &gtk::Widget, result: &mut Vec<gtk::CheckButton>) {
    if let Ok(check) = widget.clone().downcast::<gtk::CheckButton>() {
        result.push(check);
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        collect_checks(&widget, result);
        child = widget.next_sibling();
    }
}
