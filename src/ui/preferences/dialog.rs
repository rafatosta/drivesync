//! In-window preferences navigation built with libadwaita components.
use super::{ColorScheme, Preferences};
use adw::{gtk, prelude::*};
use std::rc::Rc;

/// Applies the stored color scheme and registers `app.preferences` (Ctrl+,).
pub(crate) fn install(
    app: &adw::Application,
    navigation: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
) {
    adw::StyleManager::default()
        .set_color_scheme(super::appearance(Preferences::load().color_scheme));
    let action = gtk::gio::SimpleAction::new("preferences", None);
    let weak_navigation = navigation.downgrade();
    let weak_toasts = toasts.downgrade();
    action.connect_activate(move |_, _| {
        let (Some(nav), Some(toasts)) = (weak_navigation.upgrade(), weak_toasts.upgrade()) else {
            return;
        };
        if !nav.is_sensitive() {
            return;
        }
        if let Some(page) = nav.visible_page() {
            if page.widget_name() == "drivesync-preferences" {
                return;
            }
        }
        nav.push(&build(&nav, &toasts));
    });
    app.add_action(&action);
    app.set_accels_for_action("app.preferences", &["<Primary>comma"]);
}

/// Builds the preferences submenu and its content inside the main navigation.
pub(crate) fn build(
    navigation: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
) -> adw::NavigationPage {
    let split = adw::NavigationSplitView::builder()
        .min_sidebar_width(220.0)
        .max_sidebar_width(280.0)
        .sidebar_width_fraction(0.3)
        .build();
    split.set_widget_name("preferences-split");
    let sidebar = adw::ToolbarView::new();
    let sidebar_header = adw::HeaderBar::new();
    sidebar_header.set_show_back_button(false);
    let back = gtk::Button::from_icon_name("go-previous-symbolic");
    back.set_widget_name("preferences-back");
    back.set_tooltip_text(Some(&gettextrs::gettext("Back")));
    let weak_navigation = navigation.downgrade();
    back.connect_clicked(move |_| {
        if let Some(nav) = weak_navigation.upgrade() {
            nav.pop();
        }
    });
    sidebar_header.pack_start(&back);
    sidebar.add_top_bar(&sidebar_header);
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
    split.set_sidebar(Some(&adw::NavigationPage::new(
        &sidebar,
        &gettextrs::gettext("Preferences"),
    )));

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

    let background_page = adw::PreferencesPage::new();
    let background = adw::PreferencesGroup::builder()
        .title("Background")
        .description("DriveSync can continue running after the window is closed when background execution is allowed.")
        .build();
    let keep = adw::SwitchRow::builder()
        .title("Continue in Background When Closed")
        .subtitle("Keep DriveSync running after the window is closed; otherwise closing quits")
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
        ("appearance", "Appearance", "preferences-desktop-appearance-symbolic", "color scheme choose between the system setting follow system light or dark"),
        ("background", "Background", "application-x-executable-symbolic", "drivesync continues running after the window is closed notify when running in background show a notification when the window is closed "),
    ];
    for (_, title, icon, _) in sections {
        let row = adw::ActionRow::builder()
            .title(gettextrs::gettext(title))
            .build();
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
                content.set_title(&gettextrs::gettext(*title));
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
    let page = adw::NavigationPage::new(&bin, &gettextrs::gettext("Preferences"));
    page.set_widget_name("drivesync-preferences");

    // Handlers re-read the file so each control changes only its own setting.
    let weak_toasts = toasts.downgrade();
    let report = Rc::new(move |result: Result<(), gtk::glib::Error>| {
        if let (Err(_), Some(toasts)) = (result, weak_toasts.upgrade()) {
            toasts.add_toast(adw::Toast::new("Preferences could not be saved"));
        }
    });
    let save_theme = report.clone();
    let save_keep = report.clone();
    theme.connect_selected_notify(move |row| {
        let scheme = ColorScheme::ALL
            .get(row.selected() as usize)
            .copied()
            .unwrap_or_default();
        adw::StyleManager::default().set_color_scheme(super::appearance(scheme));
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
    page
}
