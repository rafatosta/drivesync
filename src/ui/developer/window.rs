//! Standalone developer console with navigation and empty pages.
use adw::{gtk, prelude::*};

/// Registers a window action and F12, reusing the console while it is open.
pub(crate) fn install(parent: &adw::ApplicationWindow) {
    let action = gtk::gio::SimpleAction::new("developer-console", None);
    let console = gtk::glib::WeakRef::<adw::ApplicationWindow>::new();
    let weak_parent = parent.downgrade();
    action.connect_activate(move |_, _| {
        if let Some(window) = console.upgrade() {
            window.present();
            return;
        }
        let Some(parent) = weak_parent.upgrade() else {
            return;
        };
        let window = build(&parent);
        console.set(Some(&window));
        window.present();
    });
    parent.add_action(&action);
    if let Some(app) = parent.application() {
        app.set_accels_for_action("win.developer-console", &["F12"]);
    }
}

fn build(parent: &adw::ApplicationWindow) -> adw::ApplicationWindow {
    let window = adw::ApplicationWindow::builder()
        .title("Developer Console")
        .default_width(1000)
        .default_height(720)
        .transient_for(parent)
        .destroy_with_parent(true)
        .build();
    window.set_application(parent.application().as_ref());
    window.set_widget_name("developer-console");
    let pages = gtk::Stack::builder().hexpand(true).vexpand(true).build();
    for (name, title, page) in [
        ("overview", "Overview", super::overview::build()),
        ("provider", "Provider", super::provider::build()),
        ("detector", "Detector", super::detector::build()),
        ("scanner", "Scanner", super::scanner::build()),
        ("inventory", "Inventory", super::inventory::build()),
        ("analyzer", "Analyzer", super::analyzer::build()),
        ("operator", "Operator", super::operator::build()),
        ("orchestrator", "Orchestrator", super::orchestrator::build()),
        ("logs", "Logs", super::logs::build()),
    ] {
        page.set_widget_name(&format!("developer-{name}"));
        pages.add_titled(&page, Some(name), title);
    }
    let split = adw::NavigationSplitView::builder()
        .min_sidebar_width(220.0)
        .max_sidebar_width(280.0)
        .sidebar_width_fraction(0.28)
        .build();
    let sidebar = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .activate_on_single_click(true)
        .css_classes(["navigation-sidebar"])
        .build();
    for (title, icon) in [
        ("Overview", "view-grid-symbolic"),
        ("Provider", "network-server-symbolic"),
        ("Detector", "view-refresh-symbolic"),
        ("Scanner", "folder-saved-search-symbolic"),
        ("Inventory", "view-list-symbolic"),
        ("Analyzer", "system-search-symbolic"),
        ("Operator", "system-run-symbolic"),
        ("Orchestrator", "applications-system-symbolic"),
        ("Logs", "text-x-generic-symbolic"),
    ] {
        let row = adw::ActionRow::builder().title(title).build();
        row.add_prefix(&gtk::Image::from_icon_name(icon));
        sidebar.append(&row);
    }
    let sidebar_toolbar = adw::ToolbarView::new();
    let sidebar_header = adw::HeaderBar::new();
    sidebar_header.set_title_widget(Some(&adw::WindowTitle::new("Developer Console", "")));
    sidebar_toolbar.add_top_bar(&sidebar_header);
    sidebar_toolbar.set_content(Some(
        &gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&sidebar)
            .build(),
    ));
    split.set_sidebar(Some(&adw::NavigationPage::new(
        &sidebar_toolbar,
        "Developer Console",
    )));
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    split
        .bind_property("collapsed", &header, "show-back-button")
        .sync_create()
        .build();
    let title = adw::WindowTitle::new("Overview", "");
    header.set_title_widget(Some(&title));
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&pages));
    let content = adw::NavigationPage::new(&toolbar, "Overview");
    split.set_content(Some(&content));
    sidebar.connect_row_selected({
        let pages = pages.downgrade();
        let title = title.downgrade();
        let content = content.downgrade();
        move |_, row| {
            let Some(row) = row else { return };
            let (Some(pages), Some(title), Some(content)) =
                (pages.upgrade(), title.upgrade(), content.upgrade())
            else {
                return;
            };
            if let Some(page) = pages
                .pages()
                .item(row.index() as u32)
                .and_downcast::<gtk::StackPage>()
            {
                pages.set_visible_child(&page.child());
                let name = page.title().unwrap_or_default();
                title.set_title(&name);
                content.set_title(&name);
            }
        }
    });
    sidebar.select_row(sidebar.row_at_index(0).as_ref());
    sidebar.connect_row_activated({
        let split = split.downgrade();
        move |_, _| {
            if let Some(split) = split.upgrade() {
                split.set_show_content(true);
            }
        }
    });
    let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        660.0,
        adw::LengthUnit::Sp,
    ));
    breakpoint.add_setter(&split, "collapsed", Some(&true.to_value()));
    window.set_size_request(360, 360);
    window.add_breakpoint(breakpoint);
    window.set_content(Some(&split));
    window
}
