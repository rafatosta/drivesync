//! Native preferences window built with libadwaita components.
use super::{ColorScheme, Preferences, SYNC_INTERVALS};
use adw::{gtk, prelude::*};
use std::rc::Rc;

/// Applies the stored color scheme and registers `app.preferences` (Ctrl+,).
pub(crate) fn install(app: &adw::Application, window: &adw::ApplicationWindow) {
    adw::StyleManager::default().set_color_scheme(Preferences::load().color_scheme.adw());
    let action = gtk::gio::SimpleAction::new("preferences", None);
    let weak_window = window.downgrade();
    action.connect_activate(move |_, _| {
        if let Some(window) = weak_window.upgrade() {
            build(&window).present();
        }
    });
    app.add_action(&action);
    app.set_accels_for_action("app.preferences", &["<Primary>comma"]);
}

fn build(parent: &adw::ApplicationWindow) -> adw::PreferencesWindow {
    let window = adw::PreferencesWindow::builder()
        .transient_for(parent)
        .modal(true)
        .search_enabled(true)
        .build();
    let page = adw::PreferencesPage::builder()
        .title("General")
        .icon_name("preferences-system-symbolic")
        .build();

    let appearance = adw::PreferencesGroup::builder().title("Appearance").build();
    let schemes = gtk::StringList::new(&ColorScheme::ALL.map(ColorScheme::label));
    let theme = adw::ComboRow::builder()
        .title("Color Scheme")
        .subtitle("Choose between the system setting, light or dark")
        .model(&schemes)
        .build();
    let current = Preferences::load();
    theme.set_selected(
        ColorScheme::ALL
            .iter()
            .position(|scheme| *scheme == current.color_scheme)
            .unwrap_or(0) as u32,
    );
    appearance.add(&theme);
    page.add(&appearance);

    let synchronization = adw::PreferencesGroup::builder()
        .title("Synchronization")
        .build();
    let intervals = gtk::StringList::new(&SYNC_INTERVALS.map(|(_, label)| label));
    let interval = adw::ComboRow::builder()
        .title("Check for Changes")
        .subtitle("How often DriveSync looks for new changes automatically")
        .model(&intervals)
        .build();
    interval.set_selected(
        SYNC_INTERVALS
            .iter()
            .position(|(secs, _)| *secs == current.sync_interval)
            .unwrap_or(0) as u32,
    );
    synchronization.add(&interval);
    page.add(&synchronization);

    let background = adw::PreferencesGroup::builder()
        .title("Background")
        .description("DriveSync keeps synchronizing after the window is closed when background execution is allowed.")
        .build();
    let keep = adw::SwitchRow::builder()
        .title("Continue in Background When Closed")
        .subtitle(
            "Keep synchronizing after the window is closed; otherwise closing quits DriveSync",
        )
        .active(current.keep_running)
        .build();
    background.add(&keep);
    let notify = adw::SwitchRow::builder()
        .title("Notify When Running in Background")
        .subtitle("Show a notification when the window is closed")
        .active(current.notify_on_close)
        .sensitive(current.keep_running)
        .build();
    background.add(&notify);
    page.add(&background);
    window.add(&page);

    // Handlers re-read the file so each control changes only its own setting.
    let weak_window = window.downgrade();
    let report = Rc::new(move |result: Result<(), gtk::glib::Error>| {
        if let (Err(_), Some(window)) = (result, weak_window.upgrade()) {
            window.add_toast(adw::Toast::new("Preferences could not be saved"));
        }
    });
    let save_theme = report.clone();
    let save_keep = report.clone();
    let save_interval = report.clone();
    interval.connect_selected_notify(move |row| {
        if let Some((secs, _)) = SYNC_INTERVALS.get(row.selected() as usize) {
            let mut value = Preferences::load();
            value.sync_interval = *secs;
            save_interval(value.save());
        }
    });
    theme.connect_selected_notify(move |row| {
        let scheme = ColorScheme::ALL
            .get(row.selected() as usize)
            .copied()
            .unwrap_or_default();
        adw::StyleManager::default().set_color_scheme(scheme.adw());
        let mut value = Preferences::load();
        value.color_scheme = scheme;
        save_theme(value.save());
    });
    let dependent = notify.downgrade();
    keep.connect_active_notify(move |row| {
        if let Some(notify) = dependent.upgrade() {
            notify.set_sensitive(row.is_active());
        }
        let mut value = Preferences::load();
        value.keep_running = row.is_active();
        save_keep(value.save());
    });
    notify.connect_active_notify(move |row| {
        let mut value = Preferences::load();
        value.notify_on_close = row.is_active();
        report(value.save());
    });
    window
}
