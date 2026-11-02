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

/// Presents global settings in an adaptive sidebar without changing their storage.
fn build(parent: &adw::ApplicationWindow) -> adw::Window {
    let window = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Preferences")
        .default_width(850)
        .default_height(600)
        .build();
    let split = adw::NavigationSplitView::builder()
        .min_sidebar_width(220.0)
        .max_sidebar_width(280.0)
        .sidebar_width_fraction(0.3)
        .build();
    split.set_widget_name("preferences-split");
    let sidebar = adw::ToolbarView::new();
    sidebar.add_top_bar(&adw::HeaderBar::new());
    let sidebar_content = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search Preferences")
        .margin_top(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    search.set_widget_name("preferences-search");
    sidebar_content.append(&search);
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .activate_on_single_click(true)
        .css_classes(["navigation-sidebar"])
        .build();
    list.set_widget_name("preferences-list");
    sidebar_content.append(&list);
    let no_results = gtk::Label::builder()
        .label("No matching preferences")
        .css_classes(["dim-label"])
        .margin_top(12)
        .visible(false)
        .build();
    sidebar_content.append(&no_results);
    let sidebar_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&sidebar_content)
        .build();
    sidebar.set_content(Some(&sidebar_scroll));
    split.set_sidebar(Some(&adw::NavigationPage::new(&sidebar, "Preferences")));

    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    split
        .bind_property("collapsed", &header, "show-back-button")
        .sync_create()
        .build();
    toolbar.add_top_bar(&header);
    let pages = gtk::Stack::new();
    pages.set_widget_name("preferences-pages");
    toolbar.set_content(Some(&pages));
    let content = adw::NavigationPage::new(&toolbar, "Appearance");
    split.set_content(Some(&content));

    let appearance_page = adw::PreferencesPage::new();
    let appearance = adw::PreferencesGroup::builder().title("Appearance").build();
    let schemes = gtk::StringList::new(&ColorScheme::ALL.map(ColorScheme::label));
    let theme = adw::ComboRow::builder()
        .title("Color Scheme")
        .subtitle("Choose between the system setting, light or dark")
        .model(&schemes)
        .build();
    theme.set_widget_name("preferences-theme");
    let current = Preferences::load();
    theme.set_selected(
        ColorScheme::ALL
            .iter()
            .position(|scheme| *scheme == current.color_scheme)
            .unwrap_or(0) as u32,
    );
    appearance.add(&theme);
    appearance_page.add(&appearance);
    pages.add_named(&appearance_page, Some("appearance"));

    let synchronization_page = adw::PreferencesPage::new();
    let synchronization = adw::PreferencesGroup::builder()
        .title("Synchronization")
        .build();
    let intervals = gtk::StringList::new(&SYNC_INTERVALS.map(|(_, label)| label));
    let interval = adw::ComboRow::builder()
        .title("Check for Changes")
        .subtitle("How often DriveSync looks for new changes automatically")
        .model(&intervals)
        .build();
    interval.set_widget_name("preferences-interval");
    interval.set_selected(
        SYNC_INTERVALS
            .iter()
            .position(|(secs, _)| *secs == current.sync_interval)
            .unwrap_or(0) as u32,
    );
    synchronization.add(&interval);
    synchronization_page.add(&synchronization);
    pages.add_named(&synchronization_page, Some("synchronization"));

    let background_page = adw::PreferencesPage::new();
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
    keep.set_widget_name("preferences-keep-running");
    background.add(&keep);
    let notify = adw::SwitchRow::builder()
        .title("Notify When Running in Background")
        .subtitle("Show a notification when the window is closed")
        .active(current.notify_on_close)
        .sensitive(current.keep_running)
        .build();
    notify.set_widget_name("preferences-notify");
    background.add(&notify);
    background_page.add(&background);
    pages.add_named(&background_page, Some("background"));

    let sections = [
        ("appearance", "Appearance", "preferences-desktop-theme-symbolic", "color scheme choose between the system setting follow system light or dark"),
        ("synchronization", "Synchronization", "emblem-synchronizing-symbolic", "check for changes how often drivesync looks for new changes automatically every minute 5 15 30 hour"),
        ("background", "Background", "application-x-executable-symbolic", "drivesync keeps synchronizing after the window is closed when background execution is allowed continue in background when closed keep synchronizing after the window is closed otherwise closing quits drivesync notify when running in background show a notification when the window is closed"),
    ];
    for (_, title, icon, _) in sections {
        let row = adw::ActionRow::builder().title(title).build();
        row.add_prefix(&gtk::Image::from_icon_name(icon));
        list.append(&row);
    }
    let weak_pages = pages.downgrade();
    let weak_content = content.downgrade();
    let weak_split = split.downgrade();
    list.connect_row_selected(move |_, row| {
        if let (Some(row), Some(pages), Some(content)) =
            (row, weak_pages.upgrade(), weak_content.upgrade())
        {
            if let Some((name, title, _, _)) = sections.get(row.index() as usize) {
                pages.set_visible_child_name(name);
                content.set_title(title);
            }
        }
    });
    list.select_row(list.row_at_index(0).as_ref());
    list.connect_row_activated(move |_, _| {
        if let Some(split) = weak_split.upgrade() {
            split.set_show_content(true);
        }
    });
    let weak_list = list.downgrade();
    let weak_no_results = no_results.downgrade();
    search.connect_search_changed(move |entry| {
        if let (Some(list), Some(no_results)) = (weak_list.upgrade(), weak_no_results.upgrade()) {
            let query = entry.text().to_lowercase();
            let query = query.trim();
            let mut first_match = None;
            for (index, (_, title, _, terms)) in sections.iter().enumerate() {
                let title = title.to_lowercase();
                let matches = query
                    .split_whitespace()
                    .all(|word| title.contains(word) || terms.contains(word));
                if let Some(row) = list.row_at_index(index as i32) {
                    row.set_visible(matches);
                    if matches && first_match.is_none() {
                        first_match = Some(row);
                    }
                }
            }
            no_results.set_visible(first_match.is_none());
            if list
                .selected_row()
                .is_none_or(|row| !row.property::<bool>("visible"))
            {
                list.select_row(first_match.as_ref());
            }
        }
    });
    let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        660.0,
        adw::LengthUnit::Sp,
    ));
    breakpoint.add_setter(&split, "collapsed", Some(&true.to_value()));
    let bin = adw::BreakpointBin::new();
    bin.set_width_request(320);
    bin.set_height_request(320);
    bin.set_child(Some(&split));
    bin.add_breakpoint(breakpoint);
    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&bin));
    window.set_content(Some(&toasts));

    // Handlers re-read the file so each control changes only its own setting.
    let weak_toasts = toasts.downgrade();
    let report = Rc::new(move |result: Result<(), gtk::glib::Error>| {
        if let (Err(_), Some(toasts)) = (result, weak_toasts.upgrade()) {
            toasts.add_toast(adw::Toast::new("Preferences could not be saved"));
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    #[ignore = "requires a desktop display and an isolated XDG_CONFIG_HOME"]
    fn preferences_navigation_search_and_settings() {
        let config = std::env::var("XDG_CONFIG_HOME").expect("isolated configuration directory");
        assert!(config.starts_with("/tmp/drivesync-preferences-ui-"));
        adw::init().unwrap();
        let application =
            adw::Application::new(None::<&str>, gtk::gio::ApplicationFlags::NON_UNIQUE);
        application
            .register(None::<&gtk::gio::Cancellable>)
            .unwrap();
        let parent = adw::ApplicationWindow::new(&application);
        let window = build(&parent);
        let root = window.upcast_ref::<gtk::Widget>();
        let list = find(root, "preferences-list")
            .unwrap()
            .downcast::<gtk::ListBox>()
            .unwrap();
        let pages = find(root, "preferences-pages")
            .unwrap()
            .downcast::<gtk::Stack>()
            .unwrap();
        let search = find(root, "preferences-search")
            .unwrap()
            .downcast::<gtk::SearchEntry>()
            .unwrap();
        let split = find(root, "preferences-split")
            .unwrap()
            .downcast::<adw::NavigationSplitView>()
            .unwrap();
        let keep = find(root, "preferences-keep-running")
            .unwrap()
            .downcast::<adw::SwitchRow>()
            .unwrap();
        let notify = find(root, "preferences-notify")
            .unwrap()
            .downcast::<adw::SwitchRow>()
            .unwrap();
        let interval = find(root, "preferences-interval")
            .unwrap()
            .downcast::<adw::ComboRow>()
            .unwrap();
        let theme = find(root, "preferences-theme")
            .unwrap()
            .downcast::<adw::ComboRow>()
            .unwrap();

        assert_eq!(pages.visible_child_name().as_deref(), Some("appearance"));
        list.select_row(list.row_at_index(1).as_ref());
        assert_eq!(
            pages.visible_child_name().as_deref(),
            Some("synchronization")
        );
        split.set_collapsed(true);
        assert!(!split.property::<bool>("show-content"));
        let selected = list.selected_row().unwrap();
        list.emit_by_name::<()>("row-activated", &[&selected]);
        assert!(split.property::<bool>("show-content"));
        split.set_show_content(false);
        search.set_text("closed notification");
        search.emit_by_name::<()>("search-changed", &[]);
        assert_eq!(pages.visible_child_name().as_deref(), Some("background"));
        assert!(!list.row_at_index(0).unwrap().property::<bool>("visible"));
        assert!(list.row_at_index(2).unwrap().property::<bool>("visible"));
        search.set_text("no such setting");
        search.emit_by_name::<()>("search-changed", &[]);
        assert!(list.selected_row().is_none());
        search.set_text("");
        search.emit_by_name::<()>("search-changed", &[]);
        assert!(list.row_at_index(0).unwrap().property::<bool>("visible"));

        keep.set_active(false);
        assert!(!notify.is_sensitive());
        keep.set_active(true);
        assert!(notify.is_sensitive());
        notify.set_active(false);
        interval.set_selected(2);
        theme.set_selected(2);
        let saved = Preferences::load();
        assert!(saved.keep_running);
        assert!(!saved.notify_on_close);
        assert_eq!(saved.sync_interval, 900);
        assert_eq!(saved.color_scheme, ColorScheme::Dark);
        window.close();
        parent.close();
    }
}
