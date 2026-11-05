//! In-window preferences navigation built with libadwaita components.
use super::{ColorScheme, Preferences};
use adw::{gtk, prelude::*};
use drivesync_engine::AccountId;
use std::rc::Rc;

/// Applies the stored color scheme and registers `app.preferences` (Ctrl+,).
pub(crate) fn install(
    app: &adw::Application,
    navigation: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
) {
    adw::StyleManager::default().set_color_scheme(Preferences::load().color_scheme.adw());
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
            if page.widget_name() == "drivesync-preferences"
                || page.activate_action("account.preferences", None).is_ok()
            {
                return;
            }
        }
        nav.push(&build(&nav, &toasts, super::Accounts::new(vec![])));
    });
    app.add_action(&action);
    app.set_accels_for_action("app.preferences", &["<Primary>comma"]);
}

pub(crate) fn open(
    navigation: &adw::NavigationView,
    page: &adw::NavigationPage,
    accounts: &super::Accounts,
    selected: Option<&AccountId>,
    show_accounts: bool,
) {
    accounts.select(selected, show_accounts);
    if navigation.visible_page().as_ref() != Some(page) {
        navigation.push(page);
    }
}

/// Builds the preferences submenu and its content inside the main navigation.
pub(crate) fn build(
    navigation: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    account_context: Rc<super::Accounts>,
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

    pages.add_named(&account_context.page(), Some("accounts"));

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
    let remote = adw::PreferencesGroup::builder()
        .title(gettextrs::gettext("Google Drive"))
        .build();
    let interval = adw::SpinRow::with_range(1.0, 1440.0, 1.0);
    interval.set_widget_name("preferences-interval");
    interval.set_title(&gettextrs::gettext("Remote check interval"));
    interval.set_subtitle(&gettextrs::gettext(
        "Minutes between checks for changes in Google Drive",
    ));
    interval.set_value(f64::from(current.remote_check_minutes));
    remote.add(&interval);
    background_page.add(&background);
    background_page.add(&remote);
    pages.add_named(&background_page, Some("background"));

    let sections = [
        ("appearance", "Appearance", "preferences-desktop-appearance-symbolic", "color scheme choose between the system setting follow system light or dark"),
        ("accounts", "Accounts", "system-users-symbolic", "account accounts identity description email folders local google drive settings conta contas pastas descrição"),
        ("background", "Background", "application-x-executable-symbolic", "drivesync continues running after the window is closed notify when running in background show a notification when the window is closed remote check interval google drive minutes"),
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
    account_context.bind_navigation(&list, &split, &search);
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
    let save_interval = report.clone();
    interval.connect_value_notify(move |row| {
        let mut value = Preferences::load();
        value.remote_check_minutes = row.value() as u32;
        save_interval(value.save());
    });
    let save_theme = report.clone();
    let save_keep = report.clone();
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
    page
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
    #[ignore = "requires a desktop display"]
    fn account_preferences_preserve_the_correct_editors() {
        adw::init().unwrap();
        let nav = adw::NavigationView::new();
        nav.push(&adw::NavigationPage::new(&gtk::Box::default(), "Dashboard"));
        let toasts = adw::ToastOverlay::new();
        let mut records = vec![];
        let mut editors = vec![];
        for id in ["google:settings-one", "google:settings-two"] {
            let saved = drivesync_engine::RestoredAccount {
                account: drivesync_engine::AccountSummary {
                    id: AccountId(id.into()),
                    provider_subject: id.into(),
                    email: Some(format!("{id}@example.com")),
                },
                error: None,
                folder: None,
            };
            let editor = gtk::Box::new(gtk::Orientation::Vertical, 12);
            let draft = adw::EntryRow::builder().title("Draft").build();
            draft.set_text(id);
            editor.append(&draft);
            records.push(super::super::AccountSettings::new(saved, editor.clone()));
            editors.push((editor, draft));
        }
        let context = super::super::Accounts::new(records);
        let second = AccountId("google:settings-two".into());
        let window = build(&nav, &toasts, context.clone());
        open(&nav, &window, &context, Some(&second), true);
        let sections = find(window.upcast_ref(), "preferences-pages")
            .unwrap()
            .downcast::<gtk::Stack>()
            .unwrap();
        assert_eq!(sections.visible_child_name().as_deref(), Some("accounts"));
        let selected = find(window.upcast_ref(), "preferences-account-selector")
            .unwrap()
            .downcast::<adw::ComboRow>()
            .unwrap();
        let account_pages = find(window.upcast_ref(), "preferences-account-editors")
            .unwrap()
            .downcast::<gtk::Stack>()
            .unwrap();
        assert_eq!(selected.selected(), 1);
        assert_eq!(
            selected
                .selected_item()
                .unwrap()
                .downcast::<gtk::StringObject>()
                .unwrap()
                .string()
                .as_str(),
            "google:settings-two@example.com"
        );
        assert_eq!(
            account_pages.visible_child_name().as_deref(),
            Some(second.0.as_str())
        );
        assert_eq!(
            account_pages.visible_child().as_ref(),
            Some(editors[1].0.upcast_ref())
        );
        editors[1].1.set_text("unsaved second account draft");
        open(&nav, &window, &context, None, false);
        assert_eq!(
            selected.selected(),
            1,
            "focusing global preferences preserves the account chosen inside Preferences"
        );
        selected.set_selected(0);
        assert_eq!(
            selected
                .selected_item()
                .unwrap()
                .downcast::<gtk::StringObject>()
                .unwrap()
                .string()
                .as_str(),
            "google:settings-one@example.com"
        );
        assert_eq!(editors[0].1.text(), "google:settings-one");
        let first = AccountId("google:settings-one".into());
        open(&nav, &window, &context, Some(&first), true);
        assert_eq!(nav.visible_page().as_ref(), Some(&window));
        assert_eq!(
            account_pages.visible_child_name().as_deref(),
            Some(first.0.as_str())
        );
        let search = find(window.upcast_ref(), "preferences-search")
            .unwrap()
            .downcast::<gtk::SearchEntry>()
            .unwrap();
        search.set_text("folders");
        search.emit_by_name::<()>("search-changed", &[]);
        assert_eq!(sections.visible_child_name().as_deref(), Some("accounts"));
        search.set_text("closed notification");
        search.emit_by_name::<()>("search-changed", &[]);
        assert_eq!(sections.visible_child_name().as_deref(), Some("background"));
        open(&nav, &window, &context, Some(&second), true);
        assert!(search.text().is_empty());
        assert_eq!(selected.selected(), 1);
        assert_eq!(sections.visible_child_name().as_deref(), Some("accounts"));
        assert!(nav.pop());
        open(&nav, &window, &context, Some(&second), true);
        assert_eq!(nav.visible_page().as_ref(), Some(&window));
        assert_eq!(editors[1].1.text(), "unsaved second account draft");
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
        let nav = adw::NavigationView::new();
        let toasts = adw::ToastOverlay::new();
        let window = build(&nav, &toasts, super::super::Accounts::new(vec![]));
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
            .downcast::<adw::SpinRow>()
            .unwrap();
        assert_eq!(interval.value(), 5.0);
        interval.set_value(12.0);
        let theme = find(root, "preferences-theme")
            .unwrap()
            .downcast::<adw::ComboRow>()
            .unwrap();

        assert_eq!(pages.visible_child_name().as_deref(), Some("appearance"));
        list.select_row(list.row_at_index(2).as_ref());
        assert_eq!(pages.visible_child_name().as_deref(), Some("background"));
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
        theme.set_selected(2);
        let saved = Preferences::load();
        assert!(saved.keep_running);
        assert!(!saved.notify_on_close);
        assert_eq!(saved.color_scheme, ColorScheme::Dark);
        assert_eq!(saved.remote_check_minutes, 12);
        parent.close();
    }
}
