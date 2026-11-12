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
    capture: impl Fn(&gtk::Window, &str),
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

    // Snapshot the same content mounted directly in one window, without a parent.
    window.set_visible(false);
    let dialog = adw::Window::builder()
        .default_width(680)
        .default_height(620)
        .build();
    let view = SetupDialog::new(dialog.upcast_ref(), &providers);
    dialog.set_content(Some(view.widget()));
    let mut state = SetupState::default();
    view.render(&state);
    assert!(!find(dialog.upcast_ref(), "setup-continue").is_sensitive());
    capture(dialog.upcast_ref(), "setup-provider");
    state.select_provider(providers[0].clone());
    state.step = Step::Connection;
    view.render(&state);
    capture(dialog.upcast_ref(), "setup-connection");
    let header_position = header_positions(dialog.upcast_ref(), "connection");
    assert!(!find(dialog.upcast_ref(), "setup-continue").is_visible());
    assert!(!find(dialog.upcast_ref(), "setup-copy").is_visible());
    state.signing_in = true;
    state.busy = true;
    state.authorization_url = Some("https://example.invalid/authorize".into());
    view.render(&state);
    assert!(find(dialog.upcast_ref(), "setup-connect").is_sensitive());
    assert!(find(dialog.upcast_ref(), "setup-copy").is_visible());
    assert_eq!(
        find(dialog.upcast_ref(), "setup-connect-label")
            .downcast::<gtk::Label>()
            .unwrap()
            .label()
            .as_str(),
        gettextrs::gettext("Cancel sign-in").as_str()
    );
    capture(dialog.upcast_ref(), "setup-connection-waiting");
    state.signing_in = false;
    state.busy = false;
    state.authorization_url = None;
    state.account = Some(account.clone());
    view.render(&state);
    capture(dialog.upcast_ref(), "setup-connection-authorized");
    state.require_remote_authorization();
    view.render(&state);
    assert!(!find(dialog.upcast_ref(), "setup-continue").is_sensitive());
    assert!(find(dialog.upcast_ref(), "setup-connect").is_visible());
    assert_eq!(
        find(dialog.upcast_ref(), "setup-connect-label")
            .downcast::<gtk::Label>()
            .unwrap()
            .label()
            .as_str(),
        gettextrs::gettext("Authorize Drive access…").as_str()
    );
    capture(dialog.upcast_ref(), "setup-reauthorize-drive");
    state.remote_authorization_required = false;
    state.feedback.clear();
    state.step = Step::Synchronization;
    state.account_nickname = "Trabalho".into();
    state.draft.local_uri =
        Some("file:///run/user/1000/doc/fixture-portal-grant/Downloads/Google%20Drive".into());
    state.feedback =
        gettextrs::gettext("Local folder selected. Nothing is saved until setup is completed.");
    let root = crate::ui::components::remote_folder_picker::MockFolderSource::default().root();
    state.remote_folder = Some(
        crate::ui::components::remote_folder_picker::RemoteFolderLocation {
            folder: root.clone(),
            path: vec![root],
        },
    );
    view.render(&state);
    capture(dialog.upcast_ref(), "setup-synchronization");
    assert_eq!(
        header_positions(dialog.upcast_ref(), "synchronization"),
        header_position,
        "artwork and titles must stay aligned between setup steps"
    );
    let scroll = find(dialog.upcast_ref(), "setup-synchronization-scroll")
        .downcast::<gtk::ScrolledWindow>()
        .unwrap();
    assert!(
        scroll.vadjustment().upper() <= scroll.vadjustment().page_size() + 1.0,
        "configuration content must fit at the common setup minimum"
    );
    state.step = Step::Review;
    view.render(&state);
    assert!(find(dialog.upcast_ref(), "setup-start").is_sensitive());
    capture(dialog.upcast_ref(), "setup-review");
    assert_eq!(
        header_positions(dialog.upcast_ref(), "review"),
        header_position,
        "artwork and titles must stay aligned between setup steps"
    );
    let scroll = find(dialog.upcast_ref(), "setup-review-scroll")
        .downcast::<gtk::ScrolledWindow>()
        .unwrap();
    assert!(
        scroll.vadjustment().upper() <= scroll.vadjustment().page_size() + 1.0,
        "review content must fit at the common setup minimum"
    );
    dialog.set_default_size(360, 480);
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
    capture(dialog.upcast_ref(), "setup-review-small-dark");
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceLight);
    dialog.destroy();

    let (host, nav, toasts) = new_host();
    let context = new_context();
    let setup = super::show(&context, &nav, &toasts, None);
    assert_eq!(setup, host.clone().upcast::<gtk::Window>());
    assert_eq!(setup, super::show(&context, &nav, &toasts, None));
    assert_eq!(
        visible_windows(),
        1,
        "setup must occupy the only visible window"
    );
    let check = find_type::<gtk::CheckButton>(setup.upcast_ref()).unwrap();
    let next = find(&setup, "setup-continue")
        .downcast::<gtk::Button>()
        .unwrap();
    check.set_active(true);
    assert!(next.is_sensitive());
    check.set_active(false);
    assert!(!next.is_sensitive());
    next.emit_clicked();
    assert_eq!(
        find_type::<gtk::Stack>(setup.upcast_ref())
            .unwrap()
            .visible_child_name()
            .as_deref(),
        Some("provider")
    );
    check.set_active(true);
    next.emit_clicked();
    assert!(!next.is_sensitive());
    setup.close();
    wait_until(|| !setup.is_visible());

    let uri = gtk::gio::File::for_path(gtk::glib::user_data_dir())
        .uri()
        .to_string();
    let (host, nav, toasts) = new_host();
    let context = new_context();
    let setup = super::controller::Controller::with_local_choice(
        &context,
        &nav,
        &toasts,
        account.clone(),
        uri.clone(),
    );
    let next = find(&setup, "setup-continue")
        .downcast::<gtk::Button>()
        .unwrap();
    assert!(!next.is_sensitive(), "both folders are required");
    let choose = find(&setup, "setup-choose-remote")
        .downcast::<gtk::Button>()
        .unwrap();
    choose.emit_clicked();
    let picker = remote_picker();
    assert_eq!(picker.transient_for().as_ref(), Some(&setup));
    assert!(!choose.is_sensitive());
    button_with_label(picker.upcast_ref(), &gettextrs::gettext("Cancel"))
        .unwrap()
        .emit_clicked();
    assert!(choose.is_sensitive());
    assert!(!next.is_sensitive());
    choose.emit_clicked();
    let picker = remote_picker();
    let grid = find_type::<gtk::GridView>(picker.upcast_ref()).unwrap();
    let selection = grid
        .model()
        .unwrap()
        .downcast::<gtk::SingleSelection>()
        .unwrap();
    wait_until(|| selection.n_items() == 4);
    selection.set_selected(1);
    button_with_label(picker.upcast_ref(), &gettextrs::gettext("Select folder"))
        .unwrap()
        .emit_clicked();
    assert!(next.is_sensitive());
    let remote_row = find(&setup, "setup-remote-folder")
        .downcast::<adw::ActionRow>()
        .unwrap();
    assert_eq!(
        remote_row.subtitle().as_deref(),
        Some("Meu Drive / Projetos")
    );
    choose.emit_clicked();
    let picker = remote_picker();
    button_with_label(picker.upcast_ref(), &gettextrs::gettext("Cancel"))
        .unwrap()
        .emit_clicked();
    assert_eq!(
        remote_row.subtitle().as_deref(),
        Some("Meu Drive / Projetos")
    );
    assert!(next.is_sensitive());
    next.emit_clicked();
    wait_until(|| !next.is_visible());
    assert_eq!(
        find(&setup, "setup-review-remote")
            .downcast::<adw::ActionRow>()
            .unwrap()
            .subtitle()
            .as_deref(),
        Some("Meu Drive / Projetos")
    );
    assert!(find(&setup, "setup-start").is_sensitive());
    assert!(find_type::<adw::NavigationSplitView>(setup.upcast_ref()).is_none());
    assert_eq!(visible_windows(), 1);
    assert!(
        gtk::glib::MainContext::default()
            .block_on(crate::services::setup::load_draft(&account.id))
            .unwrap()
            .local_uri
            .is_none(),
        "current choices must not be persisted"
    );
    capture(&setup, "setup-single-window-review");
    setup.close();
    wait_until(|| !host.is_visible());
    assert!(gtk::glib::MainContext::default()
        .block_on(
            context
                .engine
                .borrow()
                .as_ref()
                .unwrap()
                .list_accounts(drivesync_engine::ListAccountsRequest { provider: None })
        )
        .unwrap()
        .accounts
        .is_empty());

    // Closing setup also destroys an outstanding modal picker without applying it.
    let (host, nav, toasts) = new_host();
    let context = new_context();
    let setup = super::controller::Controller::with_local_choice(
        &context,
        &nav,
        &toasts,
        account.clone(),
        uri.clone(),
    );
    find(&setup, "setup-choose-remote")
        .downcast::<gtk::Button>()
        .unwrap()
        .emit_clicked();
    let picker = remote_picker();
    setup.close();
    wait_until(|| !host.is_visible());
    assert!(!picker.is_visible());

    // Explicit logout returns to the first page in the same window.
    let (host, nav, toasts) = new_host();
    let context = new_context();
    let setup = super::show(&context, &nav, &toasts, Some(account.clone()));
    let remove = find(&setup, "setup-disconnect")
        .downcast::<gtk::Button>()
        .unwrap();
    remove.emit_clicked();
    answer_confirmation("cancel");
    assert!(remove.is_visible());
    remove.emit_clicked();
    answer_confirmation("disconnect");
    wait_until(|| !remove.is_visible());
    assert_eq!(
        find_type::<gtk::Stack>(setup.upcast_ref())
            .unwrap()
            .visible_child_name()
            .as_deref(),
        Some("provider")
    );
    assert!(!find(&setup, "setup-continue").is_sensitive());
    assert_eq!(visible_windows(), 1);
    setup.close();
    wait_until(|| !host.is_visible());

    // A cleanup failure must remain visible and retry logout before exiting.
    let (host, nav, toasts) = new_host();
    let context = Context::default();
    let setup = super::show(&context, &nav, &toasts, Some(account.clone()));
    setup.close();
    wait_until(|| {
        find(&setup, "setup-feedback")
            .downcast::<gtk::Label>()
            .unwrap()
            .label()
            == gettextrs::gettext("The account service is unavailable. Try again.")
    });
    assert!(host.is_visible());
    *context.engine.borrow_mut() = new_context().engine.borrow().clone();
    setup.close();
    wait_until(|| !host.is_visible());

    // A late saved identity must be cleaned before closing, never resumed.
    let (host, nav, toasts) = new_host();
    let context = new_context();
    gtk::glib::MainContext::default()
        .block_on(crate::services::setup::save_draft(
            &account.id,
            &SetupDraft {
                local_uri: Some(uri.clone()),
            },
        ))
        .unwrap();
    let setup = super::controller::Controller::closing_with_late_account(
        &context,
        &nav,
        &toasts,
        account.clone(),
    );
    wait_until(|| !host.is_visible());
    assert!(!setup.is_visible());
    assert!(gtk::glib::MainContext::default()
        .block_on(crate::services::setup::load_draft(&account.id))
        .unwrap()
        .local_uri
        .is_none());

    // Legacy incomplete registrations are discarded at startup. There is no
    // resume screen; hidden startup mounts the fresh wizard without presenting it.
    let (host, nav, toasts) = new_host();
    let context = new_context();
    gtk::glib::MainContext::default()
        .block_on(crate::services::setup::save_draft(
            &account.id,
            &SetupDraft {
                local_uri: Some(uri),
            },
        ))
        .unwrap();
    crate::application::navigation::show_accounts(
        &context,
        vec![account.clone()],
        &nav,
        &toasts,
        None,
        false,
    );
    wait_until(|| host.widget_name() == "drivesync-account-setup");
    assert!(!host.is_visible());
    assert!(gtk::glib::MainContext::default()
        .block_on(crate::services::setup::load_draft(&account.id))
        .unwrap()
        .local_uri
        .is_none());
    host.present();
    assert_eq!(visible_windows(), 1);
    assert!(!find_type::<gtk::CheckButton>(host.upcast_ref())
        .unwrap()
        .is_active());
    assert!(!find(host.upcast_ref(), "setup-continue").is_sensitive());
    assert!(!contains(host.upcast_ref(), "setup-landing"));
    assert!(!contains(host.upcast_ref(), "pending-account-resume"));
    capture(host.upcast_ref(), "setup-fresh-start");
    host.close();
    wait_until(|| !host.is_visible());

    // Completion persists choices, mounts an empty dashboard, and survives a restart.
    let (host, nav, toasts) = new_host();
    let context = new_context();
    let app = window
        .application()
        .and_downcast::<adw::Application>()
        .unwrap();
    host.set_application(Some(&app));
    crate::ui::preferences::install(&app, &nav, &toasts, &context);
    crate::ui::components::connection::install_action(&context, &nav, &toasts);
    let mut completed = account.clone();
    completed.id = drivesync_engine::AccountId("completed-fixture".into());
    gtk::glib::MainContext::default()
        .block_on(crate::services::configuration::remove(&completed.id))
        .unwrap();
    let local_path = gtk::glib::user_data_dir().join("completion-local");
    std::fs::create_dir_all(&local_path).unwrap();
    let local_uri = gtk::gio::File::for_path(&local_path).uri().to_string();
    let setup = super::controller::Controller::with_local_choice(
        &context,
        &nav,
        &toasts,
        completed.clone(),
        local_uri.clone(),
    );
    let nickname = find(&setup, "setup-account-nickname")
        .downcast::<adw::EntryRow>()
        .unwrap();
    nickname.set_text(" Trabalho ");
    find(&setup, "setup-choose-remote")
        .downcast::<gtk::Button>()
        .unwrap()
        .emit_clicked();
    let picker = remote_picker();
    let select =
        button_with_label(picker.upcast_ref(), &gettextrs::gettext("Select folder")).unwrap();
    wait_until(|| select.is_sensitive());
    select.emit_clicked();
    let next = find(&setup, "setup-continue")
        .downcast::<gtk::Button>()
        .unwrap();
    wait_until(|| next.is_sensitive());
    next.emit_clicked();
    wait_until(|| !next.is_visible());
    assert!(find(&setup, "setup-review-account")
        .downcast::<adw::ActionRow>()
        .unwrap()
        .subtitle()
        .unwrap()
        .contains("Trabalho"));
    find(&setup, "setup-back")
        .downcast::<gtk::Button>()
        .unwrap()
        .emit_clicked();
    assert_eq!(nickname.text().as_str(), " Trabalho ");
    next.emit_clicked();
    wait_until(|| !next.is_visible());
    let finish = find(&setup, "setup-start")
        .downcast::<gtk::Button>()
        .unwrap();
    std::fs::remove_dir(&local_path).unwrap();
    finish.emit_clicked();
    wait_until(|| finish.is_sensitive());
    assert_eq!(host.widget_name(), "drivesync-account-setup");
    assert!(gtk::glib::MainContext::default()
        .block_on(crate::services::configuration::load(&completed.id))
        .unwrap()
        .is_none());
    std::fs::create_dir(&local_path).unwrap();
    finish.emit_clicked();
    wait_until(|| host.widget_name() == "drivesync-dashboard");
    assert!(find(&setup, "dashboard-content").first_child().is_none());
    assert!(!contains(setup.upcast_ref(), "setup-continue"));
    assert_eq!(visible_windows(), 1);
    nav.activate_action("connection.open", None).unwrap();
    assert_eq!(host.widget_name(), "drivesync-dashboard");
    assert!(!contains(setup.upcast_ref(), "setup-continue"));
    capture(&setup, "dashboard-completed");
    app.activate_action("preferences", None);
    wait_until(|| {
        nav.visible_page()
            .is_some_and(|p| p.widget_name() == "drivesync-preferences")
    });
    assert!(contains(setup.upcast_ref(), "preferences-configuration"));
    find(&setup, "preferences-back")
        .downcast::<gtk::Button>()
        .unwrap()
        .emit_clicked();
    wait_until(|| {
        nav.visible_page()
            .is_some_and(|p| p.widget_name() == "drivesync-dashboard")
    });
    assert!(find(&setup, "dashboard-content").first_child().is_none());
    host.close();
    wait_until(|| !host.is_visible());
    let saved = gtk::glib::MainContext::default()
        .block_on(crate::services::configuration::load(&completed.id))
        .unwrap()
        .unwrap();
    assert_eq!(saved.local_uri, local_uri);
    let (host, nav, toasts) = new_host();
    completed.state = drivesync_engine::AccountState::ReauthorizationRequired;
    crate::application::navigation::show_accounts(
        &context,
        vec![completed.clone()],
        &nav,
        &toasts,
        None,
        false,
    );
    wait_until(|| {
        nav.visible_page()
            .is_some_and(|p| p.widget_name() == "drivesync-dashboard")
    });
    assert!(!contains(host.upcast_ref(), "setup-continue"));
    assert!(gtk::glib::MainContext::default()
        .block_on(crate::services::configuration::load(&completed.id))
        .unwrap()
        .is_some());
    host.destroy();

    let (host, nav, toasts) = new_host();
    let existing = super::controller::Controller::with_completed_account(
        &context,
        &nav,
        &toasts,
        completed.clone(),
    );
    wait_until(|| host.widget_name() == "drivesync-dashboard");
    assert!(!contains(existing.upcast_ref(), "setup-continue"));
    assert_eq!(
        gtk::glib::MainContext::default()
            .block_on(crate::services::configuration::load(&completed.id))
            .unwrap(),
        Some(saved.clone())
    );
    host.close();
    wait_until(|| !host.is_visible());

    // Closing during the final commit waits without removing the completed record.
    let (host, nav, toasts) = new_host();
    let context = new_context();
    let mut closing = account.clone();
    closing.id = drivesync_engine::AccountId("completion-close-fixture".into());
    gtk::glib::MainContext::default()
        .block_on(crate::services::configuration::remove(&closing.id))
        .unwrap();
    let setup = super::controller::Controller::with_local_choice(
        &context,
        &nav,
        &toasts,
        closing.clone(),
        saved.local_uri.clone(),
    );
    find(&setup, "setup-choose-remote")
        .downcast::<gtk::Button>()
        .unwrap()
        .emit_clicked();
    let picker = remote_picker();
    let select =
        button_with_label(picker.upcast_ref(), &gettextrs::gettext("Select folder")).unwrap();
    wait_until(|| select.is_sensitive());
    select.emit_clicked();
    let next = find(&setup, "setup-continue")
        .downcast::<gtk::Button>()
        .unwrap();
    wait_until(|| next.is_sensitive());
    next.emit_clicked();
    wait_until(|| !next.is_visible());
    find(&setup, "setup-start")
        .downcast::<gtk::Button>()
        .unwrap()
        .emit_clicked();
    setup.close();
    assert!(host.is_visible());
    wait_until(|| !host.is_visible());
    assert!(gtk::glib::MainContext::default()
        .block_on(crate::services::configuration::load(&closing.id))
        .unwrap()
        .is_some());
}
fn new_host() -> (adw::Window, adw::NavigationView, adw::ToastOverlay) {
    let nav = adw::NavigationView::new();
    nav.add(&adw::NavigationPage::new(
        &gtk::Box::new(gtk::Orientation::Vertical, 0),
        "Fixture",
    ));
    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&nav));
    let host = adw::Window::builder()
        .content(&toasts)
        .default_width(680)
        .default_height(620)
        .build();
    (host, nav, toasts)
}
fn new_context() -> Context {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let context = Context::default();
    let engine = gtk::glib::MainContext::default()
        .block_on(drivesync_engine::Engine::open(
            drivesync_engine::EngineConfig {
                data_directory: gtk::glib::user_data_dir().join(format!(
                    "setup-fixture-{}",
                    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                )),
                application_id: "com.rtosta.DriveSync.SetupFixture".into(),
                google_oauth_file: None,
            },
        ))
        .unwrap();
    *context.engine.borrow_mut() = Some(engine);
    context
}
fn visible_windows() -> usize {
    gtk::Window::list_toplevels()
        .into_iter()
        .filter(|window| window.is_visible())
        .count()
}

