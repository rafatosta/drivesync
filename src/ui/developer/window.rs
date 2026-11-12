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
    let sidebar = gtk::StackSidebar::new();
    sidebar.set_stack(&pages);
    sidebar.set_size_request(220, -1);
    let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    body.append(&sidebar);
    body.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    body.append(&pages);
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new("Developer Console", "")));
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&body));
    window.set_content(Some(&toolbar));
    window
}
