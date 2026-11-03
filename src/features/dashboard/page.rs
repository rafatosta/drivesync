//! Adaptive dashboard page, account selection and feature composition.
use adw::{gtk, prelude::*};
use drivesync_engine::{DriveSyncEngine, RestoredAccount};
use std::sync::Arc;

/// Builds account navigation with account-scoped configuration and automatic downloads.
pub fn build(
    engine: Arc<DriveSyncEngine>,
    accounts: Vec<RestoredAccount>,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
) -> adw::NavigationPage {
    build_selected(engine, accounts, nav, toasts, None)
}

/// Builds the dashboard with the requested account selected after authorization.
pub fn build_selected(
    engine: Arc<DriveSyncEngine>,
    accounts: Vec<RestoredAccount>,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    selected: Option<&str>,
) -> adw::NavigationPage {
    crate::features::synchronization::automatic::reconcile(engine.clone(), &accounts);
    let split = adw::NavigationSplitView::builder()
        .min_sidebar_width(240.0)
        .max_sidebar_width(300.0)
        .sidebar_width_fraction(0.28)
        .build();
    split.set_widget_name("dashboard-split");
    let sidebar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new("DriveSync", "")));
    header.pack_start(&crate::app::main_menu::add_account_button());
    header.pack_end(&crate::app::main_menu::button());
    sidebar.add_top_bar(&header);
    let side_content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let count = gtk::Label::builder()
        .label(format!("Accounts · {}", accounts.len()))
        .xalign(0.0)
        .margin_start(18)
        .margin_top(20)
        .css_classes(["heading"])
        .build();
    side_content.append(&count);
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .activate_on_single_click(true)
        .css_classes(["navigation-sidebar"])
        .build();
    list.set_widget_name("account-list");
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&list)
        .build();
    side_content.append(&scroller);
    sidebar.set_content(Some(&side_content));
    let version = gtk::Label::builder()
        .label(crate::core::build_info::VERSION)
        .css_classes(["caption", "dim-label"])
        .selectable(true)
        .margin_top(8)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .tooltip_text(crate::core::build_info::REVISION)
        .build();
    sidebar.add_bottom_bar(&version);
    split.set_sidebar(Some(&adw::NavigationPage::new(&sidebar, "Accounts")));

    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    split
        .bind_property("collapsed", &header, "show-back-button")
        .sync_create()
        .build();
    let menu = crate::app::main_menu::button();
    split
        .bind_property("collapsed", &menu, "visible")
        .sync_create()
        .build();
    header.pack_end(&menu);
    toolbar.add_top_bar(&header);
    let details = gtk::Stack::new();
    details.set_widget_name("account-details");
    let ids: Vec<_> = accounts.iter().map(|a| a.account.id.0.clone()).collect();
    let initial = selected
        .and_then(|id| ids.iter().position(|candidate| candidate == id))
        .unwrap_or(0);
    let preferences = super::account_view::populate(engine, accounts, nav, toasts, &list, &details);
    let weak_details = details.downgrade();
    let weak_split = split.downgrade();
    list.connect_row_selected(move |_, row| {
        if let (Some(details), Some(row)) = (weak_details.upgrade(), row) {
            if let Some(id) = ids.get(row.index() as usize) {
                details.set_visible_child_name(id);
                if let Some(split) = weak_split.upgrade() {
                    split.set_show_content(true);
                }
            }
        }
    });
    let weak_split = split.downgrade();
    list.connect_row_activated(move |_, _| {
        if let Some(split) = weak_split.upgrade() {
            split.set_show_content(true);
        }
    });
    list.select_row(list.row_at_index(initial as i32).as_ref());
    toolbar.set_content(Some(&crate::ui::scroll(&details, 760)));
    split.set_content(Some(&adw::NavigationPage::new(
        &toolbar,
        &gettextrs::gettext("Account"),
    )));
    let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        660.0,
        adw::LengthUnit::Sp,
    ));
    breakpoint.add_setter(&split, "collapsed", Some(&true.to_value()));
    // Breakpoints belong to a breakpoint bin, not the application's outer routing view.
    let bin = adw::BreakpointBin::new();
    bin.set_width_request(320);
    bin.set_height_request(320);
    bin.set_child(Some(&split));
    bin.add_breakpoint(breakpoint);
    let page = adw::NavigationPage::builder()
        .title("Dashboard")
        .child(&bin)
        .build();
    let actions = gtk::gio::SimpleActionGroup::new();
    let open = gtk::gio::SimpleAction::new("preferences", None);
    let weak_list = list.downgrade();
    open.connect_activate(move |_, _| {
        let Some(list) = weak_list.upgrade() else {
            return;
        };
        if let Some(parent) = list.root().and_downcast::<gtk::Window>() {
            let row = list.selected_row();
            let id = row
                .as_ref()
                .and_then(|row| preferences.id_at(row.index() as usize));
            crate::features::preferences::present_with_accounts(
                &parent,
                preferences.clone(),
                id,
                false,
            );
        }
    });
    actions.add_action(&open);
    page.insert_action_group("account", Some(&actions));
    page
}

#[cfg(test)]
pub(crate) fn verify_download_worker_failure() {
    super::synchronization_view::verify_worker_failure();
}

#[cfg(test)]
pub(crate) fn verify_download_progress(mut capture: impl FnMut(&adw::Window, &str)) {
    crate::features::preferences::verify_upload_presentations(&mut capture);
    super::synchronization_view::verify_progress_presentations(capture);
}