fn find(window: &gtk::Window, name: &str) -> gtk::Widget {
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

fn contains(widget: &gtk::Widget, name: &str) -> bool {
    if widget.widget_name() == name {
        return true;
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        if contains(&widget, name) {
            return true;
        }
        child = widget.next_sibling();
    }
    false
}
fn confirmation() -> adw::MessageDialog {
    gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|window| window.downcast::<adw::MessageDialog>().ok())
        .find(|dialog| {
            dialog.widget_name() == "disconnect-account-confirmation" && dialog.is_visible()
        })
        .expect("account disconnect confirmation")
}

fn answer_confirmation(response: &str) {
    let dialog = confirmation();
    // The response API only emits a signal; emulate the button's dismissal too.
    dialog.response(response);
    dialog.destroy();
}

fn remote_picker() -> gtk::Window {
    gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Window>().ok())
        .find(|window| window.has_css_class("remote-folder-picker") && window.is_visible())
        .expect("remote picker must be visible")
}
fn button_with_label(widget: &gtk::Widget, label: &str) -> Option<gtk::Button> {
    if let Ok(button) = widget.clone().downcast::<gtk::Button>() {
        if button.label().as_deref() == Some(label) {
            return Some(button);
        }
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        if let Some(button) = button_with_label(&widget, label) {
            return Some(button);
        }
        child = widget.next_sibling();
    }
    None
}

/// Compares actual positions after the page transition settles.
fn header_positions(window: &gtk::Window, step: &str) -> (f32, f32) {
    let artwork = find(window, &format!("setup-{step}-artwork"));
    let title = find(window, &format!("setup-{step}-title"));
    (
        artwork.compute_bounds(window).unwrap().y(),
        title.compute_bounds(window).unwrap().y(),
    )
}
